use std::time::Duration;

use avian2d::prelude::*;
use bevy::platform::collections::HashMap;
use bevy::prelude::*;

use crate::character::Attack;
use crate::movement::Direction;

const DELTA_X: f32 = 10.0;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Animation>();

    app.add_systems(
        Update,
        (
            update_animation_timer,
            update_animation_movement,
            update_animation_atlas,
        ),
    );
}

/// Анимация движения персонажа
fn update_animation_movement(
    mut q: Query<(
        &LinearVelocity,
        &Direction,
        Option<&Attack>,
        &mut Sprite,
        &mut Animation,
    )>,
) {
    for (linear_velocity, direction, maybe_attack, mut sprite, mut animation) in &mut q {
        let dx = linear_velocity.x;

        // Разворот в зависимости от направления взгляда
        if dx.abs() > DELTA_X {
            sprite.flip_x = matches!(direction, Direction::Left);
        }

        // Стоит или идёт в зависимости от скорости
        let movement_mode = if ops::abs(dx) >= DELTA_X {
            MovementMode::Walking
        } else {
            MovementMode::Idle
        };

        // Атакует или нет
        let is_attacking = maybe_attack
            .map(|attack| attack.is_attacking())
            .unwrap_or_default();

        let attack_mode = if is_attacking {
            AttackMode::Firing
        } else {
            AttackMode::None
        };

        let animation_state = AnimationState { movement_mode, attack_mode };
        animation.change_state(animation_state);
    }
}

/// Тик времени анимации для смены кадров
fn update_animation_timer(time: Res<Time>, mut query: Query<&mut Animation>) {
    for mut animation in &mut query {
        animation.update(time.delta());
    }
}

/// Смена набора спрайтов соответствующих текущему состоянию анимации
fn update_animation_atlas(mut query: Query<(&Animation, &mut Sprite)>) {
    for (animation, mut sprite) in &mut query {
        let Some(atlas) = sprite.texture_atlas.as_mut() else {
            continue;
        };

        if animation.has_changed() {
            atlas.index = animation.get_atlas_index();
        }
    }
}

#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component)]
pub struct Animation {
    timer: Timer,
    frame_index: usize,
    state: AnimationState,
    #[reflect(ignore)]
    config: HashMap<AnimationState, (Duration, Vec<usize>)>,
}

impl Animation {
    /// Регистрация анимации для персонажа
    pub fn register_state(
        mut self,
        state: AnimationState,
        duration: Duration,
        frames: Vec<usize>,
    ) -> Self {
        self.config.insert(state, (duration, frames));
        self
    }

    /// Меняет кадр анимации по тику
    fn update(&mut self, delta: Duration) {
        self.timer.tick(delta);

        if !self.timer.is_finished() {
            return;
        }

        if let Some((_, frames)) = self.config.get(&self.state) {
            self.frame_index = (self.frame_index + 1) % frames.len();
        }
    }

    /// Меняет состояния анимации, если она изменилась
    fn change_state(&mut self, state: AnimationState) {
        if self.state != state
            && let Some((duration, _)) = self.config.get(&state)
        {
            let old_state = self.state;
            self.state = state;
            
            // Сохраняем фазу движения, если режим не сменился
            if state.movement_mode != old_state.movement_mode {
                self.frame_index = 0;
            }

            self.timer = Timer::new(*duration, TimerMode::Repeating);
        }
    }

    /// Поменялась ли анимация в текущем тике?
    fn has_changed(&self) -> bool {
        self.timer.is_finished()
    }

    /// Определяет индекс спрайта по номеру кадра
    fn get_atlas_index(&self) -> usize {
        *self
            .config
            .get(&self.state)
            .and_then(|(_, frames)| frames.get(self.frame_index))
            .unwrap_or(&0)
    }
}

#[derive(Reflect, Hash, Eq, PartialEq, Default, Debug, Copy, Clone)]
pub enum MovementMode {
    #[default]
    Idle,
    Walking,
}

#[derive(Reflect, Hash, Eq, PartialEq, Default, Debug, Copy, Clone)]
pub enum AttackMode {
    #[default]
    None,
    Firing,
}

#[derive(Reflect, Hash, Eq, PartialEq, Default, Debug, Copy, Clone)]
pub struct AnimationState {
    pub movement_mode: MovementMode,
    pub attack_mode: AttackMode,
}
