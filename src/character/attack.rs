use std::cmp::Ordering;
use std::time::Duration;

use avian2d::math::*;
use avian2d::prelude::*;
use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use bevy::sprite::Anchor;

use crate::character::Character;
use crate::level::Layer;
use crate::movement::Direction;
use crate::movement::Ground;

/// Время прицеливания и выстрела
const ATTACK_TIME: Duration = Duration::from_millis(100);
/// Пауза между выстрелами
const COOLDOWN_TIME: Duration = Duration::from_millis(100);
/// Длина стрельбы
const MAX_X_DISTANCE: Scalar = 500.0;
/// Высота стрельбы
const MAX_Y_DISTANCE: Scalar = 100.0;
/// Время анимации попадания
const HIT_LIFETIME: Duration = Duration::from_millis(300);
/// Путь до спрайтов с анимацией попадания
const HIT_SPRITE_SHEET_PATH: &str = "images/blood_hit.png";
/// Размер спрайта попадания
const HIT_SIZE: UVec2 = UVec2::new(30, 30);
/// Кол-во кадров анимации попадания
const HIT_FRAMES: u32 = 3;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, attack);
    app.add_systems(
        Update,
        (
            tick_attack_timer.before(attack),
            tick_hit_timer.after(attack),
        ),
    );
}

#[derive(Component, Default)]
pub enum AttackInput {
    #[default]
    Idle,
    Attack,
}

impl AttackInput {
    /// Атакует ли прямо сейчас?
    pub fn is_attacking(&self) -> bool {
        matches!(self, AttackInput::Attack)
    }
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

fn tick_attack_timer(time: Res<Time>, q: Query<&mut Attack>) {
    for mut attack in q {
        match &mut attack.state {
            State::Idle => (),
            State::Attack(timer) | State::Cooldown(timer) => {
                timer.tick(time.delta());
            }
        }
    }
}

type HitEntityComponents<'a> = (
    &'a Transform,
    &'a Layer,
    &'a Anchor,
    &'a Collider,
    Has<Character>,
    Has<Ground>,
);

/// Стейт-машина атаки
fn attack(
    q: Query<(
        Entity,
        &mut Attack,
        &AttackInput,
        &Direction,
        &GlobalTransform,
        &Layer,
    )>,
    spatial_q: SpatialQuery,
    hit_entity_q: Query<HitEntityComponents>,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlas_layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    for (attacker, mut attack, attack_input, direction, global_transform, layer) in q {
        match &attack.state {
            State::Idle => {
                if !matches!(attack_input, AttackInput::Attack) {
                    continue;
                }

                // Если не готов атаковать, игнорируем команду
                if !matches!(attack.state, State::Idle) {
                    return;
                }

                // Заводим таймер атаки
                attack.state = State::Attack(Timer::from_seconds(
                    ATTACK_TIME.as_secs_f32(),
                    TimerMode::Once,
                ));
            }
            State::Attack(timer) => {
                // Задержка на прицеливание
                if !timer.is_finished() {
                    continue;
                }

                // Если куда-то попали, отрабатываем попадание
                let origin = global_transform.translation().truncate();

                if let Some(hit) =
                    shoot(attacker, origin, layer, direction, &spatial_q, hit_entity_q)
                {
                    take_hit(
                        &hit,
                        &mut commands,
                        &asset_server,
                        &mut texture_atlas_layouts,
                    );
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

#[derive(Clone, Debug)]
struct HitEntityBundle {
    hit_data: ShapeHitData,
    transform: Transform,
    layer: Layer,
    anchor: Anchor,
    collider: Collider,
    is_character: bool,
}

/// Стрельба
fn shoot(
    attacker: Entity,
    origin: Vec2,
    origin_layer: &Layer,
    direction: &Direction,
    spatial_q: &SpatialQuery,
    hit_entity_q: Query<HitEntityComponents>,
) -> Option<HitEntityBundle> {
    // Находим все сущности на всех слоях, в которые можно попасть
    let mut hits = Vec::new();

    let mut filter = SpatialQueryFilter {
        mask: LayerMask(u32::MAX),
        excluded_entities: EntityHashSet::new(),
    };

    filter.excluded_entities.insert(attacker);

    loop {
        // Выпускаем виртуальный прямоугольник. Пересечение с ним означает потенциальное попадание.
        let maybe_hit = spatial_q.cast_shape(
            &Collider::rectangle(MAX_X_DISTANCE, MAX_Y_DISTANCE * 2.0),
            Vec2::new(origin.x, origin.y - MAX_Y_DISTANCE),
            0.0,
            (*direction).into(),
            &ShapeCastConfig {
                max_distance: MAX_X_DISTANCE,
                target_distance: 0.0,
                compute_contact_on_penetration: true,
                ignore_origin_penetration: true,
            },
            &filter,
        );

        let Some(hit_data) = maybe_hit else {
            break; // Если попаданий больше нет, поиск закончен
        };

        // В следующий раз исключаем из поиска сущность, в которую попали, чтобы найти следующую
        filter.excluded_entities.insert(hit_data.entity);

        let Ok(components) = hit_entity_q.get(hit_data.entity) else {
            continue;
        };

        if components.5 {
            continue; // В пол не стреляем
        }

        hits.push(HitEntityBundle {
            hit_data,
            transform: components.0.clone(),
            layer: components.1.clone(),
            anchor: components.2.clone(),
            collider: components.3.clone(),
            is_character: components.4,
        });
    }

    // В `hits` достижимые сущности в порядке близости. Выбираем, в кого из них стрелять.
    hits.sort_by_key(|h| -(h.is_character as isize)); // Персонажи приоритетнее объектов

    for hit in &hits {
        if !is_obstructed(origin_layer, &hit.layer, *hit.anchor, spatial_q) {
            return Some(hit.clone());
        }
    }

    None
}

/// Определяет нет ли препятствия между слоями для заданной точки
fn is_obstructed(src: &Layer, dst: &Layer, point: Vec2, spatial_q: &SpatialQuery) -> bool {
    let src_id = src.id();
    let dst_id = dst.id();

    let (low, high) = match dst_id.cmp(&src_id) {
        Ordering::Equal => return false, // Тот же слой => ничего не может влезть
        Ordering::Greater => (src_id, dst_id),
        Ordering::Less => (dst_id, src_id),
    };

    let depth = high.saturating_sub(low).saturating_sub(1);

    if depth == 0 {
        return false; // Соседние слои => между ними ничего не может влезть
    }

    let layer_mask = ((1 << depth) - 1) << (low + 1); // (3, 6) => 0b110000

    let filter = SpatialQueryFilter {
        mask: LayerMask(layer_mask),
        ..Default::default()
    };

    spatial_q.project_point(point, true, &filter).is_some()
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

/// Обработка попадания
fn take_hit(
    hit_entity_bundle: &HitEntityBundle,
    commands: &mut Commands,
    asset_server: &Res<AssetServer>,
    texture_atlas_layouts: &mut ResMut<Assets<TextureAtlasLayout>>,
) {
    let layout = TextureAtlasLayout::from_grid(HIT_SIZE, HIT_FRAMES, 1, None, None);
    let texture_atlas_layout = texture_atlas_layouts.add(layout);

    let hit = commands
        .spawn((
            Name::new("Hit"),
            Sprite {
                image: asset_server.load(HIT_SPRITE_SHEET_PATH),
                texture_atlas: Some(TextureAtlas {
                    layout: texture_atlas_layout,
                    index: 0,
                }),
                ..Default::default()
            },
            Transform::from_translation(Vec3::new(
                hit_entity_bundle.anchor.x,
                hit_entity_bundle.anchor.y
                    + hit_entity_bundle
                        .collider
                        .shape()
                        .as_capsule()
                        .unwrap()
                        .height()
                        * 0.4,
                hit_entity_bundle.transform.translation.z,
            )),
            Hit::default(),
        ))
        .id();

    commands
        .entity(hit_entity_bundle.hit_data.entity)
        .add_child(hit);
}

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

        // Анимация попадания
        if let Some(atlas) = sprite.texture_atlas.as_mut() {
            let elapsed_rate = hit.timer.elapsed().as_secs_f64() / HIT_LIFETIME.as_secs_f64();
            atlas.index = (HIT_FRAMES as f64 * elapsed_rate) as usize;
        }
    }
}
