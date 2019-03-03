use sdl2::mouse::MouseButton;
use std::collections::HashSet;

#[derive(Debug)]
pub struct Mouse {
    state: [f32; 4],
    buttons_last_update: HashSet<MouseButton>,
}

impl Default for Mouse {
    fn default() -> Self {
        Self {
            state: [0.0, 0.0, -0.0, -0.0],
            buttons_last_update: Default::default(),
        }
    }
}

impl Mouse {
    pub fn _new() -> Self {
        Default::default()
    }

    pub fn update(&mut self, buttons: HashSet<MouseButton>, x: u32, y: u32) -> [f32; 4] {
        let new_buttons = &buttons - &self.buttons_last_update;
        let old_buttons = &self.buttons_last_update - &buttons;
        let mouse_down =
            new_buttons.contains(&MouseButton::Left) && !old_buttons.contains(&MouseButton::Left);
        let mouse_up =
            !new_buttons.contains(&MouseButton::Left) && old_buttons.contains(&MouseButton::Left);
        let x = x as f32;
        let y = y as f32;
        if mouse_down {
            self.down(x, y);
        } else if mouse_up {
            self.up();
        }
        self.hover(x, y);
        self.buttons_last_update = buttons;
        self.state
    }

    fn down(&mut self, x: f32, y: f32) -> &mut Self {
        self.state[2] = x;
        self.state[3] = y;
        self
    }

    fn up(&mut self) -> &mut Self {
        self.state[2] *= -1.0;
        self.state[3] *= -1.0;
        self
    }

    fn hover(&mut self, x: f32, y: f32) -> &mut Self {
        if self.state[2] > 0.0 && self.state[3] > 0.0 {
            self.state[0] = x;
            self.state[1] = y;
        }
        self
    }
}
