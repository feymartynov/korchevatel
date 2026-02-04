use bevy::prelude::*;

use crate::hit::HitMessage;
use crate::object::Object;

/// Путь до спрайтов с попаданиями
const HIT_SPRITE_SHEET_PATH: &str = "images/bullet_holes.png";
/// Размер спрайта попадания
const HIT_SIZE: UVec2 = UVec2::new(16, 16);
/// Кол-во колонок на спрайтшите попадания
const HIT_SPRITE_SHEET_COLUMNS: u32 = 8;
/// Кол-во строк на спрайтшите попадания
const HIT_SPRITE_SHEET_ROWS: u32 = 8;
/// Кол-во вариантов спрайта попадания
const HIT_SPRITES_COUNT: usize = (HIT_SPRITE_SHEET_COLUMNS * HIT_SPRITE_SHEET_ROWS) as usize;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, take_hit);
}

/// Обработка попадания в объект
fn take_hit(
    mut message_reader: MessageReader<HitMessage>,
    q: Query<(), With<Object>>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for message in message_reader.read() {
        let Ok(()) = q.get(message.entity()) else {
            continue;
        };

        let texture_atlas_layout = texture_atlas_layouts.add(TextureAtlasLayout::from_grid(
            HIT_SIZE,
            HIT_SPRITE_SHEET_COLUMNS,
            HIT_SPRITE_SHEET_ROWS,
            None,
            None,
        ));

        let hit = commands
            .spawn((
                Name::new("Bullet hole"),
                Sprite {
                    image: asset_server.load(HIT_SPRITE_SHEET_PATH),
                    texture_atlas: Some(TextureAtlas {
                        layout: texture_atlas_layout,
                        index: rand::random_range(0..HIT_SPRITES_COUNT),
                    }),
                    ..Default::default()
                },
                Transform::from_translation(Vec3::new(
                    message.hit_point.x,
                    message.hit_point.y,
                    message.transform.translation.z,
                )),
            ))
            .id();

        commands.entity(message.entity()).add_child(hit);
    }
}
