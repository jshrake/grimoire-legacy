use std::sync::mpsc::Sender;

use config::{KeyboardConfig, TextureFormat};
use error::Result;
use resource::{ResourceData, ResourceData2D};
use sdl2::keyboard::KeyboardState;
use stream::Stream;

pub struct Keyboard {
    bytes: [u8; 256 * 3],
}

impl Keyboard {
    pub fn new(_config: &KeyboardConfig) -> Self {
        Self {
            bytes: [0; 256 * 3],
        }
    }

    pub fn tick(&mut self, keyboard_state: &KeyboardState) {
        let mut presses: Vec<u8> = vec![0; 256];
        keyboard_state.pressed_scancodes().for_each(|s| {
            let idx = s as usize;
            presses[idx] = 1;
        });

        // Update the second row with keypresses
        for i in 0..256 {
            self.bytes[i + 256] = if presses[i] == 1 && self.bytes[i] == 0 {
                1
            } else {
                0
            }
        }
        // Update the first row with current state
        for i in 0..256 {
            self.bytes[i] = presses[i];
        }
        // Update the toggle row
        for i in 0..256 {
            if self.bytes[i + 256] == 1 {
                self.bytes[i + 256 * 2] = 1 - self.bytes[i + 256 * 2];
            }
        }
    }
}

impl Stream for Keyboard {
    fn stream_to(&mut self, dest: &Sender<ResourceData>) -> Result<()> {
        let resource = ResourceData::D2(ResourceData2D {
            bytes: self.bytes.to_vec(),
            width: 256,
            height: 3,
            format: TextureFormat::RU8,
            xoffset: 0,
            yoffset: 0,
            subwidth: 256,
            subheight: 3,
            time: 0.0,
        });
        match dest.send(resource) {
            _ => (),
        }
        Ok(())
    }
}
