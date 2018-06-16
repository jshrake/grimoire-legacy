use std;

pub use gleam::gl::Gl;
pub use gleam::gl::GlesFns;
pub use gleam::gl::*;
pub type GLRc = std::rc::Rc<Gl>;
use gleam::gl;

#[allow(dead_code)]
pub fn create_buffer(gl: &GLRc) -> GLuint {
    let buffers = gl.gen_buffers(1);
    *buffers.first().expect("gl.gen_buffers failed")
}

#[allow(dead_code)]
pub fn connect_uniform_buffer(
    gl: &GLRc,
    buffer: GLuint,
    program: GLuint,
    name: &str,
    bind_index: GLuint,
) {
    let block_index = gl.get_uniform_block_index(program, name);
    if block_index < 2_000_000 {
        gl.uniform_block_binding(program, block_index, bind_index);
        gl.bind_buffer(gl::UNIFORM_BUFFER, buffer);
        gl.bind_buffer_base(gl::UNIFORM_BUFFER, bind_index, buffer);
    }
}

#[allow(dead_code)]
pub fn create_shader(gl: &GLRc, shader_type: GLenum, source: &[&[u8]]) -> Result<GLuint, String> {
    let shader = gl.create_shader(shader_type);
    //assert!(shader != 0);
    gl.shader_source(shader, source);
    gl.compile_shader(shader);
    let compiled = gl.get_shader_iv(shader, gl::COMPILE_STATUS);
    if compiled == 0 {
        let log = gl.get_shader_info_log(shader);
        gl.delete_shader(shader);
        return Err(log.trim().to_string());
    }
    Ok(shader)
}

#[allow(dead_code)]
pub fn create_program(gl: &GLRc, vs: GLuint, fs: GLuint) -> Result<GLuint, String> {
    let program = gl.create_program();
    assert!(program != 0);
    gl.attach_shader(program, vs);
    gl.attach_shader(program, fs);
    gl.link_program(program);
    let linked = gl.get_program_iv(program, gl::LINK_STATUS);
    if linked == 0 {
        let log = gl.get_program_info_log(program);
        gl.detach_shader(program, vs);
        gl.detach_shader(program, fs);
        gl.delete_program(program);
        return Err(log.trim().to_string());
    }
    gl.detach_shader(program, vs);
    gl.detach_shader(program, fs);
    Ok(program)
}

#[allow(dead_code)]
pub fn create_texture(gl: &GLRc) -> GLuint {
    let textures = gl.gen_textures(1);
    *textures.first().expect("gl.gen_textures failed")
}

#[allow(dead_code)]
pub fn create_texture3d(
    gl: &GLRc,
    internalformat: GLint,
    width: GLsizei,
    height: GLsizei,
    depth: GLsizei,
    format: GLenum,
    data_type: GLenum,
    opt_data: Option<&[u8]>,
) -> GLuint {
    let texture = create_texture(gl);
    gl.bind_texture(gl::TEXTURE_3D, texture);
    // NOTE(jshrake): This next line is very important
    // default UNPACK_ALIGNMENT is 4
    gl.pixel_store_i(gl::UNPACK_ALIGNMENT, 1);
    gl.tex_image_3d(
        gl::TEXTURE_3D,
        0,
        internalformat,
        width,
        height,
        depth,
        0,
        format,
        data_type,
        opt_data,
    );
    texture
}

#[allow(dead_code)]
pub fn create_texture2d(
    gl: &GLRc,
    internalformat: GLint,
    width: GLsizei,
    height: GLsizei,
    format: GLenum,
    data_type: GLenum,
    opt_data: Option<&[u8]>,
) -> GLuint {
    let texture = create_texture(gl);
    gl.bind_texture(gl::TEXTURE_2D, texture);
    gl.tex_image_2d(
        gl::TEXTURE_2D,
        0,
        internalformat,
        width,
        height,
        0,
        format,
        data_type,
        opt_data,
    );
    texture
}

#[allow(dead_code)]
pub fn create_renderbuffer(
    gl: &GLRc,
    internalformat: GLenum,
    width: GLsizei,
    height: GLsizei,
) -> GLuint {
    let renderbuffers = gl.gen_renderbuffers(1);
    let renderbuffer = *renderbuffers.first().expect("gl.gen_renderbuffers failed");
    gl.bind_renderbuffer(gl::RENDERBUFFER, renderbuffer);
    gl.renderbuffer_storage(gl::RENDERBUFFER, internalformat, width, height);
    renderbuffer
}

#[allow(dead_code)]
pub fn create_framebuffer(gl: &GLRc) -> GLuint {
    let framebuffers = gl.gen_framebuffers(1);
    *framebuffers.first().expect("gl.gen_framebuffers failed")
}

#[allow(dead_code)]
pub fn attach_texture_to_framebuffer(
    gl: &GLRc,
    framebuffer: GLuint,
    texture: GLuint,
    attachment: GLenum,
) {
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
    gl.framebuffer_texture_2d(gl::FRAMEBUFFER, attachment, gl::TEXTURE_2D, texture, 0);
}

#[allow(dead_code)]
pub fn attach_renderbuffer_to_framebuffer(
    gl: &GLRc,
    framebuffer: GLuint,
    renderbuffer: GLuint,
    attachment: GLenum,
) {
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
    gl.framebuffer_renderbuffer(gl::FRAMEBUFFER, attachment, gl::RENDERBUFFER, renderbuffer);
}

#[allow(dead_code)]
pub fn check_framebuffer_status(gl: &GLRc, framebuffer: GLuint) -> GLenum {
    gl.bind_framebuffer(gl::FRAMEBUFFER, framebuffer);
    gl.check_frame_buffer_status(gl::FRAMEBUFFER)
}

#[allow(dead_code)]
pub fn create_vao(gl: &GLRc) -> GLuint {
    let vaos = gl.gen_vertex_arrays(1);
    *vaos.first().expect("gl.gen_vertex_arrays failed")
}

#[allow(dead_code)]
pub fn create_pbo(gl: &GLRc) -> GLuint {
    let vaos = gl.gen_vertex_arrays(1);
    *vaos.first().expect("gl.gen_vertex_arrays failed")
}
