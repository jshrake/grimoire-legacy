use std::sync::mpsc::Sender;
use crate::config::{KeyboardConfig, TextureFormat};
use crate::error::Result;
use crate::resource::{ResourceData, ResourceData2D};
use crate::stream::Stream;
use std::boxed::Box;

// Keyboard is used as a variant so we want to box bytes such that
// it doesn't make the enum huge
pub struct Keyboard {
    bytes: Box<[u8; 256 * 3]>,
}

impl Keyboard {
    pub fn new(_config: &KeyboardConfig) -> Self {
        Self {
            bytes: Box::new([0; 256 * 3]),
        }
    }
    pub fn tick(&mut self, presses: &[u8; 256]) {
        // Update the second row with keypresses
        for (i, press) in presses.iter().enumerate() {
            self.bytes[i + 256] = if *press == 255 && self.bytes[i] == 0 {
                255
            } else {
                0
            }
        }
        // Update the first row with current state
        self.bytes[..256].clone_from_slice(&presses[..]);
        // Update the toggle row
        for i in 0..256 {
            if self.bytes[i + 256] == 255 {
                self.bytes[i + 256 * 2] = 255 - self.bytes[i + 256 * 2];
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
