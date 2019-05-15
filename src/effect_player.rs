use crate::config::EffectConfig;
use crate::config::ResourceConfig;
use crate::effect::{Effect, EffectState};
use crate::error::{Error, ErrorKind, Result};
use crate::file_stream::FileStream;
use crate::mouse::Mouse;
use crate::platform::Platform;
use crate::stream::{ResourceStream, Stream};
use chrono::prelude::*;
use failure::ResultExt;
use glsl_include::Context as GlslIncludeContex;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

pub struct EffectPlayer<'a> {
    config_stream: FileStream,
    shader_include_streams: BTreeMap<String, FileStream>,
    shader_streams: BTreeMap<String, FileStream>,
    resource_streams: BTreeMap<String, ResourceStream>,
    unexpanded_pass_shaders: BTreeMap<String, String>,
    glsl_include_ctx: RefCell<GlslIncludeContex<'a>>,
    effect: Effect<'a>,
    playing: bool,
    time: Duration,
    frame: u32,
    mouse: Mouse,
}

impl<'a> EffectPlayer<'a> {
    pub fn new(
        config_path: &Path,
        glsl_version: String,
        shader_include_streams: BTreeMap<String, FileStream>,
        glsl_include_ctx: GlslIncludeContex<'a>,
    ) -> Result<Self> {
        Ok(Self {
            effect: Effect::new(glsl_version),
            glsl_include_ctx: RefCell::new(glsl_include_ctx),
            config_stream: FileStream::new(config_path)?,
            shader_include_streams,
            shader_streams: Default::default(),
            unexpanded_pass_shaders: Default::default(),
            resource_streams: Default::default(),
            mouse: Default::default(),
            playing: Default::default(),
            time: Default::default(),
            frame: Default::default(),
        })
    }

    pub fn play(&mut self) -> Result<()> {
        info!("[PLAYBACK] PLAY");
        self.playing = true;
        for stream in &mut self.resource_streams.values_mut() {
            stream.play()?;
        }
        Ok(())
    }

    pub fn pause(&mut self) -> Result<()> {
        info!("[PLAYBACK] PAUSE");
        self.playing = false;
        for stream in &mut self.resource_streams.values_mut() {
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
        info!("[PLAYBACK] RESTART");
        self.time = Default::default();
        self.frame = Default::default();
        for stream in &mut self.resource_streams.values_mut() {
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

    pub fn tick(&mut self, platform: &mut Platform) -> Result<()> {
        // Configuration changes
        if let Some(config_bytes) = self.config_stream.try_recv()? {
            let config_string: String = String::from_utf8(config_bytes)
                .map_err(|err| Error::from_utf8(self.config_stream.path(), err))?;
            let effect_config = EffectConfig::from_toml(&config_string)?;
            // Clear and repopulate resource streams
            self.resource_streams.clear();
            for (name, resource_config) in &effect_config.resources {
                let stream = ResourceStream::new(name, resource_config)
                    .with_context(|_| ErrorKind::BadResourceConfig(name.to_string()))?;
                self.resource_streams.insert(name.clone(), stream);
            }
            for (name, input) in &effect_config.resources {
                match input {
                    ResourceConfig::UniformFloat(u) => {
                        self.effect.stage_uniform1f(name.clone(), u.uniform);
                    }
                    ResourceConfig::UniformVec2(u) => {
                        self.effect.stage_uniform2f(name.clone(), u.uniform);
                    }
                    ResourceConfig::UniformVec3(u) => {
                        self.effect.stage_uniform3f(name.clone(), u.uniform);
                    }
                    ResourceConfig::UniformVec4(u) => {
                        self.effect.stage_uniform4f(name.clone(), u.uniform);
                    }
                    _ => continue,
                };
            }
            // clear and repopulate shader streams
            self.shader_streams.clear();
            for pass_config in &effect_config.passes {
                {
                    let vertex_path_str = &pass_config.vertex;
                    let vertex_path = Path::new(vertex_path_str);
                    let vertex_path = std::fs::canonicalize(vertex_path)
                        .expect("canonicalize failed on vertex path");
                    let vertex_stream = FileStream::new(vertex_path.as_path())?;
                    self.shader_streams
                        .insert(vertex_path_str.clone(), vertex_stream);
                }
                {
                    let fragment_path_str = &pass_config.fragment;
                    let fragment_path = Path::new(fragment_path_str);
                    let fragment_path = std::fs::canonicalize(fragment_path)
                        .expect("canonicalize failed on fragment path");
                    let fragment_stream = FileStream::new(fragment_path.as_path())?;
                    self.shader_streams
                        .insert(fragment_path_str.clone(), fragment_stream);
                }
                if let Some(ref geometry_path_str) = pass_config.geometry {
                    let geometry_path = Path::new(geometry_path_str);
                    let geometry_path = std::fs::canonicalize(geometry_path)
                        .expect("canonicalize failed on geometry path");
                    let geometry_stream = FileStream::new(geometry_path.as_path())?;
                    self.shader_streams
                        .insert(geometry_path_str.clone(), geometry_stream);

                }
            }
            self.effect.stage_config(effect_config)?;
        }

        // Check for changes in the config or shaders
        let mut shader_include_did_change = false;
        let mut pass_shader_did_change = false;
        // Include shader changes
        for (include_path, stream) in self.shader_include_streams.iter_mut() {
            if let Some(shader_bytes) = stream.try_recv()? {
                let mut ctx = self.glsl_include_ctx.borrow_mut();
                let shader_string: String = String::from_utf8(shader_bytes)
                    .map_err(|err| Error::from_utf8(stream.path(), err))?;
                ctx.include(include_path.to_string(), shader_string);
                shader_include_did_change = true;
            }
        }

        // Pass shader changes
        for (path, stream) in self.shader_streams.iter_mut() {
            if let Some(shader_bytes) = stream.try_recv()? {
                let shader_string: String = String::from_utf8(shader_bytes)
                    .map_err(|err| Error::from_utf8(stream.path(), err))?;
                self.unexpanded_pass_shaders
                    .insert(path.to_string(), shader_string);
                pass_shader_did_change = true;
            }
        }
        let shader_did_change = shader_include_did_change || pass_shader_did_change;
        if shader_did_change {
            let mut shader_cache = BTreeMap::new();
            let ctx = self.glsl_include_ctx.borrow_mut();
            for (path, source) in self.unexpanded_pass_shaders.iter() {
                let expanded = ctx
                    .expand(source.clone())
                    .expect("glsl include expansion failed");
                shader_cache.insert(path.clone(), expanded);
            }
            self.effect.stage_shader_cache(shader_cache)?;
        }

        // resource streaming
        for (ref name, ref mut stream) in &mut self.resource_streams.iter_mut() {
            match stream.tick(platform) {
                Ok(ref mut resources) => {
                    while let Some(resource) = resources.next() {
                        self.effect.stage_resource(&name, resource);
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
        self.effect.stage_state("GRIM_STATE", &state);
        self.effect.draw(
            &platform.gl,
            state.window_resolution[0],
            state.window_resolution[1],
        )?;
        if self.playing {
            self.step_forward(platform.time_delta);
        }
        Ok(())
    }
}
