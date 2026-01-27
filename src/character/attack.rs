use std::time::Duration;

use avian2d::math::*;
use avian2d::prelude::*;
use bevy::prelude::*;

use crate::level::Layer;
use crate::movement::Direction;

const ATTACK_TIME: Duration = Duration::from_millis(100);
const COOLDOWN_TIME: Duration = Duration::from_millis(100);
const MAX_DISTANCE: Scalar = 500.0;

pub(super) fn plugin(app: &mut App) {
    app.add_message::<AttackMessage>();
    app.add_systems(FixedUpdate, (apply_attack.before(attack), attack));
    app.add_systems(Update, tick_timer.before(attack));
}

#[derive(Message)]
pub enum AttackMessage {
    /// Команда атаковать
    Attack { attacker: Entity },
}

/// Компонент атаки
#[derive(Component, Debug, Default)]
pub struct Attack {
    state: State,
}

#[derive(Debug, Default)]
enum State {
    /// Готов к атаке
    #[default]
    Idle,
    /// В процессе атаки
    Attack(Timer),
    /// Пауза перед следующей атакой
    Cooldown(Timer),
}

impl Attack {
    /// Атакует ли прямо сейчас?
    pub fn is_attacking(&self) -> bool {
        matches!(self.state, State::Attack(_))
    }
}

/// Команда на атаку
fn apply_attack(mut message_reader: MessageReader<AttackMessage>, mut q: Query<&mut Attack>) {
    for message in message_reader.read() {
        match message {
            AttackMessage::Attack { attacker } => {
                let Ok(mut attack) = q.get_mut(*attacker) else {
                    continue;
                };

                // Если не готов атаковать, игнорируем команду
                if !matches!(attack.state, State::Idle) {
                    continue;
                }

                // Заводим таймер атаки
                attack.state = State::Attack(Timer::from_seconds(
                    ATTACK_TIME.as_secs_f32(),
                    TimerMode::Once,
                ));
            }
        }
    }
}

fn tick_timer(time: Res<Time>, q: Query<&mut Attack>) {
    for mut attack in q {
        match &mut attack.state {
            State::Idle => (),
            State::Attack(timer) | State::Cooldown(timer) => {
                timer.tick(time.delta());
            }
        }
    }
}

/// Стейт-машина атаки
fn attack(q: Query<(&mut Attack, &Direction, &Layer, &GlobalTransform)>, spatial_q: SpatialQuery) {
    for (mut attack, direction, layer, global_transform) in q {
        match &attack.state {
            State::Idle => (),
            State::Attack(timer) => {
                // Задержка на прицеливание
                if !timer.is_finished() {
                    continue;
                }

                // Собственно, выстрел. Выпускаем виртуальный луч, смотрим во что попадёт
                let maybe_hit = spatial_q.cast_ray(
                    // TODO: От ружья, а не от центра персонажа
                    global_transform.translation().truncate(),
                    (*direction).into(),
                    MAX_DISTANCE,
                    false,
                    &SpatialQueryFilter {
                        // TODO: Стрельба между слоями
                        mask: (*layer).into(),
                        ..Default::default()
                    },
                );

                if let Some(hit_data) = maybe_hit {
                    println!("Hit! {hit_data:?}");
                } else {
                    println!("Miss!");
                }

                // Пауза перед следующей атакой
                attack.state = State::Cooldown(Timer::from_seconds(
                    COOLDOWN_TIME.as_secs_f32(),
                    TimerMode::Once,
                ));
            }
            State::Cooldown(timer) => {
                // Если пауза прошла, готов снова атаковать
                if timer.is_finished() {
                    attack.state = State::Idle;
                }
            }
        }
    }
}
