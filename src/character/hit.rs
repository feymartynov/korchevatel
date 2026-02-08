use std::time::Duration;

use bevy::prelude::*;

use crate::{character::Character, hit::HitMessage};

/// Время анимации попадания
const HIT_LIFETIME: Duration = Duration::from_millis(300);
/// Путь до спрайтов с анимацией попадания
const HIT_SPRITE_SHEET_PATH: &str = "images/blood_hit.png";
/// Размер спрайта попадания
const HIT_SIZE: UVec2 = UVec2::new(30, 30);
/// Кол-во кадров анимации попадания
const HIT_FRAMES: u32 = 3;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, take_hit);
    app.add_systems(Update, tick_hit_timer);
}

#[derive(Component)]
struct Hit {
    timer: Timer,
}

impl Default for Hit {
    fn default() -> Self {
        Self {
            timer: Timer::from_seconds(HIT_LIFETIME.as_secs_f32(), TimerMode::Once),
        }
    }
}

/// Обработка попадания в персонажа
fn take_hit(
    mut message_reader: MessageReader<HitMessage>,
    q: Query<(), With<Character>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for message in message_reader.read() {
        let Ok(()) = q.get(message.entity()) else {
            continue;
        };

        let layout = TextureAtlasLayout::from_grid(HIT_SIZE, HIT_FRAMES, 1, None, None);
        let texture_atlas_layout = texture_atlas_layouts.add(layout);

        let hit = commands
            .spawn((
                Name::new("Blood splash"),
                Sprite {
                    image: asset_server.load(HIT_SPRITE_SHEET_PATH),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout,
                        index: 0,
                    }),
                    ..Default::default()
                },
                Transform::from_translation(Vec3::new(
                    message.hit_point.x,
                    message.hit_point.y,
                    message.global_transform.translation().z,
                )),
                Hit::default(),
            ))
            .id();

        commands.entity(message.entity()).add_child(hit);
    }
}

/// Анимация попадания в персонажа
fn tick_hit_timer(
    time: Res<Time>,
    q: Query<(Entity, &mut Hit, &mut Sprite)>,
    mut commands: Commands,
) {
    for (entity, mut hit, mut sprite) in q {
        hit.timer.tick(time.delta());

        if hit.timer.is_finished() {
            commands.entity(entity).despawn();
        }

        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            let elapsed_rate = hit.timer.elapsed().as_secs_f64() / HIT_LIFETIME.as_secs_f64();
            atlas.index = (HIT_FRAMES as f64 * elapsed_rate) as usize;
        }
    }
}
