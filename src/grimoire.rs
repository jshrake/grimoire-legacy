use std::path::Path;
use std::time::Duration;

use chrono::prelude::*;
use config::EffectConfig;
use effect::{Effect, EffectState};
use error::{Error, ErrorKind, Result};
use failure::ResultExt;
use file_stream::FileStream;
use mouse::Mouse;
use platform::Platform;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use stream::{ResourceStream, Stream};

pub struct Grimoire {
    shader_stream: FileStream,
    resource_streams: Vec<(String, ResourceStream)>,
    shader: Effect,
    playing: bool,
    time: Duration,
    frame: u32,
    mouse: Mouse,
    backgrounded: bool,
}

impl Grimoire {
    pub fn new(
        path: &Path,
        glsl_version: String,
        shader_header: String,
        shader_footer: String,
    ) -> Result<Self> {
        Ok(Self {
            shader_stream: FileStream::new(path)?,
            shader: Effect::new(glsl_version, shader_header, shader_footer),
            resource_streams: Default::default(),
            mouse: Default::default(),
            playing: Default::default(),
            time: Default::default(),
            frame: Default::default(),
            backgrounded: Default::default(),
        })
    }

    pub fn play(&mut self) -> Result<()> {
        self.playing = true;
        for (_, ref mut stream) in &mut self.resource_streams {
            stream.play()?;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
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
        if self.backgrounded {
            return Ok(false);
        }
        // load the shader
        let shader_bytes_opt = self.shader_stream.try_recv()?;
        let mut new_shader_event = false;
        if let Some(shader_bytes) = shader_bytes_opt {
            let shader_string: String = String::from_utf8(shader_bytes)
                .map_err(|err| Error::from_utf8(self.shader_stream.path(), err))?;
            let shader_config = EffectConfig::from_comment_block_in_str(&shader_string)?;
            let config_dirty = *self.shader.config() != shader_config;
            // If config is dirty, clear and repopulate the resource streams
            if config_dirty {
                self.resource_streams.clear();
                for (name, resource_config) in &shader_config.resources {
                    let stream = ResourceStream::new(name, resource_config)
                        .with_context(|_| ErrorKind::BadResourceConfig(name.to_string()))?;
                    self.resource_streams.push((name.clone(), stream));
                }
            }
            new_shader_event = self.shader.stage_shader(shader_string, shader_config);
        };
        // resource streaming
        for (_i, (ref name, ref mut stream)) in &mut self.resource_streams.iter_mut().enumerate() {
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
            let dt = duration_to_float_secs(platform.time_delta);
            let local_date: DateTime<Local> = Local::now();
            let year = local_date.year() as f32;
            let month = local_date.month() as f32;
            let day = local_date.day() as f32;
            let sec = local_date.hour() as f32 * 60.0 * 60.0
                + local_date.minute() as f32 * 60.0
                + local_date.second() as f32;
            let date = [year, month, day, sec];
            EffectState {
                time: duration_to_float_secs(self.time),
                time_delta: dt,
                date: date,
                frame: self.frame as f32,
                frame_rate: 1.0 / dt,
                mouse: mouse,
                window_resolution: [
                    platform.window_resolution.0 as f32,
                    platform.window_resolution.1 as f32,
                    platform.window_resolution.0 as f32 / platform.window_resolution.1 as f32,
                ],
            }
        };
        self.shader.stage_state("GRIM_STATE", state);
        // draw
        self.shader.draw(
            &platform.gl,
            platform.window_resolution.0,
            platform.window_resolution.1,
        )?;
        if new_shader_event && self.shader.is_ok() {
            info!("Successfully loaded shader!")
        }
        if self.playing {
            self.step_forward(platform.time_delta);
        }

        Ok(false)
    }
}
