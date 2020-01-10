use crate::gl::GLRc;
use sdl2::EventPump;
use std::time::Duration;

pub struct Platform<'a> {
  pub events: &'a mut EventPump,
  pub gl: GLRc,
  pub window_resolution: (u32, u32),
  pub mouse_resolution: (u32, u32),
  pub time_delta: Duration,
  pub keyboard: [u8; 256],
}
