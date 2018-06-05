use std;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::default::Default;
use std::hash::{Hash, Hasher};

use config::*;
use error::{Error, ErrorKind, Result};
use failure::ResultExt;
use gl;
use gl::GLRc;
use gl::{GLenum, GLint, GLsizei, GLuint, GLvoid};
use resource::{ResourceCubemapFace, ResourceData};

#[derive(Debug)]
pub struct Effect {
    config: EffectConfig,
    string: String,
    version: String,
    header: String,
    footer: String,
    width: u32,
    height: u32,
    staged_resources: BTreeMap<u64, Vec<ResourceData>>,
    staged_buffer_data: BTreeMap<String, Vec<u8>>,
    pipeline: GLPipeline,
    resources: BTreeMap<u64, GLResource>,
    framebuffers: Vec<GLFramebuffer>,
    config_dirty: bool,
    pipeline_dirty: bool,
}

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

#[derive(Debug, Default, Copy, Clone)]
struct GLResource {
    target: GLenum,
    texture: GLuint,
    resolution: [f32; 3],
    time: f32,
}

#[derive(Debug, Default)]
struct GLFramebuffer {
    framebuffer: GLuint,
    depth_renderbuffer: GLuint,
    attachment_count: u32,
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
    clear_color: [f32; 4],
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

impl Default for Effect {
    fn default() -> Effect {
        Self {
            string: Default::default(),
            version: Default::default(),
            header: Default::default(),
            footer: Default::default(),
            config: Default::default(),
            staged_resources: Default::default(),
            staged_buffer_data: Default::default(),
            resources: Default::default(),
            pipeline: Default::default(),
            framebuffers: Default::default(),
            config_dirty: true,
            pipeline_dirty: true,
            width: 0,
            height: 0,
        }
    }
}

#[derive(Debug)]
struct GLTextureParam {
    internal: GLenum,
    format: GLenum,
    data_type: GLenum,
}

impl Effect {
    pub fn new(glsl_version: String, shader_header: String, shader_footer: String) -> Self {
        Self {
            version: glsl_version,
            header: shader_header,
            footer: shader_footer,
            ..Default::default()
        }
    }

    pub fn config(&self) -> &EffectConfig {
        &self.config
    }

    pub fn is_ok(&self) -> bool {
        // Only Ok if our pipeline has passes
        !self.pipeline.passes.is_empty()
    }

    pub fn stage_shader(&mut self, shader_string: String, shader_config: EffectConfig) -> bool {
        debug!(
            "[SHADER] Reloading (shader={:?}), (config={:?})",
            shader_string, shader_config
        );
        let mut new_commits = false;
        if shader_config != self.config {
            self.config_dirty = true;
            self.config = shader_config;
            self.staged_resources.clear();
            new_commits = true;
        }
        if shader_string != self.string {
            self.pipeline_dirty = true;
            self.string = shader_string;
            new_commits = true;
        }
        new_commits
    }

    pub fn stage_resource(&mut self, name: &str, resource: ResourceData) {
        debug!("[SHADER] Stage resource: {}={}", name, resource);
        let hashed_name = hash_resource_name(name);
        self.staged_resources
            .entry(hashed_name)
            .or_insert(Vec::new())
            .push(resource);
    }

    pub fn stage_state(&mut self, name: &str, state: EffectState) {
        debug!("[SHADER] Stage state: {}={:?}", name, state);
        self.stage_buffer_data(name, &state);
    }

    pub fn stage_buffer_data<T: Sized>(&mut self, name: &str, data: &T) {
        let bytes: &[u8] = unsafe { to_slice::<T, u8>(&data) };
        self.staged_buffer_data
            .insert(name.to_string(), Vec::from(bytes));
    }

    pub fn draw(&mut self, gl: &GLRc, width: u32, height: u32) -> Result<()> {
        // TODO(jshrake): Allow users to specify multisampling at cli
        //gl.enable(gl::MULTISAMPLE);
        // TODO(jshrake): Allow users to enable srgb
        //gl.enable(gl::FRAMEBUFFER_SRGB);
        gl.enable(gl::TEXTURE_CUBE_MAP_SEAMLESS);
        // Clear the default framebuffer initially to a weird color to signal an error
        gl.bind_framebuffer(gl::FRAMEBUFFER, 0);
        gl.viewport(0, 0, width as i32, height as i32);
        gl.clear_color(0.8, 0.2, 0.7, 1.0);
        gl.clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);
        if self.config.passes.is_empty() {
            return Ok(());
        }
        // Delete all resource and framebuffers on dirty config
        if self.config_dirty {
            self.config_dirty = false;
            self.delete_resources(gl);
            self.delete_framebuffers(gl);
        }
        // iterate through the staged resources and upload to the GPU
        for (hash, staged_resource_list) in &self.staged_resources {
            for staged_resource in staged_resource_list {
                match staged_resource {
                    ResourceData::D2(data) => {
                        let params = gl_texture_params_from_texture_format(&data.format);
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let texture = gl::create_texture2d(
                                gl,
                                params.internal as i32,
                                data.width as i32,
                                data.height as i32,
                                params.format,
                                params.data_type,
                                None,
                            );
                            GLResource {
                                target: gl::TEXTURE_2D,
                                texture: texture,
                                time: 0.0,
                                resolution: Default::default(),
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
                        gl.bind_texture(resource.target, resource.texture);
                        gl.tex_sub_image_2d(
                            resource.target,
                            0,
                            data.xoffset as i32,
                            data.yoffset as i32,
                            data.subwidth as i32,
                            data.subheight as i32,
                            params.format,
                            params.data_type,
                            &data.bytes,
                        );
                    }
                    ResourceData::D3(data) => {
                        let params = gl_texture_params_from_texture_format(&data.format);
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
                            GLResource {
                                target: gl::TEXTURE_3D,
                                texture: texture,
                                time: 0.0,
                                resolution: Default::default(),
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
                    }
                    ResourceData::Cube(data) => {
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let texture = gl::create_texture(gl);
                            GLResource {
                                target: gl::TEXTURE_CUBE_MAP,
                                texture: texture,
                                resolution: Default::default(),
                                time: 0.0,
                            }
                        });
                        gl.bind_texture(resource.target, resource.texture);
                        for (face, data) in data.iter() {
                            let params = gl_texture_params_from_texture_format(&data.format);
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
                    }
                }
            }
        }
        // clear now that all the staged resources are now uploaded to the card
        self.staged_resources.clear();

        // rebuild framebuffers on resize or when needs init
        let framebuffer_dirty = self.config.passes.len() != self.framebuffers.len();
        let resized = self.width != width || self.height != height;
        if framebuffer_dirty || resized {
            // reset dirty flags
            self.width = width;
            self.height = height;
            self.delete_framebuffers(gl);

            // Iterate through all pass_len - 1 passes and generate
            // a framebuffer and a requested number of color attachments
            let pass_len = self.config.passes.len();
            for (pass_index, pass_config) in
                self.config.passes.iter().take(pass_len - 1).enumerate()
            {
                let framebuffer = gl::create_framebuffer(gl);
                gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
                let mut color_attachments = Vec::new();
                let pass_width = pass_config.buffer.width.unwrap_or(self.width);
                let pass_height = pass_config.buffer.height.unwrap_or(self.height);
                let pass_resolution = [
                    pass_width as f32,
                    pass_height as f32,
                    pass_width as f32 / pass_height as f32,
                ];
                // zero out the allocated color attachments
                // Note that the attachments are RGBA32F -- 4 channels x 4 bytes
                let zero_data = vec![0 as u8; (pass_width * pass_height * 4 * 4) as usize];
                // Allocate twice as many attachments than we need,
                // so that we can ping pong the fbo color attachments
                // after drawing
                let (internal, format, data_type) = match pass_config.buffer.format {
                    BufferFormat::U8 => (gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE),
                    BufferFormat::F16 => (gl::RGBA16F, gl::RGBA, gl::HALF_FLOAT),
                    BufferFormat::F32 => (gl::RGBA32F, gl::RGBA, gl::FLOAT),
                };
                for attachment_index in 0..(2 * pass_config.buffer.attachments) {
                    let texture = gl::create_texture2d(
                        gl,
                        internal as i32,
                        pass_width as i32,
                        pass_height as i32,
                        format,
                        data_type,
                        Some(&zero_data),
                    );
                    let resource_hash = hash_pass_attachment(pass_index as u32, attachment_index);
                    color_attachments.push(resource_hash);
                    self.resources.insert(
                        resource_hash,
                        GLResource {
                            target: gl::TEXTURE_2D,
                            texture: texture,
                            resolution: pass_resolution,
                            time: 0.0,
                        },
                    );
                    // The attachments from [pass_config.attachments, 2*pass_config.attachments)
                    // are initially bound to the framebuffer
                    if attachment_index >= pass_config.buffer.attachments {
                        let resource = self.resources[&resource_hash];
                        gl.framebuffer_texture_2d(
                            gl::FRAMEBUFFER,
                            gl::COLOR_ATTACHMENT0
                                + (attachment_index - pass_config.buffer.attachments),
                            gl::TEXTURE_2D,
                            resource.texture,
                            0,
                        );
                    }
                }
                // create and attach a depth renderbuffer
                let depth = gl::create_renderbuffer(
                    gl,
                    gl::DEPTH_COMPONENT24,
                    pass_width as i32,
                    pass_height as i32,
                );
                gl::attach_renderbuffer_to_framebuffer(
                    gl,
                    framebuffer,
                    depth,
                    gl::DEPTH_ATTACHMENT,
                );
                assert!(gl::check_framebuffer_status(gl, framebuffer) == gl::FRAMEBUFFER_COMPLETE);
                self.framebuffers.push(GLFramebuffer {
                    framebuffer: framebuffer,
                    depth_renderbuffer: depth,
                    color_attachments: color_attachments,
                    attachment_count: pass_config.buffer.attachments,
                    resolution: pass_resolution,
                });
            }
            // the last pass gets a default framebuffer
            self.framebuffers.push(GLFramebuffer {
                framebuffer: 0,
                color_attachments: Default::default(),
                depth_renderbuffer: Default::default(),
                attachment_count: 0,
                resolution: [
                    self.width as f32,
                    self.height as f32,
                    self.width as f32 / self.height as f32,
                ],
            });
        }

        // rebuild the rendering pipeline.
        // - populates self.pipeline.passes
        if self.pipeline_dirty {
            self.pipeline_dirty = false;
            self.delete_pipeline(gl);
            self.pipeline.vertex_array_object = gl::create_vao(gl);
            // build the program for each pass
            for (pass_index, pass_config) in self.config.passes.iter().enumerate() {
                let uniform_sampler_strings =
                    glsl_pass_config_uniform_strings(pass_config, &self.config.resources)
                        .with_context(|_| ErrorKind::GLPass(pass_index))?;
                let vertex_shader_list = {
                    let mut list = Vec::new();
                    list.push(self.version.clone());
                    list.push(glsl_define("GRIM_VERTEX"));
                    list.push(glsl_define(&format!("GRIM_VERTEX_PASS_{}", pass_index)));
                    list.push(self.header.clone());
                    list.append(&mut uniform_sampler_strings.clone());
                    list.push("#line 1 0".to_string());
                    list.push(self.string.clone());
                    list.push(self.footer.clone());
                    list.join("\n")
                };
                let fragment_shader_list = {
                    let mut list = Vec::new();
                    list.push(self.version.clone());
                    list.push(glsl_define("GRIM_FRAGMENT"));
                    list.push(glsl_define(&format!("GRIM_FRAGMENT_PASS_{}", pass_index)));
                    list.push(glsl_define(&format!("GRIM_PASS_{}", pass_index)));
                    list.push(self.header.clone());
                    list.append(&mut uniform_sampler_strings.clone());
                    list.push("#line 1 0".to_string());
                    list.push(self.string.clone());
                    list.push(self.footer.clone());
                    list.join("\n")
                };
                let vertex_shader =
                    gl::create_shader(gl, gl::VERTEX_SHADER, &[vertex_shader_list.as_bytes()])
                        .map_err(|err| Error::glsl_vertex(err))
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
                    let (resource, wrap, filter) = match channel_config {
                        ChannelConfig::SimpleResource(ref name) => {
                            let resource_hash = hash_resource_name(name);
                            (resource_hash, gl::REPEAT, gl::LINEAR)
                        }
                        ChannelConfig::SimplePass(pass) => {
                            let resource_hash = hash_pass_attachment(*pass, 0);
                            (resource_hash, gl::CLAMP_TO_EDGE, gl::LINEAR)
                        }
                        ChannelConfig::SimplePassAttachment([pass, attachment]) => {
                            let resource_hash = hash_pass_attachment(*pass, *attachment);
                            (resource_hash, gl::CLAMP_TO_EDGE, gl::LINEAR)
                        }
                        ChannelConfig::CompletePass {
                            pass,
                            attachment,
                            wrap,
                            filter,
                        } => {
                            let resource_hash = hash_pass_attachment(*pass, *attachment);
                            let wrap = match wrap {
                                WrapConfig::Clamp => gl::CLAMP_TO_EDGE,
                                WrapConfig::Repeat => gl::REPEAT,
                            };
                            let filter = match filter {
                                FilterConfig::Linear => gl::LINEAR,
                                FilterConfig::Nearest => gl::NEAREST,
                            };
                            (resource_hash, wrap, filter)
                        }
                        ChannelConfig::CompleteResource {
                            ref resource,
                            ref wrap,
                            ref filter,
                        } => {
                            let resource_hash = hash_resource_name(&resource);
                            let wrap = match wrap {
                                &WrapConfig::Clamp => gl::CLAMP_TO_EDGE,
                                &WrapConfig::Repeat => gl::REPEAT,
                            };
                            let filter = match filter {
                                &FilterConfig::Linear => gl::LINEAR,
                                &FilterConfig::Nearest => gl::NEAREST,
                            };
                            (resource_hash, wrap, filter)
                        }
                    };
                    samplers.push(GLSampler {
                        resource: resource,
                        resolution_uniform_loc: resolution_uniform_loc,
                        playback_time_uniform_loc: playback_time_uniform_loc,
                        uniform_loc: uniform_loc,
                        mag_filter: filter,
                        min_filter: filter,
                        wrap_r: wrap,
                        wrap_s: wrap,
                        wrap_t: wrap,
                    });
                }
                // get per-pass uniforms for this program
                let resolution_uniform_loc = gl.get_uniform_location(program, "iResolution");
                let vertex_count_uniform_loc = gl.get_uniform_location(program, "iVertexCount");

                // specify draw state
                let draw_count = pass_config.draw.count;
                let (draw_mode, draw_count) = match pass_config.draw.mode {
                    DrawModeConfig::Triangles => (gl::TRIANGLES, 3 * draw_count),
                    DrawModeConfig::Points => (gl::POINTS, draw_count),
                    DrawModeConfig::Lines => (gl::LINES, 2 * draw_count),
                    DrawModeConfig::TriangleFan => (gl::TRIANGLE_FAN, 3 * draw_count),
                    DrawModeConfig::TriangleStrip => (gl::TRIANGLE_STRIP, 3 * draw_count),
                    DrawModeConfig::LineLoop => (gl::LINE_LOOP, 2 * draw_count),
                    DrawModeConfig::LineStrip => (gl::LINE_STRIP, 2 * draw_count),
                };
                fn config_blend_to_gl_blend(blend: &BlendFactorConfig) -> GLenum {
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
                };
                fn depth_func_to_gl_depth(depth: &DepthFuncConfig) -> GLenum {
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
                };
                let blend = pass_config.blend.as_ref().map(|blend| {
                    (
                        config_blend_to_gl_blend(&blend.src),
                        config_blend_to_gl_blend(&blend.dst),
                    )
                });
                let depth = pass_config
                    .depth
                    .as_ref()
                    .map(|depth| depth_func_to_gl_depth(&depth));
                self.pipeline.passes.push(GLPass {
                    // shader resources
                    vertex_shader: vertex_shader,
                    fragment_shader: fragment_shader,
                    program: program,
                    // uniforms
                    resolution_uniform_loc: resolution_uniform_loc,
                    vertex_count_uniform_loc: vertex_count_uniform_loc,
                    samplers: samplers,
                    // render state
                    draw_mode: draw_mode,
                    draw_count: draw_count as i32,
                    clear_color: pass_config.clear,
                    blend: blend,
                    depth: depth,
                })
            }

            // Now that we built all the pass programs, remember to connect the existing
            // uniform buffers to the programs
            for (index, (name, buffer)) in self.pipeline.uniform_buffers.iter().enumerate() {
                for pass in &self.pipeline.passes {
                    gl::connect_uniform_buffer(gl, *buffer, pass.program, name, index as u32);
                }
            }
        }

        // upload staged resources to the GPU
        for (uniform_name, data) in &self.staged_buffer_data {
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
                    let buffer = gl::create_uniform_buffer(gl);
                    for program in programs {
                        gl::connect_uniform_buffer(gl, buffer, program, uniform_name, index);
                    }
                    gl.bind_buffer(gl::UNIFORM_BUFFER, buffer);
                    // TODO(jshrake): Is DYNAMIC_DRAW the correct hint? Consider STREAM_DRAW
                    gl.buffer_data_untyped(
                        gl::UNIFORM_BUFFER,
                        data.len() as isize,
                        std::ptr::null(),
                        gl::DYNAMIC_DRAW,
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

        // draw
        gl.bind_vertex_array(self.pipeline.vertex_array_object);
        for (pass_index, pass) in self.pipeline.passes.iter().enumerate() {
            let framebuffer = &self.framebuffers[pass_index];
            gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer.framebuffer);
            gl.viewport(0, 0, self.width as i32, self.height as i32);
            gl.clear_color(
                pass.clear_color[0],
                pass.clear_color[1],
                pass.clear_color[2],
                pass.clear_color[3],
            );
            gl.clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT);

            gl.use_program(pass.program);

            // Set pass uniforms
            if pass.resolution_uniform_loc > -1 {
                gl.uniform_3fv(pass.resolution_uniform_loc, &framebuffer.resolution);
            }
            if pass.vertex_count_uniform_loc > -1 {
                gl.uniform_1i(pass.vertex_count_uniform_loc, pass.draw_count);
            }

            // Activate the appropriate textures and update sampler uniforms
            for (sampler_idx, ref sampler) in pass.samplers.iter().enumerate() {
                if sampler.uniform_loc < 0 {
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
            let attachment_count = framebuffer.attachment_count;
            // Specify draw buffers when attachments are present
            if attachment_count != 0 {
                let draw_buffers: Vec<GLenum> = (0..attachment_count)
                    .map(|i| gl::COLOR_ATTACHMENT0 + i)
                    .collect();
                gl.draw_buffers(&draw_buffers);
            }
            gl.draw_arrays(pass.draw_mode, 0, pass.draw_count);
            // ping pong the framebuffer textures
            for attachment_idx in 0..attachment_count {
                let mut ping_hash = framebuffer.color_attachments[attachment_idx as usize];
                let mut pong_hash =
                    framebuffer.color_attachments[(attachment_idx + attachment_count) as usize];
                let ping = self.resources[&ping_hash];
                let pong = self.resources[&pong_hash];
                self.resources.insert(ping_hash, pong);
                self.resources.insert(pong_hash, ping);
                // TODO(jshrake): Should we detatch the existing color attachments before
                // attaching?
                gl.framebuffer_texture_2d(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0 + attachment_idx,
                    gl::TEXTURE_2D,
                    0,
                    0,
                );
                gl.framebuffer_texture_2d(
                    gl::FRAMEBUFFER,
                    gl::COLOR_ATTACHMENT0 + attachment_idx,
                    gl::TEXTURE_2D,
                    ping.texture,
                    0,
                );
            }
        }
        Ok(())
    }

    // gl resource deletion

    fn delete_pipeline(&mut self, gl: &GLRc) {
        // delete existing pass gl resources
        gl.delete_vertex_arrays(&[self.pipeline.vertex_array_object]);
        for pass in &self.pipeline.passes {
            gl.delete_program(pass.program);
            gl.delete_shader(pass.vertex_shader);
            gl.delete_shader(pass.fragment_shader);
        }
        self.pipeline.passes.clear();
    }

    // Delete all resources except for the framebuffer color attachments
    fn delete_resources(&mut self, gl: &GLRc) {
        // Determine which resources belong to the framebuffer
        let mut framebuffer_attachment_set = BTreeSet::new();
        for framebuffer in &self.framebuffers {
            for attachment in &framebuffer.color_attachments {
                framebuffer_attachment_set.insert(attachment);
            }
        }
        // Delete all texture resources except the ones
        // marked as framebuffer attachments
        for (hash, resource) in &self.resources {
            if framebuffer_attachment_set.contains(hash) {
                continue;
            }
            gl.delete_textures(&[resource.texture]);
        }
        // Remove all resources except for the ones marked as framebuffer attachments
        self.resources = self
            .resources
            .iter()
            .filter(move |(hash, _)| framebuffer_attachment_set.contains(hash))
            .map(|(hash, resource)| (*hash, *resource))
            .collect();
    }

    fn delete_framebuffers(&mut self, gl: &GLRc) {
        // delete existing framebuffer gl resources
        for framebuffer in &self.framebuffers {
            for color_attachment in &framebuffer.color_attachments {
                if let Some(resource) = self.resources.remove(color_attachment) {
                    gl.delete_textures(&[resource.texture]);
                }
            }
            gl.delete_renderbuffers(&[framebuffer.depth_renderbuffer]);
            gl.delete_framebuffers(&[framebuffer.framebuffer]);
        }
        self.framebuffers.clear();
    }
}

fn glsl_define(name: &str) -> String {
    format!("#define {}", name)
}

fn glsl_pass_config_uniform_strings(
    pass_config: &PassConfig,
    resources_map: &BTreeMap<String, ResourceConfig>,
) -> Result<Vec<String>> {
    let mut list = Vec::new();
    for (name, channel_config) in pass_config.uniform_to_channel.iter() {
        list.append(&mut glsl_channel_config_uniform_strings(
            &name,
            &channel_config,
            resources_map,
        )?);
    }
    Ok(list)
}

fn glsl_channel_config_uniform_strings(
    name: &str,
    channel_config: &ChannelConfig,
    resources_map: &BTreeMap<String, ResourceConfig>,
) -> Result<Vec<String>> {
    let mut list = Vec::new();
    match channel_config {
        ChannelConfig::SimplePass(_)
        | ChannelConfig::SimplePassAttachment(_)
        | ChannelConfig::CompletePass { .. } => {
            list.append(&mut glsl_pass_uniform_strings(name));
        }
        ChannelConfig::SimpleResource(resource)
        | ChannelConfig::CompleteResource { resource, .. } => {
            let resource = resources_map
                .get(resource)
                .ok_or(Error::resource_not_found(
                    resource.as_str(),
                    name,
                    resources_map.iter().map(|(k, _)| k.as_str()).collect(),
                ))?;
            list.append(&mut glsl_resource_uniform_strings(name, resource));
        }
    }
    Ok(list)
}

fn glsl_pass_uniform_strings(name: &str) -> Vec<String> {
    let mut list = Vec::new();
    list.push(format!("uniform sampler2D {};", name));
    list.push(format!("uniform vec3 {}_Resolution;", name));
    list.push(format!("uniform vec3 {}_Time;", name));
    list
}

fn glsl_resource_uniform_strings(name: &str, config: &ResourceConfig) -> Vec<String> {
    let sampler = match config {
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
    };
    let mut list = Vec::new();
    list.push(format!("uniform {} {};", sampler, name));
    list.push(format!("uniform vec3 {}_Resolution;", name));
    list.push(format!("uniform vec3 {}_Time;", name));
    list
}

fn hash_resource_name(name: &str) -> u64 {
    let mut s = DefaultHasher::new();
    name.hash(&mut s);
    s.finish()
}

fn hash_pass_attachment(pass: u32, attachment: u32) -> u64 {
    let mut s = DefaultHasher::new();
    pass.hash(&mut s);
    attachment.hash(&mut s);
    s.finish()
}

unsafe fn to_slice<T: Sized, K>(p: &T) -> &[K] {
    ::std::slice::from_raw_parts((p as *const T) as *const K, ::std::mem::size_of::<T>())
}

fn gl_texture_params_from_texture_format(data: &TextureFormat) -> GLTextureParam {
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
            internal: gl::BGR,
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
            internal: gl::BGRA,
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
