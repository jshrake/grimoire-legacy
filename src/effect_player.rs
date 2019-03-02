use std::cell::RefCell;
use std::path::Path;
use std::time::Duration;

use chrono::prelude::*;
use crate::config::EffectConfig;
use crate::config::ResourceConfig;
use crate::effect::{Effect, EffectState};
use crate::error::{Error, ErrorKind, Result};
use failure::ResultExt;
use crate::file_stream::FileStream;
use glsl_include::Context as GlslContex;
use crate::mouse::Mouse;
use crate::platform::Platform;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use crate::stream::{ResourceStream, Stream};

pub struct EffectPlayer<'a> {
    shader_src_stream: FileStream,
    shader_string: String,
    shader_include_streams: Vec<(String, FileStream)>,
    resource_streams: Vec<(String, ResourceStream)>,
    shader: Effect<'a>,
    playing: bool,
    time: Duration,
    frame: u32,
    mouse: Mouse,
    ctx: RefCell<GlslContex<'a>>,
}

impl<'a> EffectPlayer<'a> {
    pub fn new(
        glsl_src_path: &Path,
        glsl_include_paths: Vec<(String, String)>,
        glsl_version: String,
        shader_header: String,
        shader_footer: String,
    ) -> Result<Self> {
        let mut shader_include_streams = Vec::new();
        for (read_path, include_path) in glsl_include_paths {
            shader_include_streams.push((include_path, FileStream::new(Path::new(&read_path))?));
        }
        Ok(Self {
            shader_include_streams,
            shader_src_stream: FileStream::new(glsl_src_path)?,
            shader: Effect::new(glsl_version, shader_header, shader_footer),
            shader_string: Default::default(),
            resource_streams: Default::default(),
            mouse: Default::default(),
            playing: Default::default(),
            time: Default::default(),
            frame: Default::default(),
            ctx: RefCell::new(GlslContex::new()),
        })
    }

    pub fn play(&mut self) -> Result<()> {
        debug!("[PLAYBACK] PLAY");
        self.playing = true;
        for (_, ref mut stream) in &mut self.resource_streams {
            stream.play()?;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        debug!("[PLAYBACK] PAUSE");
        self.playing = false;
        for (_, ref mut stream) in &mut self.resource_streams {
            stream.pause()?;
        }
        Ok(())
    }

    pub fn toggle_play(&mut self) -> Result<()> {
        if self.playing {
            self.pause()?;
        } else {
            self.play()?;
        }
        Ok(())
    }

    pub fn restart(&mut self) -> Result<()> {
        debug!("[PLAYBACK] RESTART");
        self.time = Default::default();
        self.frame = Default::default();
        for (_, ref mut stream) in &mut self.resource_streams {
            stream.restart()?;
        }
        Ok(())
    }

    pub fn step_forward(&mut self, dt: Duration) {
        self.time += dt;
        self.frame += 1;
    }

    pub fn step_backward(&mut self, dt: Duration) {
        if self.frame > 0 {
            self.time -= dt;
            self.frame -= 1;
        }
    }

    pub fn tick(&mut self, platform: &mut Platform) -> Result<bool> {
        // handle ESC to close the app
        for event in platform.events.poll_iter() {
            match event {
                Event::Window { win_event, .. } => match win_event {
                    _ => {}
                },
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => {
                    return Ok(true);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F1),
                    ..
                } => self.toggle_play()?,
                Event::KeyDown {
                    keycode: Some(Keycode::F2),
                    ..
                } => {
                    self.pause()?;
                    self.step_backward(platform.time_delta);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F3),
                    ..
                } => {
                    self.pause()?;
                    self.step_forward(platform.time_delta);
                }
                Event::KeyDown {
                    keycode: Some(Keycode::F4),
                    ..
                } => {
                    self.restart()?;
                }
                _ => {}
            }
        }
        // check for shader changes and rebuild the shader source
        let mut shader_did_change = false;
        for (include_path, stream) in self.shader_include_streams.iter_mut() {
            if let Some(shader_bytes) = stream.try_recv()? {
                let mut ctx = self.ctx.borrow_mut();
                let shader_string: String = String::from_utf8(shader_bytes)
                    .map_err(|err| Error::from_utf8(stream.path(), err))?;
                ctx.include(include_path.to_string(), shader_string);
                shader_did_change = true;
            }
        }
        if let Some(shader_bytes) = self.shader_src_stream.try_recv()? {
            let shader_string: String = String::from_utf8(shader_bytes)
                .map_err(|err| Error::from_utf8(self.shader_src_stream.path(), err))?;
            self.shader_string = shader_string;
            shader_did_change = true;
        }

        // If the shader file changed, load it!
        if shader_did_change {
            let shader_string = self
                .ctx
                .borrow()
                .expand(self.shader_string.to_string())
                .expect("ack");
            let shader_config = EffectConfig::from_comment_block_in_str(&shader_string)?;
            // If config is dirty, clear and repopulate the resource streams
            let config_dirty = *self.shader.config() != shader_config;
            if config_dirty {
                self.resource_streams.clear();
                for (name, resource_config) in &shader_config.resources {
                    let stream = ResourceStream::new(name, resource_config)
                        .with_context(|_| ErrorKind::BadResourceConfig(name.to_string()))?;
                    self.resource_streams.push((name.clone(), stream));
                }
                for (name, input) in &shader_config.resources {
                    match input {
                        ResourceConfig::UniformFloat(u) => {
                            self.shader.stage_uniform1f(name.clone(), u.uniform);
                        }
                        ResourceConfig::UniformVec2(u) => {
                            self.shader.stage_uniform2f(name.clone(), u.uniform);
                        }
                        ResourceConfig::UniformVec3(u) => {
                            self.shader.stage_uniform3f(name.clone(), u.uniform);
                        }
                        ResourceConfig::UniformVec4(u) => {
                            self.shader.stage_uniform4f(name.clone(), u.uniform);
                        }
                        _ => continue,
                    };
                }
            }
            self.shader.stage_shader(shader_string, shader_config)?;
        };
        // resource streaming
        for (ref name, ref mut stream) in &mut self.resource_streams.iter_mut() {
            match stream.tick(platform) {
                Ok(ref mut resources) => {
                    while let Some(resource) = resources.next() {
                        self.shader.stage_resource(&name, resource);
                    }
                }
                Err(err) => {
                    return Err(err);
                }
            };
        }
        // effect state
        let state = {
            let mouse = {
                let mouse_state = platform.events.mouse_state();
                let mouse_buttons = mouse_state.pressed_mouse_buttons().collect();
                let mouse_x = mouse_state.x() as u32;
                let mouse_y = mouse_state.y() as u32;
                let mouse_y = if mouse_y < platform.window_resolution.1 {
                    platform.window_resolution.1 - mouse_y
                } else {
                    0
                };
                self.mouse.update(mouse_buttons, mouse_x, mouse_y)
            };
            fn duration_to_float_secs(duration: Duration) -> f32 {
                duration.as_secs() as f32 + duration.subsec_nanos() as f32 * 1e-9
            }
            let time = duration_to_float_secs(self.time);
            let time_delta = duration_to_float_secs(platform.time_delta);
            let local_date: DateTime<Local> = Local::now();
            let year = local_date.year() as f32;
            let month = local_date.month() as f32;
            let day = local_date.day() as f32;
            let sec = local_date.hour() as f32 * 60.0 * 60.0
                + local_date.minute() as f32 * 60.0
                + local_date.second() as f32;
            let date = [year, month, day, sec];
            let frame = self.frame as f32;
            let frame_rate = 1.0 / time_delta;
            let window_resolution = [
                platform.window_resolution.0 as f32,
                platform.window_resolution.1 as f32,
                platform.window_resolution.0 as f32 / platform.window_resolution.1 as f32,
            ];
            EffectState {
                time,
                time_delta,
                date,
                frame,
                frame_rate,
                mouse,
                window_resolution,
            }
        };
        self.shader.stage_state("GRIM_STATE", &state);
        self.shader.draw(
            &platform.gl,
            state.window_resolution[0],
            state.window_resolution[1],
        )?;
        if self.playing {
            self.step_forward(platform.time_delta);
        }

        Ok(false)
    }
}
