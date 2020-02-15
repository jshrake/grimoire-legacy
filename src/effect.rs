use gl;
use gl::types::*;
use std;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::default::Default;
use std::ffi::{c_void, CString};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use crate::config::*;
use crate::error::{Error, ErrorKind, Result};
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
    pipeline: GLPipeline,
    default_framebuffer: Framebuffer,
    vertex_buffers: BTreeMap<u64, GLVertexBuffer>,
    resources: BTreeMap<u64, GLResource>,
    framebuffers: BTreeMap<String, Framebuffer>,
    pbo_texture_unpack_list: Vec<(GLPbo, GLResource)>,
    config_dirty: bool,
    pipeline_dirty: bool,
    first_draw: bool,
}

// The layout of this struct must match the layout of
// the uniform block GRIM_STATE defined in file header.gl::l
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
struct GLVertexBuffer {
    vbo: GLuint,
    mode: GLenum,
    count: GLsizei,
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
    depth_attachment: Option<GLuint>,
    color_attachments: Vec<u64>,
    resolution: [f32; 3],
}

#[derive(Debug, Clone)]
enum Framebuffer {
    Simple([GLFramebuffer; 1]),
    PingPong([GLFramebuffer; 2], RefCell<usize>),
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
    vbo: Option<GLVertexBuffer>,
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
    instance_count: GLsizei,
    clear_color: Option<[f32; 4]>,
    blend: Option<(GLenum, GLenum, GLenum, GLenum)>,
    clear_depth: Option<f32>,
    depth: Option<GLenum>,
    depth_write: bool,
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

impl Framebuffer {
    fn read_buffer(&self) -> &GLFramebuffer {
        match self {
            Framebuffer::Simple(f) => &f[0],
            Framebuffer::PingPong(f, i) => &f[1 - *i.borrow()],
        }
    }
    fn write_buffer(&self) -> &GLFramebuffer {
        match self {
            Framebuffer::Simple(f) => &f[0],
            Framebuffer::PingPong(f, i) => &f[*i.borrow()],
        }
    }
    fn all_buffers(&self) -> &[GLFramebuffer] {
        match self {
            Framebuffer::Simple(f) => &f[..],
            Framebuffer::PingPong(f, _) => &f[..],
        }
    }
    fn does_swap(&self) -> bool {
        match self {
            Framebuffer::PingPong(..) => true,
            _ => false,
        }
    }
    fn swap_read_write(&self) {
        match self {
            Framebuffer::Simple(_) => {}
            Framebuffer::PingPong(_, current) => {
                current.replace_with(|old| 1 - *old);
            }
        }
    }
}

impl<'a> Default for Effect<'a> {
    fn default() -> Self {
        Self {
            version: Default::default(),
            config: Default::default(),
            staged_resources: Default::default(),
            staged_uniform_buffer: Default::default(),
            resources: Default::default(),
            vertex_buffers: Default::default(),
            pipeline: Default::default(),
            framebuffers: Default::default(),
            pbo_texture_unpack_list: Default::default(),
            window_resolution: Default::default(),
            staged_uniform_1f: Default::default(),
            staged_uniform_2f: Default::default(),
            staged_uniform_3f: Default::default(),
            staged_uniform_4f: Default::default(),
            shader_cache: Default::default(),
            default_framebuffer: Framebuffer::Simple([GLFramebuffer {
                framebuffer: 0,
                resolution: [0.0, 0.0, 0.0],
                ..Default::default()
            }]),
            config_dirty: true,
            pipeline_dirty: true,
            first_draw: true,
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

    pub fn snapshot(
        &self,
        buffer: &mut Vec<u8>,
        window_width: i32,
        window_height: i32,
    ) -> Result<()> {
        let format = gl::RGB;
        let pixel_type = gl::UNSIGNED_BYTE;
        unsafe {
            gl::PixelStorei(gl::PACK_ALIGNMENT, 1);
            gl::ReadPixels(
                0,
                0,
                window_width,
                window_height,
                format,
                pixel_type,
                buffer.as_mut_ptr() as *mut c_void,
            );
        }
        Ok(())
    }

    pub fn draw(&mut self, window_width: f32, window_height: f32) -> Result<()> {
        if self.first_draw {
            self.first_draw = false;
            // TODO(jshrake): Consider adding the following to the config: enables: ["multisample, framebuffer_srgb"]
            //gl::Enable(gl::MULTISAMPLE);
            //gl::Enable(gl::FRAMEBUFFER_SRGB);
            unsafe {
                gl::Enable(gl::TEXTURE_CUBE_MAP_SEAMLESS);
                gl::Enable(gl::PROGRAM_POINT_SIZE);
            }
        }

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
        match self.default_framebuffer {
            Framebuffer::Simple(ref mut fbo) => fbo[0].resolution = self.window_resolution,
            _ => unreachable!("default framebuffer is always simple"),
        }

        // delete non framebuffer resources on dirty config
        if resources_need_init {
            self.gpu_delete_non_buffer_resources();
        }

        // build or rebuild framebuffers on resize
        if framebuffers_need_init || window_resized {
            let instant = Instant::now();
            self.gpu_delete_buffer_resources();
            self.gpu_init_framebuffers();
            info!(
                "[DRAW] Initializing framebuffer objects took {:?}",
                instant.elapsed()
            );
        }

        // build or rebuild the rendering pipeline
        if pipeline_need_init {
            let instant = Instant::now();
            self.gpu_delete_pipeline_resources();
            self.gpu_stage_resources();
            self.gpu_init_pipeline()?;
            info!(
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
        self.gpu_stage_resources();
        self.gpu_stage_buffer_data();
        let last_call_duration = instant.elapsed();
        if last_call_duration > Duration::from_millis(1) {
            warn!(
                "[DRAW] GPU resource + uniform staging took {:?}",
                last_call_duration
            );
        }

        let instant = Instant::now();
        self.gpu_draw()?;
        let draw_duration = instant.elapsed();
        if draw_duration > Duration::from_millis(5) {
            warn!("[DRAW] Draw took {:?}", draw_duration);
        }

        let instant = Instant::now();
        self.gpu_pbo_to_texture_transfer();
        let last_call_duration = instant.elapsed();
        if last_call_duration > Duration::from_millis(1) {
            warn!(
                "[DRAW] PBO to texture transfer took {:?}",
                last_call_duration
            );
        }
        Ok(())
    }

    fn framebuffer_for_pass(&self, pass: &PassConfig) -> &Framebuffer {
        if let Some(ref buffer_name) = pass.buffer {
            self.framebuffers
                .get(buffer_name)
                .unwrap_or(&self.default_framebuffer)
        } else {
            &self.default_framebuffer
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

    fn gpu_delete_non_buffer_resources(&mut self) {
        let mut framebuffer_attachment_set = BTreeSet::new();

        for framebuffer in self.framebuffers.values() {
            for fbo in framebuffer.all_buffers() {
                for attachment in &fbo.color_attachments {
                    framebuffer_attachment_set.insert(attachment);
                }
            }
        }
        // Delete all GL texture resources except the ones
        // marked as framebuffer attachments
        for (hash, resource) in &self.resources {
            if framebuffer_attachment_set.contains(hash) {
                continue;
            }
            unsafe {
                gl::DeleteTextures(1, [resource.texture].as_ptr());
            }
            for pbo in &resource.pbos {
                unsafe {
                    gl::DeleteBuffers(1, [pbo.pbo].as_ptr());
                }
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

    fn gpu_delete_buffer_resources(&mut self) {
        // Free current framebuffer resources
        for framebuffer in self.framebuffers.values() {
            // NOTE: Each framebuffer has several color attachments. We need to remove them from the
            // resources array, and delete them from GL
            for fbo in framebuffer.all_buffers() {
                for color_attachment in &fbo.color_attachments {
                    if let Some(resource) = self.resources.remove(color_attachment) {
                        unsafe {
                            gl::DeleteTextures(1, [resource.texture].as_ptr());
                        }
                    } else {
                        unreachable!(format!(
                            "Unable to remove collor attachment {} from framebuffer {:?}",
                            color_attachment, fbo
                        ));
                    }
                }
                if let Some(depth_attachment) = fbo.depth_attachment {
                    unsafe {
                        gl::DeleteTextures(1, [depth_attachment].as_ptr());
                    }
                }
                unsafe {
                    gl::DeleteFramebuffers(1, [fbo.framebuffer].as_ptr());
                }
            }
        }
        self.framebuffers.clear();
    }

    fn gpu_delete_pipeline_resources(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, [self.pipeline.vertex_array_object].as_ptr());
        }
        for pass in &self.pipeline.passes {
            unsafe {
                gl::DeleteProgram(pass.program);
                gl::DeleteShader(pass.vertex_shader);
                gl::DeleteShader(pass.fragment_shader);
            }
        }
        self.pipeline.passes.clear();
    }

    fn gpu_pbo_to_texture_transfer(&mut self) {
        // PBO->Texture unpack
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0);
        }
        for (pbo, resource) in &self.pbo_texture_unpack_list {
            unsafe {
                gl::BindTexture(resource.target, resource.texture);
                gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, pbo.pbo);
                gl::TexSubImage2D(
                    resource.target,
                    0,
                    pbo.xoffset as i32,
                    pbo.yoffset as i32,
                    pbo.subwidth as i32,
                    pbo.subheight as i32,
                    resource.params.format,
                    resource.params.data_type,
                    0 as *const c_void,
                );
                gl::GenerateMipmap(gl::TEXTURE_2D);
            }
        }
        unsafe {
            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
        }
        self.pbo_texture_unpack_list.clear();
    }

    fn gpu_draw(&mut self) -> Result<()> {
        unsafe {
            gl::BindVertexArray(self.pipeline.vertex_array_object);
            for (pass_idx, pass) in self.pipeline.passes.iter().enumerate() {
                let pass_config = &self.config.passes[pass_idx];
                // Don't draw this pass if it's marked as disabled
                if pass_config.disable {
                    continue;
                }
                for _ in 0..pass_config.loop_count {
                    // Find the framebuffer corresponding to the pass configuration
                    // The lookup can fail if the user supplies a bad configuration,
                    // like a typo in the buffer value
                    let framebuffer = self.framebuffer_for_pass(&pass_config);
                    gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer.write_buffer().framebuffer);
                    // Set the viewport to match the framebuffer resolution
                    gl::Viewport(
                        0,
                        0,
                        framebuffer.write_buffer().resolution[0] as GLint,
                        framebuffer.write_buffer().resolution[1] as GLint,
                    );
                    let mut clear_flag = None;
                    if let Some(clear_color) = pass.clear_color {
                        gl::ClearColor(
                            clear_color[0],
                            clear_color[1],
                            clear_color[2],
                            clear_color[3],
                        );
                        clear_flag = Some(gl::COLOR_BUFFER_BIT);
                    }
                    if let Some(clear_depth) = pass.clear_depth {
                        gl::ClearDepth(clear_depth.into());
                        clear_flag = clear_flag.map_or(Some(gl::DEPTH_BUFFER_BIT), |flag| {
                            Some(flag | gl::DEPTH_BUFFER_BIT)
                        });
                    }
                    if let Some(clear_flag) = clear_flag {
                        gl::Clear(clear_flag);
                    }

                    // Bind the program for this pass
                    gl::UseProgram(pass.program);

                    // Set per-pass non-sampler uniforms
                    if pass.resolution_uniform_loc > -1 {
                        let buf = &framebuffer.write_buffer().resolution;
                        gl::Uniform3fv(
                            pass.resolution_uniform_loc,
                            (buf.len() / 3) as GLsizei,
                            buf.as_ptr(),
                        );
                    }
                    if pass.vertex_count_uniform_loc > -1 {
                        gl::Uniform1i(pass.vertex_count_uniform_loc, pass.draw_count);
                    }

                    // Set staged uniform data
                    // TODO: cache get_uniform_location calls
                    for (name, data) in &self.staged_uniform_1f {
                        let loc = get_uniform_location(pass.program, &name);
                        gl::Uniform1f(loc, *data);
                    }
                    for (name, data) in &self.staged_uniform_2f {
                        let loc = get_uniform_location(pass.program, &name);
                        gl::Uniform2fv(loc, (data.len() / 2) as GLsizei, data.as_ptr());
                    }
                    for (name, data) in &self.staged_uniform_3f {
                        let loc = get_uniform_location(pass.program, &name);
                        gl::Uniform3fv(loc, (data.len() / 3) as GLsizei, data.as_ptr());
                    }
                    for (name, data) in &self.staged_uniform_4f {
                        let loc = get_uniform_location(pass.program, &name);
                        gl::Uniform4fv(loc, (data.len() / 4) as GLsizei, data.as_ptr());
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
                            gl::ActiveTexture(gl::TEXTURE0 + sampler_idx as u32);
                            gl::BindTexture(resource.target, resource.texture);
                            gl::TexParameteri(
                                resource.target,
                                gl::TEXTURE_WRAP_S,
                                sampler.wrap_s as i32,
                            );
                            gl::TexParameteri(
                                resource.target,
                                gl::TEXTURE_WRAP_T,
                                sampler.wrap_t as i32,
                            );
                            if resource.target == gl::TEXTURE_3D
                                || resource.target == gl::TEXTURE_CUBE_MAP
                            {
                                gl::TexParameteri(
                                    resource.target,
                                    gl::TEXTURE_WRAP_R,
                                    sampler.wrap_r as i32,
                                );
                            }
                            gl::TexParameteri(
                                resource.target,
                                gl::TEXTURE_MIN_FILTER,
                                sampler.min_filter as i32,
                            );
                            gl::TexParameteri(
                                resource.target,
                                gl::TEXTURE_MAG_FILTER,
                                sampler.mag_filter as i32,
                            );
                            gl::Uniform1i(sampler.uniform_loc, sampler_idx as i32);
                            // bind resolution & playback time uniforms
                            //info!("pass: {:?}, sampler: {:?}, {:?}", pass_idx, sampler_idx, sampler);
                            if sampler.resolution_uniform_loc > -1 {
                                gl::Uniform3fv(
                                    sampler.resolution_uniform_loc as i32,
                                    (resource.resolution.len() / 3) as GLsizei,
                                    resource.resolution.as_ptr(),
                                );
                            }
                            if sampler.playback_time_uniform_loc > -1 {
                                gl::Uniform1f(
                                    sampler.playback_time_uniform_loc as i32,
                                    resource.time,
                                );
                            }
                        }
                    }
                    // Set the blend state
                    if let Some((src_rgb, dst_rgb, src_a, dst_a)) = pass.blend {
                        gl::Enable(gl::BLEND);
                        gl::BlendFuncSeparate(src_rgb, dst_rgb, src_a, dst_a);
                    } else {
                        gl::Disable(gl::BLEND);
                    }
                    // Set the depth test state
                    if let Some(depth_func) = pass.depth {
                        gl::Enable(gl::DEPTH_TEST);
                        gl::DepthFunc(depth_func);
                    } else {
                        gl::Disable(gl::DEPTH_TEST);
                    }
                    gl::DepthMask(pass.depth_write as GLboolean);
                    // Draw!
                    if let Some(vbo) = pass.vbo {
                        let position_str = CString::new("position").unwrap();
                        let normal_str = CString::new("normal").unwrap();
                        let position_loc =
                            gl::GetAttribLocation(pass.program, position_str.as_ptr());
                        let normal_loc = gl::GetAttribLocation(pass.program, normal_str.as_ptr());
                        let defined_position = position_loc >= 0;
                        let defined_normal = normal_loc >= 0;
                        let stride = 6 * std::mem::size_of::<f32>() as i32;
                        let position_offset = 0;
                        let normal_offset = 3 * std::mem::size_of::<f32>() as u32;
                        gl::BindBuffer(gl::ARRAY_BUFFER, vbo.vbo);
                        if defined_position {
                            gl::EnableVertexAttribArray(position_loc as u32);
                            gl::VertexAttribPointer(
                                position_loc as u32,
                                3,
                                gl::FLOAT,
                                false as GLboolean,
                                stride,
                                position_offset as *const GLvoid,
                            );
                        }
                        if defined_normal {
                            gl::EnableVertexAttribArray(normal_loc as u32);
                            gl::VertexAttribPointer(
                                normal_loc as u32,
                                3,
                                gl::FLOAT,
                                false as GLboolean,
                                stride,
                                normal_offset as *const GLvoid,
                            );
                        }
                        gl::DrawArraysInstanced(
                            pass.draw_mode,
                            0,
                            pass.draw_count,
                            pass.instance_count,
                        );
                        if defined_position {
                            gl::DisableVertexAttribArray(position_loc as u32);
                        }
                        if defined_normal {
                            gl::DisableVertexAttribArray(normal_loc as u32);
                        }
                        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                    } else {
                        gl::DrawArrays(pass.draw_mode, 0, pass.draw_count);
                    }
                    // if this framebuffer swaps the read and write buffers, then
                    // swap the read + write color attachments in the self.resources map
                    if framebuffer.does_swap() {
                        let mut swap_color_attachment_resources = Vec::new();
                        for i in 0..framebuffer.write_buffer().color_attachments.len() {
                            let write_hash = framebuffer.write_buffer().color_attachments[i];
                            let read_hash = framebuffer.read_buffer().color_attachments[i];
                            swap_color_attachment_resources.push((write_hash, read_hash));
                        }
                        framebuffer.swap_read_write();
                        for (write_hash, read_hash) in swap_color_attachment_resources {
                            let write = self.resources[&write_hash];
                            let read = self.resources[&read_hash];
                            self.resources.insert(write_hash, read);
                            self.resources.insert(read_hash, write);
                        }
                    }
                    // Unbind our program to avoid spurious nvidia warnings in apitrace
                    gl::UseProgram(0);
                    // Unbind our textures to make debugging cleaner
                    for (sampler_idx, ref sampler) in pass.samplers.iter().enumerate() {
                        if sampler.uniform_loc < 0 {
                            // Note that this is not necessarily an error. The user may simply not be
                            // referencing some uniform, so the GLSL compiler compiles it out and
                            // we get an invalid unifrom loc. That's fine -- just keep moving on
                            continue;
                        }
                        if let Some(resource) = self.resources.get(&sampler.resource) {
                            gl::ActiveTexture(gl::TEXTURE0 + sampler_idx as u32);
                            gl::GenerateMipmap(gl::TEXTURE_2D);
                            gl::BindTexture(resource.target, 0);
                        }
                    }
                }
            }
            self.staged_uniform_1f.clear();
            self.staged_uniform_2f.clear();
            self.staged_uniform_3f.clear();
            self.staged_uniform_4f.clear();
        }
        Ok(())
    }

    fn gpu_init_pipeline(&mut self) -> Result<()> {
        self.pipeline.vertex_array_object = create_vao();
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
            let vertex_source = self
                .shader_cache
                .get(vertex_path)
                .expect("vertex path not found in shader_cache");
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
            let vertex_shader = create_shader(gl::VERTEX_SHADER, &[vertex_shader_list.as_bytes()])
                .map_err(|err| Error::glsl_vertex(&err, &vertex_path.clone()))
                .with_context(|_| ErrorKind::GLPass(pass_index))?;
            assert!(vertex_shader != 0);

            let fragment_path = &pass_config.fragment;
            let fragment_source = self
                .shader_cache
                .get(fragment_path)
                .expect("fragment path not found in shader_cache");
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
            let fragment_shader =
                create_shader(gl::FRAGMENT_SHADER, &[fragment_shader_list.as_bytes()])
                    .map_err(|err| {
                        unsafe {
                            gl::DeleteShader(vertex_shader);
                        }
                        Error::glsl_fragment(err, fragment_path.clone())
                    })
                    .with_context(|_| ErrorKind::GLPass(pass_index))?;
            assert!(fragment_shader != 0);

            let geometry_shader = {
                if let Some(geometry_path) = &pass_config.geometry {
                    let geometry_source = self
                        .shader_cache
                        .get(geometry_path)
                        .expect("fragment path not found in shader_cache");
                    let geometry_shader_list = {
                        let mut list = Vec::new();
                        list.push(self.version.clone());
                        list.push(include_str!("./shadertoy_uniforms.glsl").to_string());
                        list.append(&mut uniform_strings.clone());
                        list.append(&mut uniform_sampler_strings.clone());
                        list.push("#line 1 0".to_string());
                        list.push(geometry_source.clone());
                        list.join("\n")
                    };
                    let geometry_shader =
                        create_shader(gl::GEOMETRY_SHADER, &[geometry_shader_list.as_bytes()])
                            .map_err(|err| {
                                unsafe {
                                    gl::DeleteShader(vertex_shader);
                                    gl::DeleteShader(fragment_shader);
                                }
                                Error::glsl_fragment(err, geometry_path.clone())
                            })
                            .with_context(|_| ErrorKind::GLPass(pass_index))?;
                    Some(geometry_shader)
                } else {
                    None
                }
            };
            let program = create_program(vertex_shader, fragment_shader, geometry_shader)
                .map_err(|err| {
                    unsafe {
                        gl::DeleteShader(vertex_shader);
                        gl::DeleteShader(fragment_shader);
                    }
                    Error::glsl_program(err, vertex_path.clone(), fragment_path.clone())
                })
                .with_context(|_| ErrorKind::GLPass(pass_index))?;
            assert!(program != 0);

            // build the samplers used to draw this pass
            let mut samplers = Vec::new();
            for (uniform_name, channel_config) in &pass_config.uniform_to_channel {
                let uniform_loc = get_uniform_location(program, &uniform_name);
                let resolution_uniform_name = format!("{}_Resolution", &uniform_name);
                let resolution_uniform_loc =
                    get_uniform_location(program, &resolution_uniform_name);
                let playback_time_uniform_name = format!("{}_Time", &uniform_name);
                let playback_time_uniform_loc =
                    get_uniform_location(program, &playback_time_uniform_name);
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
                let sampler = GLSampler {
                    resource,
                    resolution_uniform_loc,
                    playback_time_uniform_loc,
                    uniform_loc,
                    mag_filter,
                    min_filter,
                    wrap_r: wrap,
                    wrap_s: wrap,
                    wrap_t: wrap,
                };
                if uniform_loc < 0 && resolution_uniform_loc > -1 {
                    info!("WARNING: resolution uniform \"{}\" referenced in pass {} but sampler uniform \"{}\" is not!", resolution_uniform_name, pass_index, uniform_name);
                }
                if uniform_loc < 0 && playback_time_uniform_loc > -1 {
                    info!("WARNING: playback time uniform \"{}\" referenced in pass {} but sampler uniform \"{}\" is not!", playback_time_uniform_name, pass_index, uniform_name);
                }
                samplers.push(sampler);
            }
            // get per-pass uniforms for this program
            let resolution_uniform_loc = get_uniform_location(program, "iResolution");
            let vertex_count_uniform_loc = get_uniform_location(program, "iVertexCount");

            // specify draw state
            let model_name = match pass_config.draw {
                DrawConfig::Model(ref m) => Some(&m.model),
                DrawConfig::Raw(_) => None,
            };
            let vbo = model_name
                .map(|n| hash_name_attachment(&n, 0))
                .and_then(|h| self.vertex_buffers.get(&h).map(|vbo| *vbo));

            let (draw_mode, draw_count, instance_count) = match &pass_config.draw {
                DrawConfig::Raw(config) => {
                    let draw_count = config.count as i32;
                    let (draw_mode, draw_count) = match config.mode {
                        DrawModeConfig::Triangles => (gl::TRIANGLES, 3 * draw_count),
                        DrawModeConfig::Points => (gl::POINTS, draw_count),
                        DrawModeConfig::Lines => (gl::LINES, 2 * draw_count),
                        DrawModeConfig::TriangleFan => (gl::TRIANGLE_FAN, 3 * draw_count),
                        DrawModeConfig::TriangleStrip => (gl::TRIANGLE_STRIP, 3 + (draw_count - 1)),
                        DrawModeConfig::LineLoop => (gl::LINE_LOOP, 2 * draw_count),
                        DrawModeConfig::LineStrip => (gl::LINE_STRIP, 2 * draw_count),
                    };
                    (draw_mode, draw_count, 0)
                }
                DrawConfig::Model(config) => match vbo {
                    Some(vbo) => (vbo.mode, vbo.count, config.count as i32),
                    None => (gl::TRIANGLES, 0, 0),
                },
            };
            let blend = match pass_config.blend {
                None => None,
                Some(ref blend) => match blend {
                    BlendConfig::Simple(c) => Some((
                        gl_blend_from_config(&c.src),
                        gl_blend_from_config(&c.dst),
                        gl_blend_from_config(&c.src),
                        gl_blend_from_config(&c.dst),
                    )),
                    BlendConfig::Separable(c) => Some((
                        gl_blend_from_config(&c.src_rgb),
                        gl_blend_from_config(&c.dst_rgb),
                        gl_blend_from_config(&c.src_alpha),
                        gl_blend_from_config(&c.dst_alpha),
                    )),
                },
            };
            let depth = pass_config
                .depth
                .as_ref()
                .map(|depth| gl_depth_from_config(&depth.func()));
            let (clear_color, clear_depth) = match pass_config.clear {
                None => (None, None),
                Some(ref clear) => match clear {
                    ClearConfig::Color(a) => (Some(*a), None),
                    ClearConfig::ColorDepth(a) => (Some([a[0], a[1], a[2], a[3]]), Some(a[4])),
                    ClearConfig::Complete { color, depth } => (*color, *depth),
                },
            };
            let depth_write = pass_config
                .depth
                .map(|depth| match depth {
                    DepthTestConfig::Simple(_) => true,
                    DepthTestConfig::Complete { write, .. } => write,
                })
                .unwrap_or(true);
            self.pipeline.passes.push(GLPass {
                vbo,
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
                instance_count,
                blend,
                depth,
                depth_write,
                clear_color,
                clear_depth,
            })
        }
        // Now that we built all the pass programs, remember to connect the existing
        // uniform buffers to the programs
        for (index, (name, buffer)) in self.pipeline.uniform_buffers.iter().enumerate() {
            for pass in &self.pipeline.passes {
                connect_uniform_buffer(*buffer, pass.program, name, index as u32);
            }
        }
        Ok(())
    }

    fn gpu_stage_buffer_data(&mut self) {
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
                    let buffer = create_buffer();
                    for program in programs {
                        connect_uniform_buffer(buffer, program, uniform_name, index);
                    }
                    unsafe {
                        gl::BindBuffer(gl::UNIFORM_BUFFER, buffer);
                        gl::BufferData(
                            gl::UNIFORM_BUFFER,
                            data.len() as isize,
                            std::ptr::null(),
                            gl::STREAM_DRAW,
                        );
                    }
                    buffer
                });
            unsafe {
                gl::BindBuffer(gl::UNIFORM_BUFFER, *buffer);
                gl::BufferSubData(
                    gl::UNIFORM_BUFFER,
                    0,
                    data.len() as isize,
                    data.as_ptr() as *const GLvoid,
                );
            }
        }
    }

    fn gpu_init_framebuffers(&mut self) {
        // build a map of buffer names to if it's a feedback buffer
        let mut framebuffer_kind_map = BTreeMap::new();
        for (resource_name, _) in &self.config.resources {
            framebuffer_kind_map.insert(resource_name, false);
        }
        for pass_config in &self.config.passes {
            let is_feedback = pass_config.is_feedback();
            if let Some(buffer_name) = &pass_config.buffer {
                framebuffer_kind_map
                    .entry(&buffer_name)
                    .and_modify(|e| *e = *e || is_feedback)
                    .or_insert(is_feedback);
            }
        }

        for (resource_name, resource) in &self.config.resources {
            if let ResourceConfig::Buffer(buffer) = resource {
                let is_feedback_pass = *framebuffer_kind_map.get(&resource_name).unwrap_or(&false);
                let buffers_to_make = if is_feedback_pass { 2 } else { 1 };
                // Setup 2 Framebuffers so that we can swap between them on subsequent draws
                let mut buffers = Vec::with_capacity(buffers_to_make);
                for i in 0..buffers_to_make {
                    let fbo = create_framebuffer();
                    unsafe {
                        gl::BindFramebuffer(gl::FRAMEBUFFER, fbo);
                    }
                    let mut color_attachments = Vec::new();
                    let width = buffer.width.unwrap_or(self.window_resolution[0] as u32);
                    let height = buffer.height.unwrap_or(self.window_resolution[1] as u32);
                    let scale = buffer.scale.unwrap_or(1.0);
                    // apply scale, then take the floor
                    let width = (scale * width as f32) as u32;
                    let height = (scale * height as f32) as u32;
                    let resolution = [width as f32, height as f32, width as f32 / height as f32];
                    let attachment_count = buffer.attachment_count();
                    for attachment_index in 0..attachment_count {
                        let attachment_format = match buffer.buffer {
                            BufferFormatConfig::Dumb(_) => BufferFormat::F32,
                            BufferFormatConfig::Simple(f) => f,
                            BufferFormatConfig::Complete(ref v) => v[attachment_index],
                        };
                        // calculate parameters for gl::texture creation based on config
                        let (internal, format, data_type, bytes_per) =
                            match (&buffer.components, &attachment_format) {
                                // 1 component
                                (1, BufferFormat::U8) => (gl::RED, gl::RED, gl::UNSIGNED_BYTE, 1),
                                (1, BufferFormat::F16) => (gl::R16F, gl::RED, gl::HALF_FLOAT, 2),
                                (1, BufferFormat::F32) => (gl::R32F, gl::RED, gl::FLOAT, 4),
                                // 2 components
                                (2, BufferFormat::U8) => (gl::RG, gl::RG, gl::UNSIGNED_BYTE, 1),
                                (2, BufferFormat::F16) => (gl::RG16F, gl::RG, gl::HALF_FLOAT, 2),
                                (2, BufferFormat::F32) => (gl::RG32F, gl::RG, gl::FLOAT, 4),
                                // 3 components
                                (3, BufferFormat::U8) => (gl::RGB, gl::RGB, gl::UNSIGNED_BYTE, 1),
                                (3, BufferFormat::F16) => (gl::RGB16F, gl::RGB, gl::HALF_FLOAT, 2),
                                (3, BufferFormat::F32) => (gl::RGB32F, gl::RGB, gl::FLOAT, 4),
                                // 4 components
                                (4, BufferFormat::U8) => (gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE, 1),
                                (4, BufferFormat::F16) => {
                                    (gl::RGBA16F, gl::RGBA, gl::HALF_FLOAT, 2)
                                }
                                (4, BufferFormat::F32) => (gl::RGBA32F, gl::RGBA, gl::FLOAT, 4),
                                // components specified is outside the range [0,4], default to 4
                                (_, BufferFormat::U8) => (gl::RGBA, gl::RGBA, gl::UNSIGNED_BYTE, 1),
                                (_, BufferFormat::F16) => {
                                    (gl::RGBA16F, gl::RGBA, gl::HALF_FLOAT, 2)
                                }
                                (_, BufferFormat::F32) => (gl::RGBA32F, gl::RGBA, gl::FLOAT, 4),
                            };
                        // zero out the allocated color attachments
                        // Note that the attachments are 4 channels x bytes_per
                        let zero_data = vec![
                            0 as u8;
                            (width * height * buffer.components as u32 * bytes_per)
                                as usize
                        ];
                        let texture = create_texture2d(
                            internal as i32,
                            width as i32,
                            height as i32,
                            format,
                            data_type,
                            Some(&zero_data),
                        );
                        unsafe {
                            gl::GenerateMipmap(gl::TEXTURE_2D);
                            gl::FramebufferTexture2D(
                                gl::FRAMEBUFFER,
                                gl::COLOR_ATTACHMENT0 + attachment_index as u32,
                                gl::TEXTURE_2D,
                                texture,
                                0,
                            );
                        }
                        // Offset by buffer.attachments + 1 to make room for the
                        // depth attachment texture
                        let hash = hash_name_attachment(
                            resource_name,
                            attachment_index + i * (buffer.attachment_count() + 1),
                        );
                        color_attachments.push(hash);
                        let resource = GLResource {
                            target: gl::TEXTURE_2D,
                            texture,
                            resolution,
                            time: Default::default(),
                            pbos: Default::default(),
                            pbo_idx: Default::default(),
                            params: Default::default(),
                        };
                        self.resources.insert(hash, resource);
                    } // color attachments

                    // Create and attach optional depth texture
                    let need_depth_buffer = match buffer.depth {
                        BufferDepthConfig::Simple(result) => result,
                        _ => true,
                    };
                    let depth_attachment = if need_depth_buffer {
                        let depth_internal = match buffer.depth {
                            BufferDepthConfig::Simple(true) => gl::DEPTH_COMPONENT24,
                            BufferDepthConfig::Complete(BufferDepthFormat::U16) => {
                                gl::DEPTH_COMPONENT16
                            }
                            BufferDepthConfig::Complete(BufferDepthFormat::U24) => {
                                gl::DEPTH_COMPONENT24
                            }
                            BufferDepthConfig::Complete(BufferDepthFormat::U32) => {
                                gl::DEPTH_COMPONENT32
                            }
                            BufferDepthConfig::Complete(BufferDepthFormat::F32) => {
                                gl::DEPTH_COMPONENT32F
                            }
                            _ => unreachable!(),
                        };
                        // TODO(jshrake): Do we need to zero-out the depth buffer?
                        let depth_texture = create_texture2d(
                            depth_internal as i32,
                            width as i32,
                            height as i32,
                            gl::DEPTH_COMPONENT,
                            gl::FLOAT,
                            None,
                        );
                        unsafe {
                            gl::FramebufferTexture2D(
                                gl::FRAMEBUFFER,
                                gl::DEPTH_ATTACHMENT,
                                gl::TEXTURE_2D,
                                depth_texture,
                                0,
                            );
                        }
                        let hash = hash_name_attachment(
                            resource_name,
                            buffer.attachment_count() + i * (buffer.attachment_count() + 1),
                        );
                        let resource = GLResource {
                            target: gl::TEXTURE_2D,
                            texture: depth_texture,
                            resolution,
                            time: Default::default(),
                            pbos: Default::default(),
                            pbo_idx: Default::default(),
                            params: Default::default(),
                        };
                        self.resources.insert(hash, resource);
                        Some(depth_texture)
                    } else {
                        None
                    };

                    // Call draw_buffers if we have attachments
                    // Assuming this is not the default framebuffer, we always
                    // have at least one color attachment
                    let draw_buffers: Vec<GLenum> = (0..attachment_count)
                        .map(|i| gl::COLOR_ATTACHMENT0 + i as u32)
                        .collect();
                    if !draw_buffers.is_empty() {
                        unsafe {
                            gl::DrawBuffers(draw_buffers.len() as GLsizei, draw_buffers.as_ptr());
                        }
                    }
                    // This should never fail
                    let fbo_status = check_framebuffer_status(fbo);
                    assert!(fbo_status == gl::FRAMEBUFFER_COMPLETE);
                    if fbo_status != gl::FRAMEBUFFER_COMPLETE {
                        info!("error creating framebuffer. status: {:?}", fbo_status);
                    }
                    buffers.push(GLFramebuffer {
                        framebuffer: fbo,
                        depth_attachment,
                        color_attachments,
                        resolution,
                    });
                }
                let framebuffer = match is_feedback_pass {
                    true => {
                        assert_eq!(buffers.len(), 2);
                        let mut l = [Default::default(), Default::default()];
                        for (i, b) in buffers.into_iter().enumerate() {
                            l[i] = b;
                        }
                        Framebuffer::PingPong(l, RefCell::new(1))
                    }
                    _ => {
                        assert_eq!(buffers.len(), 1);
                        let mut l = [Default::default()];
                        for (i, b) in buffers.into_iter().enumerate() {
                            l[i] = b;
                        }
                        Framebuffer::Simple(l)
                    }
                };
                self.framebuffers.insert(resource_name.clone(), framebuffer);
            }
        }
    }

    fn gpu_stage_resources(&mut self) {
        for (hash, staged_resource_list) in &self.staged_resources {
            for staged_resource in staged_resource_list.iter() {
                match staged_resource {
                    ResourceData::Geometry(data) => {
                        let byte_len =
                            (data.buffer.len() as isize) * (std::mem::size_of::<f32>() as isize);
                        let vbo = self.vertex_buffers.entry(*hash).or_insert_with(|| {
                            let vbo = create_buffer();
                            let mode = gl::TRIANGLES;
                            // The buffer is interleaved with position (vec3) + normal (vec3) data (/2)
                            let count = ((data.buffer.len() / 2) / 3) as GLsizei;
                            unsafe {
                                gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
                                gl::BufferData(
                                    gl::ARRAY_BUFFER,
                                    byte_len,
                                    std::ptr::null() as *const GLvoid,
                                    gl::DYNAMIC_DRAW,
                                );
                                gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                            }
                            GLVertexBuffer { vbo, mode, count }
                        });
                        unsafe {
                            gl::BindBuffer(gl::ARRAY_BUFFER, vbo.vbo);
                            gl::BufferSubData(
                                gl::ARRAY_BUFFER,
                                0,
                                byte_len,
                                data.buffer.as_ptr() as *const GLvoid,
                            );
                            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
                        }
                    }
                    ResourceData::D2(data) => {
                        let params = gl_texture_params_from_texture_format(data.format);
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let pbos: Vec<GLPbo> = gl_configure_pbos(
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
                            let texture = create_texture2d(
                                params.internal as i32,
                                data.width as i32,
                                data.height as i32,
                                params.format,
                                params.data_type,
                                None,
                            );
                            unsafe {
                                gl::GenerateMipmap(gl::TEXTURE_2D);
                            }
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
                        unsafe {
                            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, pbo.pbo);
                            gl::BufferSubData(
                                gl::PIXEL_UNPACK_BUFFER,
                                0,
                                data.bytes.len() as isize,
                                data.bytes.as_ptr() as *const GLvoid,
                            );
                            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, 0);
                        }
                        self.pbo_texture_unpack_list.push((pbo, *resource));
                    }
                    ResourceData::D3(data) => {
                        let params = gl_texture_params_from_texture_format(data.format);
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let texture = create_texture3d(
                                params.internal as i32,
                                data.width as i32,
                                data.height as i32,
                                data.depth as i32,
                                params.format,
                                params.data_type,
                                None,
                            );
                            // TODO(jshrake): Is this necessary? Would we ever use a mipmap filter for 3D textures?
                            unsafe {
                                gl::GenerateMipmap(gl::TEXTURE_3D);
                            }
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
                        unsafe {
                            gl::BindTexture(resource.target, resource.texture);
                            gl::TexSubImage3D(
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
                                data.bytes.as_ptr() as *const c_void,
                            );
                            // TODO(jshrake): Is this necessary? Would we ever use a mipmap filter for 3D textures?
                            gl::GenerateMipmap(gl::TEXTURE_3D);
                        }
                    }
                    ResourceData::Cube(data) => {
                        let resource = self.resources.entry(*hash).or_insert_with(|| {
                            let texture = create_texture();
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
                        unsafe {
                            gl::BindTexture(resource.target, resource.texture);
                        }
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
                            unsafe {
                                gl::TexImage2D(
                                    target,
                                    0,
                                    params.internal as i32,
                                    data.width as i32,
                                    data.height as i32,
                                    0,
                                    params.format,
                                    params.data_type,
                                    data.bytes.as_ptr() as *const c_void,
                                );
                            }
                        }
                        unsafe {
                            gl::GenerateMipmap(gl::TEXTURE_CUBE_MAP);
                        }
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

fn gl_configure_pbos(data_len: usize) -> Vec<GLuint> {
    let pbos = create_buffers(PBO_COUNT as i32);
    for pbo in &pbos {
        unsafe {
            gl::BindBuffer(gl::PIXEL_UNPACK_BUFFER, *pbo);
            gl::BufferData(
                gl::PIXEL_UNPACK_BUFFER,
                data_len as isize,
                std::ptr::null(),
                gl::STREAM_DRAW,
            );
        }
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

#[allow(dead_code)]
pub fn create_buffer() -> GLuint {
    let mut result = [0 as GLuint];
    unsafe {
        gl::GenBuffers(1, result.as_mut_ptr());
    }
    result[0]
}

pub fn create_buffers(n: GLsizei) -> Vec<GLuint> {
    let mut result = vec![0 as GLuint; n as usize];
    unsafe {
        gl::GenBuffers(n, result.as_mut_ptr());
    }
    result
}

#[allow(dead_code)]
pub fn connect_uniform_buffer(buffer: GLuint, program: GLuint, name: &str, bind_index: GLuint) {
    let c_string = CString::new(name).unwrap();
    unsafe {
        let block_index = gl::GetUniformBlockIndex(program, c_string.as_ptr());
        if block_index < 2_000_000 {
            gl::UniformBlockBinding(program, block_index, bind_index);
            gl::BindBuffer(gl::UNIFORM_BUFFER, buffer);
            gl::BindBufferBase(gl::UNIFORM_BUFFER, bind_index, buffer);
        }
    }
}

#[allow(dead_code)]
pub fn create_shader(
    shader_type: GLenum,
    strings: &[&[u8]],
) -> std::result::Result<GLuint, String> {
    unsafe {
        let shader = gl::CreateShader(shader_type);
        //assert!(shader != 0);
        let pointers: Vec<*const u8> = strings.iter().map(|string| (*string).as_ptr()).collect();
        let lengths: Vec<GLint> = strings.iter().map(|string| string.len() as GLint).collect();
        gl::ShaderSource(
            shader,
            pointers.len() as GLsizei,
            pointers.as_ptr() as *const *const GLchar,
            lengths.as_ptr(),
        );
        gl::CompileShader(shader);
        let compiled = {
            let mut compiled: [i32; 1] = [0];
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, compiled.as_mut_ptr());
            compiled[0]
        };
        if compiled == 0 {
            let log = get_shader_info_log(shader);
            gl::DeleteShader(shader);
            return Err(log.trim().to_string());
        }
        Ok(shader)
    }
}

#[allow(dead_code)]
pub fn create_program(
    vs: GLuint,
    fs: GLuint,
    gs: Option<GLuint>,
) -> std::result::Result<GLuint, String> {
    unsafe {
        let program = gl::CreateProgram();
        assert!(program != 0);
        gl::AttachShader(program, vs);
        if let Some(gs) = gs {
            gl::AttachShader(program, gs);
        }
        gl::AttachShader(program, fs);
        gl::LinkProgram(program);
        let linked = {
            let mut linked = 0;
            gl::GetProgramiv(program, gl::LINK_STATUS, &mut linked);
            linked
        };
        gl::DetachShader(program, vs);
        if let Some(gs) = gs {
            gl::DetachShader(program, gs);
        }
        gl::DetachShader(program, fs);
        if linked == 0 {
            let log = get_program_info_log(program);
            gl::DeleteProgram(program);
            return Err(log.trim().to_string());
        }
        Ok(program)
    }
}

fn get_program_info_log(program: GLuint) -> String {
    let mut max_len = [0];
    unsafe {
        gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, max_len.as_mut_ptr());
    }
    if max_len[0] == 0 {
        return String::new();
    }
    let mut result = vec![0u8; max_len[0] as usize];
    let mut result_len = 0 as GLsizei;
    unsafe {
        gl::GetProgramInfoLog(
            program,
            max_len[0] as GLsizei,
            &mut result_len,
            result.as_mut_ptr() as *mut GLchar,
        );
    }
    result.truncate(if result_len > 0 {
        result_len as usize
    } else {
        0
    });
    String::from_utf8(result).unwrap()
}

fn get_shader_info_log(shader: GLuint) -> String {
    let mut max_len = [0];
    unsafe {
        gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, max_len.as_mut_ptr());
    }
    if max_len[0] == 0 {
        return String::new();
    }
    let mut result = vec![0u8; max_len[0] as usize];
    let mut result_len = 0 as GLsizei;
    unsafe {
        gl::GetShaderInfoLog(
            shader,
            max_len[0] as GLsizei,
            &mut result_len,
            result.as_mut_ptr() as *mut GLchar,
        );
    }
    result.truncate(if result_len > 0 {
        result_len as usize
    } else {
        0
    });
    String::from_utf8(result).unwrap()
}

fn get_uniform_location(program: GLuint, name: &str) -> GLint {
    let name = CString::new(name).unwrap();
    unsafe { gl::GetUniformLocation(program, name.as_ptr()) }
}

#[allow(dead_code)]
pub fn create_texture() -> GLuint {
    let mut result = [0 as GLuint];
    unsafe {
        gl::GenTextures(1, result.as_mut_ptr());
    }
    result[0]
}

#[allow(dead_code)]
pub fn create_texture3d(
    internalformat: GLint,
    width: GLsizei,
    height: GLsizei,
    depth: GLsizei,
    format: GLenum,
    data_type: GLenum,
    opt_data: Option<&[u8]>,
) -> GLuint {
    let texture = create_texture();
    unsafe {
        gl::BindTexture(gl::TEXTURE_3D, texture);
        // NOTE(jshrake): This next line is very important
        // default UNPACK_ALIGNMENT is 4
        gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
        match opt_data {
            Some(data) => {
                gl::TexImage3D(
                    gl::TEXTURE_3D,
                    0,
                    internalformat,
                    width,
                    height,
                    depth,
                    0,
                    format,
                    data_type,
                    data.as_ptr() as *const GLvoid,
                );
            }
            None => {
                gl::TexImage3D(
                    gl::TEXTURE_3D,
                    0,
                    internalformat,
                    width,
                    height,
                    depth,
                    0,
                    format,
                    data_type,
                    std::ptr::null(),
                );
            }
        }
    }
    texture
}

#[allow(dead_code)]
pub fn create_texture2d(
    internalformat: GLint,
    width: GLsizei,
    height: GLsizei,
    format: GLenum,
    data_type: GLenum,
    opt_data: Option<&[u8]>,
) -> GLuint {
    let texture = create_texture();
    unsafe {
        gl::BindTexture(gl::TEXTURE_2D, texture);
        match opt_data {
            Some(data) => {
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    internalformat,
                    width,
                    height,
                    0,
                    format,
                    data_type,
                    data.as_ptr() as *const GLvoid,
                );
            }
            None => {
                gl::TexImage2D(
                    gl::TEXTURE_2D,
                    0,
                    internalformat,
                    width,
                    height,
                    0,
                    format,
                    data_type,
                    std::ptr::null(),
                );
            }
        }
    }
    texture
}

#[allow(dead_code)]
pub fn create_renderbuffer(internalformat: GLenum, width: GLsizei, height: GLsizei) -> GLuint {
    let mut result = [0 as GLuint];
    unsafe {
        gl::GenRenderbuffers(1, result.as_mut_ptr());
        gl::BindRenderbuffer(gl::RENDERBUFFER, result[0]);
        gl::RenderbufferStorage(gl::RENDERBUFFER, internalformat, width, height);
    }
    result[0]
}

#[allow(dead_code)]
pub fn create_framebuffer() -> GLuint {
    let mut result = [0 as GLuint];
    unsafe {
        gl::GenFramebuffers(1, result.as_mut_ptr());
    }
    result[0]
}

#[allow(dead_code)]
pub fn attach_texture_to_framebuffer(framebuffer: GLuint, texture: GLuint, attachment: GLenum) {
    unsafe {
        gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
        gl::FramebufferTexture2D(gl::FRAMEBUFFER, attachment, gl::TEXTURE_2D, texture, 0);
    }
}

#[allow(dead_code)]
pub fn attach_renderbuffer_to_framebuffer(
    framebuffer: GLuint,
    renderbuffer: GLuint,
    attachment: GLenum,
) {
    unsafe {
        gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
        gl::FramebufferRenderbuffer(gl::FRAMEBUFFER, attachment, gl::RENDERBUFFER, renderbuffer);
    }
}

#[allow(dead_code)]
pub fn check_framebuffer_status(framebuffer: GLuint) -> GLenum {
    unsafe {
        gl::BindFramebuffer(gl::FRAMEBUFFER, framebuffer);
        gl::CheckFramebufferStatus(gl::FRAMEBUFFER)
    }
}

#[allow(dead_code)]
pub fn create_vao() -> GLuint {
    let mut result = [0 as GLuint];
    unsafe {
        gl::GenVertexArrays(1, result.as_mut_ptr());
    }
    result[0]
}

#[allow(dead_code)]
pub fn create_pbo() -> GLuint {
    let mut result = [0 as GLuint];
    unsafe {
        gl::GenVertexArrays(1, result.as_mut_ptr());
    }
    result[0]
}
