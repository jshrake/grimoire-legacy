use std;
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::default::Default;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use crate::config::*;
use crate::error::{Error, ErrorKind, Result};
use crate::gl;
use crate::gl::{GLRc, GLenum, GLint, GLsizei, GLuint, GLvoid};
use crate::resource::{ResourceCubemapFace, ResourceData};
use failure::ResultExt;

const PBO_COUNT: usize = 3;

#[derive(Debug)]
pub struct Effect<'a> {
    config: EffectConfig,
    version: String,
    window_resolution: [f32; 3],
    staged_resources: BTreeMap<u64, Vec<ResourceData>>,
    staged_uniform_buffer: BTreeMap<String, Vec<u8>>,
    staged_uniform_1f: BTreeMap<Cow<'a, str>, f32>,
    staged_uniform_2f: BTreeMap<Cow<'a, str>, [f32; 2]>,
    staged_uniform_3f: BTreeMap<Cow<'a, str>, [f32; 3]>,
    staged_uniform_4f: BTreeMap<Cow<'a, str>, [f32; 4]>,
    shader_cache: BTreeMap<String, String>,
    pbo_texture_unpack_list: Vec<(GLPbo, GLResource)>,
    pipeline: GLPipeline,
    resources: BTreeMap<u64, GLResource>,
    framebuffers: BTreeMap<String, GLFramebuffer>,
    config_dirty: bool,
    pipeline_dirty: bool,
}

// The layout of this struct must match the layout of
// the uniform block GRIM_STATE defined in file header.glsl
#[derive(Debug)]
pub struct EffectState {
    pub mouse: [f32; 4],
    pub date: [f32; 4],
    pub window_resolution: [f32; 3],
    pub time: f32,
    pub time_delta: f32,
    pub frame: f32,
    pub frame_rate: f32,
}

#[derive(Debug, Default, Clone, Copy)]
struct GLResource {
    target: GLenum,
    texture: GLuint,
    resolution: [f32; 3],
    time: f32,
    pbos: [GLPbo; PBO_COUNT],
    pbo_idx: usize,
    params: GLTextureParam,
}

#[derive(Debug, Default, Clone, Copy)]
struct GLPbo {
    pbo: GLuint,
    xoffset: GLint,
    yoffset: GLint,
    subwidth: GLsizei,
    subheight: GLsizei,
    width: GLsizei,
    height: GLsizei,
}

#[derive(Debug, Default, Clone)]
struct GLFramebuffer {
    framebuffer: GLuint,
    depth_renderbuffer: GLuint,
    attachment_count: usize,
    color_attachments: Vec<u64>,
    resolution: [f32; 3],
}

#[derive(Debug, Default)]
struct GLPipeline {
    vertex_array_object: GLuint,
    // Track uniform block names to uniform buffer objects
    uniform_buffers: BTreeMap<String, GLuint>,
    passes: Vec<GLPass>,
}

#[derive(Debug, Default)]
struct GLPass {
    // program resources
    vertex_shader: GLuint,
    fragment_shader: GLuint,
    program: GLuint,
    // uniforms
    resolution_uniform_loc: GLint,
    vertex_count_uniform_loc: GLint,
    samplers: Vec<GLSampler>,
    // render state
    draw_mode: GLenum,
    draw_count: GLsizei,
    clear_color: Option<[f32; 4]>,
    blend: Option<(GLenum, GLenum)>,
    depth: Option<GLenum>,
}

#[derive(Debug, Default)]
struct GLSampler {
    resource: u64,
    uniform_loc: GLint,
    resolution_uniform_loc: GLint,
    playback_time_uniform_loc: GLint,
    wrap_s: GLuint,
    wrap_t: GLuint,
    wrap_r: GLuint,
    min_filter: GLuint,
    mag_filter: GLuint,
}

impl<'a> Default for Effect<'a> {
    fn default() -> Self {
        Self {
            version: Default::default(),
            config: Default::default(),
            staged_resources: Default::default(),
            staged_uniform_buffer: Default::default(),
            resources: Default::default(),
            pipeline: Default::default(),
            framebuffers: Default::default(),
            pbo_texture_unpack_list: Default::default(),
            window_resolution: Default::default(),
            staged_uniform_1f: Default::default(),
            staged_uniform_2f: Default::default(),
            staged_uniform_3f: Default::default(),
            staged_uniform_4f: Default::default(),
            shader_cache: Default::default(),
            config_dirty: true,
            pipeline_dirty: true,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
struct GLTextureParam {
    internal: GLenum,
    format: GLenum,
    data_type: GLenum,
}

impl<'a> Effect<'a> {
    pub fn new(glsl_version: String) -> Self {
        Self {
            version: glsl_version,
            ..Default::default()
        }
    }

    pub fn stage_config(&mut self, config: EffectConfig) -> Result<()> {
        debug!("[SHADER] config={:?}", config);
        // Only mark the config as dirty if it's different from our existing config
        if config != self.config {
            self.config_dirty = true;
            self.config = config;
            self.staged_resources.clear();
        }
        Ok(())
    }

    pub fn stage_shader_cache(&mut self, shader_cache: BTreeMap<String, String>) -> Result<()> {
        debug!("[SHADER] shader_cache={:?}", shader_cache);
        self.pipeline_dirty = true;
        self.shader_cache = shader_cache;
        Ok(())
    }

    pub fn stage_resource(&mut self, name: &str, resource: ResourceData) {
        let instant = Instant::now();
        let hashed_name = hash_name_attachment(name, 0);
        let resource_display = resource.to_string();
        self.staged_resources
            .entry(hashed_name)
            .or_insert_with(Vec::new)
            .push(resource);
        debug!(
            "[DATA] {}={}, took {:?}",
            name,
            resource_display,
            instant.elapsed()
        );
    }

    pub fn stage_state(&mut self, name: &str, state: &EffectState) {
        self.stage_buffer_data(name, state);
    }

    pub fn stage_uniform1f<S: Into<Cow<'a, str>>>(&mut self, name: S, data: f32) {
        self.staged_uniform_1f.insert(name.into(), data);
    }

    pub fn stage_uniform2f<S: Into<Cow<'a, str>>>(&mut self, name: S, data: [f32; 2]) {
        self.staged_uniform_2f.insert(name.into(), data);
    }

    pub fn stage_uniform3f<S: Into<Cow<'a, str>>>(&mut self, name: S, data: [f32; 3]) {
        self.staged_uniform_3f.insert(name.into(), data);
    }

    pub fn stage_uniform4f<S: Into<Cow<'a, str>>>(&mut self, name: S, data: [f32; 4]) {
        self.staged_uniform_4f.insert(name.into(), data);
    }

    pub fn draw(&mut self, gl: &GLRc, window_width: f32, window_height: f32) -> Result<()> {
        // TODO(jshrake): Consider adding the following to the config: enables: ["multisample, framebuffer_srgb"]
        //gl.enable(gl::MULTISAMPLE);
        //gl.enable(gl::FRAMEBUFFER_SRGB);
        gl.enable(gl::TEXTURE_CUBE_MAP_SEAMLESS);
        gl.enable(gl::PROGRAM_POINT_SIZE);

        // Clear the default framebuffer initially to a weird color to signal an error
        gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        gl.viewport(0, 0, window_width as i32, window_height as i32);
        gl.clear_color(0.7, 0.1, 0.8, 1.0); // a random error color I picked arbitrarily
        gl.clear(gl::COLOR_BUFFER_BIT);

        // If the config didn't validate, go no further
        // The user needs to fix the error in their file
        if !self.config.is_ok() {
            return Ok(());
        }

        // determine what we need to initialize, and reset various dirty flags
        let resources_need_init = self.config_dirty;
        let framebuffers_need_init = self.config_dirty;
        let pipeline_need_init = self.pipeline_dirty;
        let window_resized = (self.window_resolution[0] - window_width).abs() > std::f32::EPSILON
            || (self.window_resolution[1] - window_height).abs() > std::f32::EPSILON;
        self.config_dirty = false;
        self.pipeline_dirty = false;
        self.window_resolution[0] = window_width;
        self.window_resolution[1] = window_height;
        self.window_resolution[2] = self.window_resolution[0] / self.window_resolution[1];

        // delete non framebuffer resources on dirty config
        if resources_need_init {
            let instant = Instant::now();
            self.gpu_delete_non_buffer_resources(gl);
            debug!("[DRAW] Deleting resources took {:?}", instant.elapsed());
        }

        // build or rebuild framebuffers on resize
        if framebuffers_need_init || window_resized {
            let instant = Instant::now();
            self.gpu_delete_buffer_resources(gl);
            self.gpu_init_framebuffers(gl);
            debug!(
                "[DRAW] Initializing framebuffer objects took {:?}",
                instant.elapsed()
            );
        }

        // build or rebuild the rendering pipeline
        if pipeline_need_init {
            let instant = Instant::now();
            self.gpu_delete_pipeline_resources(gl);
            self.gpu_init_pipeline(gl)?;
            debug!(
                "[DRAW] Initializing rendering pipeline took {:?}",
                instant.elapsed()
            );
        }

        // Return early if gpu pipeline is not ok. This indicates that gpu_init_pipeline
        // failed and the user needs to fix the error in their shader file
        if !self.gpu_pipeline_is_ok() {
            self.staged_resources.clear();
            return Ok(());
        }

        let instant = Instant::now();
        self.gpu_stage_resources(gl);
        self.gpu_stage_buffer_data(gl);
        debug!("[DRAW] Resource uploads took {:?}", instant.elapsed());

        let instant = Instant::now();
        self.gpu_draw(gl)?;
        debug!("[DRAW] Draw took {:?}", instant.elapsed());

        let instant = Instant::now();
        self.gpu_pbo_ping_pong(gl);
        debug!("[DRAW] PBO ping pong took {:?}", instant.elapsed());

        let instant = Instant::now();
        self.gpu_pbo_to_texture_transfer(gl);
        debug!("[DRAW] PBO to texture upload took {:?}", instant.elapsed());
        Ok(())
    }

    fn framebuffer_for_pass(&self, pass: &PassConfig) -> Option<&GLFramebuffer> {
        if let Some(ref buffer_name) = pass.buffer {
            self.framebuffers.get(buffer_name)
        } else {
            None
        }
    }

    fn gpu_pipeline_is_ok(&self) -> bool {
        // Assume our pipeline is ok if the count matches the
        // number of passes defined in the config
        self.pipeline.passes.len() == self.config.passes.len()
    }

    fn stage_buffer_data<T: Sized + std::fmt::Debug>(&mut self, name: &str, data: &T) {
        let instant = Instant::now();
        let bytes: &[u8] = unsafe { to_slice::<T, u8>(data) };
        self.staged_uniform_buffer
            .insert(name.to_string(), Vec::from(bytes));
        debug!("[DATA] {}={:?} took {:?}", name, data, instant.elapsed());
    }

    fn gpu_delete_non_buffer_resources(&mut self, gl: &GLRc) {
        let mut framebuffer_attachment_set = BTreeSet::new();
        for framebuffer in self.framebuffers.values() {
            for attachment in &framebuffer.color_attachments {
                framebuffer_attachment_set.insert(attachment);
            }
        }
        // Delete all GL texture resources except the ones
        // marked as framebuffer attachments
        for (hash, resource) in &self.resources {
            if framebuffer_attachment_set.contains(hash) {
                continue;
            }
            gl.delete_textures(&[resource.texture]);
            for pbo in &resource.pbos {
                gl.delete_buffers(&[pbo.pbo]);
            }
        }
        // Remove all resources except for the ones marked as framebuffer attachments
        self.resources = self
            .resources
            .iter()
            .filter(move |(hash, _)| framebuffer_attachment_set.contains(hash))
            .map(|(hash, resource)| (*hash, *resource))
            .collect();
    }

    fn gpu_delete_buffer_resources(&mut self, gl: &GLRc) {
        // Free current framebuffer resources
        for framebuffer in self.framebuffers.values() {
            // NOTE: Each framebuffer has several color attachments. We need to remove them from the
            // resources array, and delete them from GL
            for color_attachment in &framebuffer.color_attachments {
                if let Some(resource) = self.resources.remove(color_attachment) {
                    gl.delete_textures(&[resource.texture]);
                } else {
                    unreachable!(format!(
                        "Unable to remove collor attachment {} from framebuffer {:?}",
                        color_attachment, framebuffer
                    ));
                }
            }
            gl.delete_renderbuffers(&[framebuffer.depth_renderbuffer]);
            gl.delete_framebuffers(&[framebuffer.framebuffer]);
        }
        self.framebuffers.clear();
    }

    fn gpu_delete_pipeline_resources(&mut self, gl: &GLRc) {
        gl.delete_vertex_arrays(&[self.pipeline.vertex_array_object]);
        for pass in &self.pipeline.passes {
            gl.delete_program(pass.program);
            gl.delete_shader(pass.vertex_shader);
            gl.delete_shader(pass.fragment_shader);
        }
        self.pipeline.passes.clear();
    }

    fn gpu_pbo_to_texture_transfer(&mut self, gl: &GLRc) {
        // PBO->Texture unpack
        gl.active_texture(gl::TEXTURE0);
        for (pbo, resource) in &self.pbo_texture_unpack_list {
            gl.bind_texture(resource.target, resource.texture);
            gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, pbo.pbo);
            gl.tex_sub_image_2d_pbo(
                resource.target,
                0,
                pbo.xoffset as i32,
                pbo.yoffset as i32,
                pbo.subwidth as i32,
                pbo.subheight as i32,
                resource.params.format,
                resource.params.data_type,
                0,
            );
            gl.generate_mipmap(gl::TEXTURE_2D);
        }
        gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, 0);
        self.pbo_texture_unpack_list.clear();
    }

    fn gpu_pbo_ping_pong(&mut self, gl: &GLRc) {
        for framebuffer in self.framebuffers.values() {
            let attachment_count = framebuffer.attachment_count;
            for attachment_idx in 0..attachment_count {
                let ping_hash = framebuffer.color_attachments[attachment_idx as usize];
                let pong_hash =
                    framebuffer.color_attachments[(attachment_idx + attachment_count) as usize];
                let ping = self.resources[&ping_hash];
                let pong = self.resources[&pong_hash];
                // generate mipmaps for the color attachment that we just drew to
                gl.active_texture(gl::TEXTURE0);
                gl.bind_texture(gl::TEXTURE_2D, pong.texture);
                gl.generate_mipmap(gl::TEXTURE_2D);
                // swap the ping and pong resources
                self.resources.insert(ping_hash, pong);
                self.resources.insert(pong_hash, ping);
                // bind the ping resource
                gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer.framebuffer);
                gl.framebuffer_texture_2d(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0 + attachment_idx as u32,
                    gl::TEXTURE_2D,
                    ping.texture,
                    0,
                );
            }
        }
        gl.bind_texture(gl::TEXTURE_2D, 0);
        gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
    }

    fn gpu_draw(&mut self, gl: &GLRc) -> Result<()> {
        // Now that all OpenGL resources are configured, perform the actual draw
        let default_framebuffer = GLFramebuffer {
            framebuffer: 0,
            resolution: self.window_resolution,
            ..Default::default()
        };
        gl.bind_vertex_array(self.pipeline.vertex_array_object);
        for (pass_idx, pass) in self.pipeline.passes.iter().enumerate() {
            let pass_config = &self.config.passes[pass_idx];
            // Don't draw this pass if it's marked as disabled
            if pass_config.disable {
                continue;
            }
            // Find the framebuffer corresponding to the pass configuration
            // The lookup can fail if the user supplies a bad configuration,
            // like a typo in the buffer value
            let framebuffer = self
                .framebuffer_for_pass(&pass_config)
                .unwrap_or(&default_framebuffer);
            gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer.framebuffer);
            if let Some(clear_color) = pass.clear_color {
                gl.clear_color(
                    clear_color[0],
                    clear_color[1],
                    clear_color[2],
                    clear_color[3],
                );
                gl.clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
            }
            // Set the viewport to match the framebuffer resolution
            gl.viewport(
                0,
                0,
                framebuffer.resolution[0] as i32,
                framebuffer.resolution[1] as i32,
            );

            // Bind the program for this pass
            gl.use_program(pass.program);

            // Set per-pass non-sampler uniforms
            if pass.resolution_uniform_loc > -1 {
                gl.uniform_3fv(pass.resolution_uniform_loc, &framebuffer.resolution);
            }
            if pass.vertex_count_uniform_loc > -1 {
                gl.uniform_1i(pass.vertex_count_uniform_loc, pass.draw_count);
            }

            // Set staged uniform data
            // TODO: cache get_uniform_location calls
            for (name, data) in &self.staged_uniform_1f {
                let loc = gl.get_uniform_location(pass.program, &name);
                gl.uniform_1f(loc, *data);
            }
            for (name, data) in &self.staged_uniform_2f {
                let loc = gl.get_uniform_location(pass.program, &name);
                gl.uniform_2fv(loc, data);
            }
            for (name, data) in &self.staged_uniform_3f {
                let loc = gl.get_uniform_location(pass.program, &name);
                gl.uniform_3fv(loc, data);
            }
            for (name, data) in &self.staged_uniform_4f {
                let loc = gl.get_uniform_location(pass.program, &name);
                gl.uniform_4fv(loc, data);
            }

            // Set per-pass sampler uniforms, bind textures, and set sampler properties
            for (sampler_idx, ref sampler) in pass.samplers.iter().enumerate() {
                if sampler.uniform_loc < 0 {
                    // Note that this is not necessarily an error. The user may simply not be
                    // referencing some uniform, so the GLSL compiler compiles it out and
                    // we get an invalid unifrom loc. That's fine -- just keep moving on
                    continue;
                }
                if let Some(resource) = self.resources.get(&sampler.resource) {
                    gl.active_texture(gl::TEXTURE0 + sampler_idx as u32);
                    gl.bind_texture(resource.target, resource.texture);
                    gl.tex_parameter_i(resource.target, gl::TEXTURE_WRAP_S, sampler.wrap_s as i32);
                    gl.tex_parameter_i(resource.target, gl::TEXTURE_WRAP_T, sampler.wrap_t as i32);
                    if resource.target == gl::TEXTURE_3D || resource.target == gl::TEXTURE_CUBE_MAP
                    {
                        gl.tex_parameter_i(
                            resource.target,
                            gl::TEXTURE_WRAP_R,
                            sampler.wrap_r as i32,
                        );
                    }
                    gl.tex_parameter_i(
                        resource.target,
                        gl::TEXTURE_MIN_FILTER,
                        sampler.min_filter as i32,
                    );
                    gl.tex_parameter_i(
                        resource.target,
                        gl::TEXTURE_MAG_FILTER,
                        sampler.mag_filter as i32,
                    );
                    gl.uniform_1i(sampler.uniform_loc, sampler_idx as i32);
                    // bind resolution & playback time uniforms
                    if sampler.resolution_uniform_loc > -1 {
                        gl.uniform_3fv(sampler.resolution_uniform_loc as i32, &resource.resolution);
                    }
                    if sampler.playback_time_uniform_loc > -1 {
                        gl.uniform_1f(sampler.playback_time_uniform_loc as i32, resource.time);
                    }
                }
            }
            // Set the blend state
            if let Some((src, dst)) = pass.blend {
                gl.enable(gl::BLEND);
                gl.blend_func(src, dst);
            } else {
                gl.disable(gl::BLEND);
            }
            // Set the depth state
            if let Some(func) = pass.depth {
                gl.enable(gl::DEPTH_TEST);
                gl.depth_func(func);
            } else {
                gl.disable(gl::DEPTH_TEST);
            }
            // Call draw_buffers if we have attachments
            // Assuming this is not the default framebuffer, we always
            // have at least one color attachment
            let draw_buffers: Vec<GLenum> = (0..framebuffer.attachment_count)
                .map(|i| gl::COLOR_ATTACHMENT0 + i as u32)
                .collect();
            if !draw_buffers.is_empty() {
                gl.draw_buffers(&draw_buffers);
            }
            // Draw!
            gl.draw_arrays(pass.draw_mode, 0, pass.draw_count);
            gl.use_program(0);
        }
        self.staged_uniform_1f.clear();
        self.staged_uniform_2f.clear();
        self.staged_uniform_3f.clear();
        self.staged_uniform_4f.clear();
        Ok(())
    }

    fn gpu_init_pipeline(&mut self, gl: &GLRc) -> Result<()> {
        self.pipeline.vertex_array_object = gl::create_vao(gl);
        let uniform_strings = {
            // build the list of uniform strings from the resouces config
            let mut uniform_strings = Vec::new();
            for (name, input) in &self.config.resources {
                let type_str = match input {
                    ResourceConfig::UniformFloat(_) => "float",
                    ResourceConfig::UniformVec2(_) => "vec2",
                    ResourceConfig::UniformVec3(_) => "vec3",
                    ResourceConfig::UniformVec4(_) => "vec4",
                    _ => continue,
                };
                uniform_strings.push(format!("uniform {} {};", type_str, name));
            }
            uniform_strings
        };
        for (pass_index, pass_config) in self.config.passes.iter().enumerate() {
            // Build out the uniform sampler declarations for this pass
            let uniform_sampler_strings = {
                let mut uniform_sampler_strings = Vec::new();
                for (uniform_name, channel_config) in &pass_config.uniform_to_channel {
                    let resource_name = match channel_config {
                        ChannelConfig::Simple(name) => name,
                        ChannelConfig::Complete { resource, .. } => resource,
                    };
                    let resource_config = self
                        .config
                        .resources
                        .get(resource_name)
                        .expect("expected config.validate() to catch this error");
                    let sampler_str = match resource_config {
                        ResourceConfig::Image(_) => "sampler2D",
                        ResourceConfig::Video(_) => "sampler2D",
                        ResourceConfig::WebCam(_) => "sampler2D",
                        ResourceConfig::Keyboard(_) => "sampler2D",
                        ResourceConfig::Microphone(_) => "sampler2D",
                        ResourceConfig::Audio(_) => "sampler2D",
                        ResourceConfig::Texture2D(_) => "sampler2D",
                        ResourceConfig::Texture3D(_) => "sampler3D",
                        ResourceConfig::Cubemap(_) => "samplerCube",
                        ResourceConfig::GstAppSinkPipeline(_) => "sampler2D",
                        ResourceConfig::Buffer(_) => "sampler2D",
                        _ => continue,
                    };
                    uniform_sampler_strings
                        .push(format!("uniform {} {};", sampler_str, uniform_name));
                    uniform_sampler_strings
                        .push(format!("uniform vec3 {}_Resolution;", uniform_name));
                    uniform_sampler_strings.push(format!("uniform vec3 {}_Time;", uniform_name));
                }
                uniform_sampler_strings
            };
            let vertex_path = &pass_config.vertex;
            let fragment_path = &pass_config.fragment;
            let vertex_source = self
                .shader_cache
                .get(vertex_path)
                .expect("vertex path not found in shader_cache");
            let fragment_source = self
                .shader_cache
                .get(fragment_path)
                .expect("fragment path not found in shader_cache");
            let vertex_shader_list = {
                let mut list = Vec::new();
                list.push(self.version.clone());
                list.push(include_str!("./shadertoy_uniforms.glsl").to_string());
                list.append(&mut uniform_strings.clone());
                list.append(&mut uniform_sampler_strings.clone());
                list.push("#line 1 0".to_string());
                list.push(vertex_source.clone());
                list.join("\n")
            };
            let fragment_shader_list = {
                let mut list = Vec::new();
                list.push(self.version.clone());
                list.push(include_str!("./shadertoy_uniforms.glsl").to_string());
                list.append(&mut uniform_strings.clone());
                list.append(&mut uniform_sampler_strings.clone());
                list.push("#line 1 0".to_string());
                list.push(fragment_source.clone());
                list.join("\n")
            };
            let vertex_shader =
                gl::create_shader(gl, gl::VERTEX_SHADER, &[vertex_shader_list.as_bytes()])
                    .map_err(Error::glsl_vertex)
                    .with_context(|_| ErrorKind::GLPass(pass_index))?;
            assert!(vertex_shader != 0);
            let fragment_shader =
                gl::create_shader(gl, gl::FRAGMENT_SHADER, &[fragment_shader_list.as_bytes()])
                    .map_err(|err| {
                        gl.delete_shader(vertex_shader);
                        Error::glsl_fragment(err)
                    })
                    .with_context(|_| ErrorKind::GLPass(pass_index))?;
            assert!(fragment_shader != 0);
            let program = gl::create_program(gl, vertex_shader, fragment_shader)
                .map_err(|err| {
                    gl.delete_shader(vertex_shader);
                    gl.delete_shader(fragment_shader);
                    Error::glsl_program(err)
                })
                .with_context(|_| ErrorKind::GLPass(pass_index))?;
            assert!(program != 0);

            // build the samplers used in drawing this pass
            let mut samplers = Vec::new();
            for (uniform_name, channel_config) in &pass_config.uniform_to_channel {
                let uniform_loc = gl.get_uniform_location(program, &uniform_name);
                let resolution_uniform_loc =
                    gl.get_uniform_location(program, &format!("{}_Resolution", &uniform_name));
                let playback_time_uniform_loc =
                    gl.get_uniform_location(program, &format!("{}_Time", &uniform_name));
                let (resource, wrap, min_filter, mag_filter) = match channel_config {
                    ChannelConfig::Simple(ref name) => {
                        let hash = hash_name_attachment(name, 0);
                        // Default to linear mag filter for texture3D resources
                        let min_filter = {
                            if let Some(resource) = self.resources.get(&hash) {
                                match resource.target {
                                    gl::TEXTURE_3D => gl::LINEAR,
                                    _ => gl::LINEAR_MIPMAP_LINEAR,
                                }
                            } else {
                                gl::LINEAR_MIPMAP_LINEAR
                            }
                        };
                        (hash, gl::REPEAT, min_filter, gl::LINEAR)
                    }
                    ChannelConfig::Complete {
                        resource,
                        attachment,
                        wrap,
                        filter,
                    } => {
                        let hash = hash_name_attachment(resource, *attachment);
                        (
                            hash,
                            gl_wrap_from_config(&wrap),
                            gl_min_filter_from_config(&filter),
                            gl_mag_filter_from_config(&filter),
                        )
                    }
                };
                samplers.push(GLSampler {
                    resource,
                    resolution_uniform_loc,
                    playback_time_uniform_loc,
                    uniform_loc,
                    mag_filter,
                    min_filter,
                    wrap_r: wrap,
                    wrap_s: wrap,
                    wrap_t: wrap,
                });
            }
            // get per-pass uniforms for this program
            let resolution_uniform_loc = gl.get_uniform_location(program, "iResolution");
            let vertex_count_uniform_loc = gl.get_uniform_location(program, "iVertexCount");

            // specify draw state
            let draw_count = pass_config.draw.count as i32;
            let (draw_mode, draw_count) = match pass_config.draw.mode {
                DrawModeConfig::Triangles => (gl::TRIANGLES, 3 * draw_count),
                DrawModeConfig::Points => (gl::POINTS, draw_count),
                DrawModeConfig::Lines => (gl::LINES, 2 * draw_count),
                DrawModeConfig::TriangleFan => (gl::TRIANGLE_FAN, 3 * draw_count),
                DrawModeConfig::TriangleStrip => (gl::TRIANGLE_STRIP, 3 * draw_count),
                DrawModeConfig::LineLoop => (gl::LINE_LOOP, 2 * draw_count),
                DrawModeConfig::LineStrip => (gl::LINE_STRIP, 2 * draw_count),
            };
            let blend = pass_config.blend.as_ref().map(|blend| {
                (
                    gl_blend_from_config(&blend.src),
                    gl_blend_from_config(&blend.dst),
                )
            });
            let depth = pass_config
                .depth
                .as_ref()
                .map(|depth| gl_depth_from_config(&depth));
            self.pipeline.passes.push(GLPass {
                // shader resources
                vertex_shader,
                fragment_shader,
                program,
                // uniforms
                resolution_uniform_loc,
                vertex_count_uniform_loc,
                samplers,
                // render state
                draw_mode,
                draw_count,
                blend,
                depth,
                clear_color: pass_config.clear,
            })
        }
        // Now that we built all the pass programs, remember to connect the existing
        // uniform buffers to the programs
        for (index, (name, buffer)) in self.pipeline.uniform_buffers.iter().enumerate() {
            for pass in &self.pipeline.passes {
                gl::connect_uniform_buffer(gl, *buffer, pass.program, name, index as u32);
            }
        }
        Ok(())
    }

    fn gpu_stage_buffer_data(&mut self, gl: &GLRc) {
        for (uniform_name, data) in &self.staged_uniform_buffer {
            let programs = self.pipeline.passes.iter().map(|pass| pass.program);
            let index = self.pipeline.uniform_buffers.len() as u32;
            // If this is the first time we've seen this uniform_name,
            // we'll need to create a new uniform buffer, connect
            // it to call the programs, and allocate
            let buffer = self
                .pipeline
                .uniform_buffers
                .entry(uniform_name.to_string())
                .or_insert_with(|| {
                    let buffer = gl::create_buffer(gl);
                    for program in programs {
                        gl::connect_uniform_buffer(gl, buffer, program, uniform_name, index);
                    }
                    gl.bind_buffer(gl::UNIFORM_BUFFER, buffer);
                    gl.buffer_data_untyped(
                        gl::UNIFORM_BUFFER,
                        data.len() as isize,
                        std::ptr::null(),
                        gl::STREAM_DRAW,
                    );
                    buffer
                });
            gl.bind_buffer(gl::UNIFORM_BUFFER, *buffer);
            gl.buffer_sub_data_untyped(
                gl::UNIFORM_BUFFER,
                0,
                data.len() as isize,
                data.as_ptr() as *const GLvoid,
            );
        }
    }

    fn gpu_init_framebuffers(&mut self, gl: &GLRc) {
        for (resource_name, resource) in &self.config.resources {
            if let ResourceConfig::Buffer(buffer) = resource {
                let framebuffer = gl::create_framebuffer(gl);
                gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
                let mut color_attachments = Vec::new();
                let width = buffer.width.unwrap_or(self.window_resolution[0] as u32);
                let height = buffer.height.unwrap_or(self.window_resolution[1] as u32);
                let resolution = [width as f32, height as f32, width as f32 / height as f32];
                let (internal, format, data_type, bytes_per) = match buffer.format {
                    BufferFormat::U8 => (gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE, 1),
                    BufferFormat::F16 => (gl::RGBA16F, gl::RGBA, gl::HALF_FLOAT, 2),
                    BufferFormat::F32 => (gl::RGBA32F, gl::RGBA, gl::FLOAT, 4),
                };
                // zero out the allocated color attachments
                // Note that the attachments are 4 channels x bytes_per
                let zero_data = vec![0 as u8; (width * height * 4 * bytes_per) as usize];
                // Allocate twice as many attachments than we need,
                // so that we can ping pong the fbo color attachments
                // after drawing
                for attachment_index in 0..(2 * buffer.attachments as usize) {
                    let texture = gl::create_texture2d(
                        gl,
                        internal as i32,
                        width as i32,
                        height as i32,
                        format,
                        data_type,
                        Some(&zero_data),
                    );
                    gl.generate_mipmap(gl::TEXTURE_2D);
                    let hash = hash_name_attachment(resource_name, attachment_index);
                    color_attachments.push(hash);
                    self.resources.insert(
                        hash,
                        GLResource {
                            target: gl::TEXTURE_2D,
                            texture,
                            resolution,
                            time: Default::default(),
                            pbos: Default::default(),
                            pbo_idx: Default::default(),
                            params: Default::default(),
                        },
                    );
                    // The attachments from [pass_config.attachments, 2*pass_config.attachments)
                    // are initially bound to the framebuffer
                    if attachment_index >= buffer.attachments {
                        let resource = self.resources[&hash];
                        gl.framebuffer_texture_2d(
                            gl::FRAMEBUFFER,
                            gl::COLOR_ATTACHMENT0 + (attachment_index - buffer.attachments) as u32,
                            gl::TEXTURE_2D,
                            resource.texture,
                            0,
                        );
                    }
                }
                // create and attach a depth renderbuffer
                let depth_renderbuffer =
                    gl::create_renderbuffer(gl, gl::DEPTH_COMPONENT24, width as i32, height as i32);
                gl::attach_renderbuffer_to_framebuffer(
                    gl,
                    framebuffer,
                    depth_renderbuffer,
                    gl::DEPTH_ATTACHMENT,
                );
                // This should never fail
                assert!(gl::check_framebuffer_status(gl, framebuffer) == gl::FRAMEBUFFER_COMPLETE);
                self.framebuffers.insert(
                    resource_name.clone(),
                    GLFramebuffer {
                        framebuffer,
                        depth_renderbuffer,
                        color_attachments,
                        resolution,
                        attachment_count: buffer.attachments,
                    },
                );
            }
        }
    }

    fn gpu_stage_resources(&mut self, gl: &GLRc) {
        for (hash, staged_resource_list) in &self.staged_resources {
            for staged_resource in staged_resource_list.iter() {
                match staged_resource {
                    ResourceData::D2(data) => {
                        let params = gl_texture_params_from_texture_format(data.format);
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let pbos: Vec<GLPbo> = gl_configure_pbos(
                                &gl,
                                data.width as usize
                                    * data.height as usize
                                    * data.format.bytes_per(),
                            )
                            .iter()
                            .map(|pbo| GLPbo {
                                pbo: *pbo,
                                xoffset: 0,
                                yoffset: 0,
                                subwidth: 0,
                                subheight: 0,
                                width: data.width as GLsizei,
                                height: data.height as GLsizei,
                            })
                            .collect();
                            let pbos: [GLPbo; PBO_COUNT] =
                                copy_into_array(&pbos.as_slice()[..PBO_COUNT]);
                            let texture = gl::create_texture2d(
                                gl,
                                params.internal as i32,
                                data.width as i32,
                                data.height as i32,
                                params.format,
                                params.data_type,
                                None,
                            );
                            gl.generate_mipmap(gl::TEXTURE_2D);
                            GLResource {
                                texture,
                                pbos,
                                params,
                                target: gl::TEXTURE_2D,
                                time: 0.0,
                                resolution: Default::default(),
                                pbo_idx: 0,
                            }
                        });
                        resource.resolution = [
                            data.width as f32,
                            data.height as f32,
                            data.width as f32 / data.height as f32,
                        ];
                        if data.time >= 0.0 {
                            resource.time = data.time;
                        }
                        let pbo_idx = resource.pbo_idx;
                        let pbo_next_idx = (pbo_idx + 1) % PBO_COUNT;
                        resource.pbo_idx = pbo_next_idx;
                        // CPU->PBO upload
                        // Upload the staged data into the next pbo
                        {
                            let pbo = &mut resource.pbos[pbo_idx];
                            pbo.xoffset = data.xoffset as GLsizei;
                            pbo.yoffset = data.yoffset as GLsizei;
                            pbo.subwidth = data.subwidth as GLsizei;
                            pbo.subheight = data.subheight as GLsizei;
                        }
                        let pbo = resource.pbos[pbo_idx];
                        gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, pbo.pbo);
                        gl.buffer_sub_data_untyped(
                            gl::PIXEL_UNPACK_BUFFER,
                            0,
                            data.bytes.len() as isize,
                            data.bytes.as_ptr() as *const GLvoid,
                        );
                        gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, 0);
                        self.pbo_texture_unpack_list.push((pbo, *resource));
                    }
                    ResourceData::D3(data) => {
                        let params = gl_texture_params_from_texture_format(data.format);
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let texture = gl::create_texture3d(
                                gl,
                                params.internal as i32,
                                data.width as i32,
                                data.height as i32,
                                data.depth as i32,
                                params.format,
                                params.data_type,
                                None,
                            );
                            // TODO(jshrake): Is this necessary? Would we ever use a mipmap filter for 3D textures?
                            gl.generate_mipmap(gl::TEXTURE_3D);
                            GLResource {
                                texture,
                                params,
                                target: gl::TEXTURE_3D,
                                time: 0.0,
                                resolution: Default::default(),
                                pbos: Default::default(),
                                pbo_idx: 0,
                            }
                        });
                        resource.resolution =
                            [data.width as f32, data.height as f32, data.depth as f32];
                        if data.time >= 0.0 {
                            resource.time = data.time;
                        }
                        gl.bind_texture(resource.target, resource.texture);
                        gl.tex_sub_image_3d(
                            resource.target,
                            0,
                            0,
                            0,
                            0,
                            data.width as i32,
                            data.height as i32,
                            data.depth as i32,
                            params.format,
                            params.data_type,
                            &data.bytes,
                        );
                        // TODO(jshrake): Is this necessary? Would we ever use a mipmap filter for 3D textures?
                        gl.generate_mipmap(gl::TEXTURE_3D);
                    }
                    ResourceData::Cube(data) => {
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let texture = gl::create_texture(gl);
                            gl.generate_mipmap(gl::TEXTURE_CUBE_MAP);
                            GLResource {
                                texture,
                                target: gl::TEXTURE_CUBE_MAP,
                                resolution: Default::default(),
                                time: 0.0,
                                pbos: Default::default(),
                                pbo_idx: 0,
                                params: Default::default(),
                            }
                        });
                        gl.bind_texture(resource.target, resource.texture);
                        for (face, data) in data.iter() {
                            let params = gl_texture_params_from_texture_format(data.format);
                            let target = match face {
                                // Map the face enum to the appropriate GL enum
                                ResourceCubemapFace::Right => gl::TEXTURE_CUBE_MAP_POSITIVE_X,
                                ResourceCubemapFace::Left => gl::TEXTURE_CUBE_MAP_NEGATIVE_X,
                                ResourceCubemapFace::Top => gl::TEXTURE_CUBE_MAP_POSITIVE_Y,
                                ResourceCubemapFace::Bottom => gl::TEXTURE_CUBE_MAP_NEGATIVE_Y,
                                ResourceCubemapFace::Front => gl::TEXTURE_CUBE_MAP_POSITIVE_Z,
                                ResourceCubemapFace::Back => gl::TEXTURE_CUBE_MAP_NEGATIVE_Z,
                            };
                            gl.tex_image_2d(
                                target,
                                0,
                                params.internal as i32,
                                data.width as i32,
                                data.height as i32,
                                0,
                                params.format,
                                params.data_type,
                                Some(&data.bytes),
                            );
                        }
                        gl.generate_mipmap(gl::TEXTURE_CUBE_MAP);
                    }
                }
            }
        }
        self.staged_resources.clear();
    }
}

fn gl_wrap_from_config(wrap: &WrapConfig) -> GLenum {
    match wrap {
        WrapConfig::Clamp => gl::CLAMP_TO_EDGE,
        WrapConfig::Repeat => gl::REPEAT,
    }
}

fn gl_min_filter_from_config(filter: &FilterConfig) -> GLenum {
    match filter {
        FilterConfig::Linear => gl::LINEAR,
        FilterConfig::Nearest => gl::NEAREST,
        FilterConfig::Mipmap => gl::LINEAR_MIPMAP_LINEAR,
    }
}

fn gl_mag_filter_from_config(filter: &FilterConfig) -> GLenum {
    match filter {
        FilterConfig::Linear => gl::LINEAR,
        FilterConfig::Nearest => gl::NEAREST,
        FilterConfig::Mipmap => gl::LINEAR, // This is not a typo
    }
}

fn gl_texture_params_from_texture_format(data: TextureFormat) -> GLTextureParam {
    match data {
        TextureFormat::RU8 => GLTextureParam {
            data_type: gl::UNSIGNED_BYTE,
            format: gl::RED,
            internal: gl::RED,
        },
        TextureFormat::RF16 => GLTextureParam {
            data_type: gl::HALF_FLOAT,
            format: gl::RED,
            internal: gl::R16F,
        },
        TextureFormat::RF32 => GLTextureParam {
            data_type: gl::FLOAT,
            format: gl::RED,
            internal: gl::R32F,
        },
        TextureFormat::RGU8 => GLTextureParam {
            data_type: gl::UNSIGNED_BYTE,
            format: gl::RG,
            internal: gl::RG,
        },
        TextureFormat::RGF16 => GLTextureParam {
            data_type: gl::HALF_FLOAT,
            format: gl::RG,
            internal: gl::RG16F,
        },
        TextureFormat::RGF32 => GLTextureParam {
            data_type: gl::FLOAT,
            format: gl::RG,
            internal: gl::RG32F,
        },
        TextureFormat::RGBU8 => GLTextureParam {
            data_type: gl::UNSIGNED_BYTE,
            format: gl::RGB,
            internal: gl::RGB,
        },
        TextureFormat::RGBF16 => GLTextureParam {
            data_type: gl::HALF_FLOAT,
            format: gl::RGB,
            internal: gl::RGB16F,
        },
        TextureFormat::RGBF32 => GLTextureParam {
            data_type: gl::FLOAT,
            format: gl::RGB,
            internal: gl::RGB32F,
        },
        TextureFormat::RGBAU8 => GLTextureParam {
            data_type: gl::UNSIGNED_BYTE,
            format: gl::RGBA,
            internal: gl::RGBA,
        },
        TextureFormat::RGBAF16 => GLTextureParam {
            data_type: gl::HALF_FLOAT,
            format: gl::RGBA,
            internal: gl::RGBA16F,
        },
        TextureFormat::RGBAF32 => GLTextureParam {
            data_type: gl::FLOAT,
            format: gl::RGBA,
            internal: gl::RGBA32F,
        },
        TextureFormat::BGRU8 => GLTextureParam {
            data_type: gl::UNSIGNED_BYTE,
            format: gl::BGR,
            internal: gl::RGB,
        },
        TextureFormat::BGRF16 => GLTextureParam {
            data_type: gl::HALF_FLOAT,
            format: gl::BGR,
            internal: gl::RGB16F,
        },
        TextureFormat::BGRF32 => GLTextureParam {
            data_type: gl::FLOAT,
            format: gl::BGR,
            internal: gl::RGB32F,
        },
        TextureFormat::BGRAU8 => GLTextureParam {
            data_type: gl::UNSIGNED_BYTE,
            format: gl::BGRA,
            internal: gl::RGBA,
        },
        TextureFormat::BGRAF16 => GLTextureParam {
            data_type: gl::HALF_FLOAT,
            format: gl::BGRA,
            internal: gl::RGBA16F,
        },
        TextureFormat::BGRAF32 => GLTextureParam {
            data_type: gl::FLOAT,
            format: gl::BGRA,
            internal: gl::RGBA32F,
        },
    }
}

fn gl_blend_from_config(blend: &BlendFactorConfig) -> GLenum {
    match blend {
        BlendFactorConfig::DstAlpha => gl::DST_ALPHA,
        BlendFactorConfig::DstColor => gl::DST_COLOR,
        BlendFactorConfig::One => gl::ONE,
        BlendFactorConfig::OneMinusDstAlpha => gl::ONE_MINUS_DST_ALPHA,
        BlendFactorConfig::OneMinusDstColor => gl::ONE_MINUS_DST_COLOR,
        BlendFactorConfig::OneMinusSrcAlpha => gl::ONE_MINUS_SRC_ALPHA,
        BlendFactorConfig::OneMinusSrcColor => gl::ONE_MINUS_SRC_COLOR,
        BlendFactorConfig::SrcAlpha => gl::SRC_ALPHA,
        BlendFactorConfig::SrcColor => gl::SRC_COLOR,
        BlendFactorConfig::Zero => gl::ZERO,
    }
}

fn gl_depth_from_config(depth: &DepthFuncConfig) -> GLenum {
    match depth {
        DepthFuncConfig::Always => gl::ALWAYS,
        DepthFuncConfig::Equal => gl::EQUAL,
        DepthFuncConfig::GEqual => gl::GEQUAL,
        DepthFuncConfig::Greater => gl::GREATER,
        DepthFuncConfig::LEqual => gl::LEQUAL,
        DepthFuncConfig::Less => gl::LESS,
        DepthFuncConfig::Never => gl::NEVER,
        DepthFuncConfig::NotEqual => gl::NOTEQUAL,
    }
}

fn gl_configure_pbos(gl: &GLRc, data_len: usize) -> Vec<GLuint> {
    let pbos = gl.gen_buffers(PBO_COUNT as i32);
    for pbo in &pbos {
        gl.bind_buffer(gl::PIXEL_UNPACK_BUFFER, *pbo);
        gl.buffer_data_untyped(
            gl::PIXEL_UNPACK_BUFFER,
            data_len as isize,
            std::ptr::null(),
            gl::STREAM_DRAW,
        );
    }
    pbos
}

fn copy_into_array<A, T>(slice: &[T]) -> A
where
    A: Default + AsMut<[T]>,
    T: Copy,
{
    let mut a = Default::default();
    <A as AsMut<[T]>>::as_mut(&mut a).copy_from_slice(slice);
    a
}

fn hash_name_attachment(name: &str, attachment: usize) -> u64 {
    let mut s = DefaultHasher::new();
    name.hash(&mut s);
    attachment.hash(&mut s);
    s.finish()
}

unsafe fn to_slice<T: Sized, K>(p: &T) -> &[K] {
    ::std::slice::from_raw_parts((p as *const T) as *const K, ::std::mem::size_of::<T>())
}
