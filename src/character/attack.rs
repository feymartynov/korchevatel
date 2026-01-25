use bevy::prelude::*;

#[derive(Component, Default)]
pub struct Attack {
    is_attacking: bool,
}

impl Attack {
    pub fn set_attacking(&mut self, is_attacking: bool) {
        self.is_attacking = is_attacking;
    }

    pub fn is_attacking(&self) -> bool {
        self.is_attacking
    }
}
