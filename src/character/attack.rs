use core::f32;
use std::cmp::Ordering;
use std::time::Duration;

use avian2d::math::*;
use avian2d::prelude::*;
use bevy::ecs::entity::EntityHashSet;
use bevy::prelude::*;
use rand_distr::{Distribution, Normal};

use crate::character::Character;
use crate::hit::HitMessage;
use crate::level::Layer;
use crate::movement::Direction;

/// Время прицеливания и выстрела
const ATTACK_TIME: Duration = Duration::from_millis(100);
/// Пауза между выстрелами
const COOLDOWN_TIME: Duration = Duration::from_millis(100);
/// Длина стрельбы
const MAX_X_DISTANCE: Scalar = 500.0;
/// Высота стрельбы
const MAX_Y_DISTANCE: Scalar = 100.0;
/// Среднеквадратичное отклонение от цели попадания
const PRECISION_SIGMA: f32 = 30.0;

const MAX_DISTANCE_SQUARE: f32 = MAX_X_DISTANCE * MAX_X_DISTANCE + MAX_Y_DISTANCE * MAX_Y_DISTANCE;

pub(super) fn plugin(app: &mut App) {
    app.add_systems(FixedUpdate, attack);
    app.add_systems(Update, tick_attack_timer.before(attack));
    #[cfg(debug_assertions)]
    app.add_plugins(debug::plugin);
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

type HitEntityComponents<'a> = (&'a GlobalTransform, &'a Layer, &'a Collider, Has<Character>);

/// Стейт-машина атаки
fn attack(
    q: Query<(
        Entity,
        &Character,
        &mut Attack,
        &AttackInput,
        &Direction,
        &GlobalTransform,
        &Layer,
    )>,
    spatial_q: SpatialQuery,
    hit_entity_q: Query<HitEntityComponents>,
    mut hit_message_writer: MessageWriter<HitMessage>,
) {
    for (
        attacker_entity,
        attacker_character,
        mut attack,
        attack_input,
        direction,
        global_transform,
        layer,
    ) in q
    {
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

                // Стреляем
                let origin = global_transform.translation().truncate()
                    - attacker_character.shooting_origin_offset;

                if let Some(hit) = shoot(
                    attacker_entity,
                    origin,
                    layer,
                    direction,
                    &spatial_q,
                    hit_entity_q,
                ) {
                    hit_message_writer.write(hit);
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

/// Стрельба
fn shoot(
    attacker: Entity,
    origin: Vec2,
    origin_layer: &Layer,
    direction: &Direction,
    spatial_q: &SpatialQuery,
    hit_entity_q: Query<HitEntityComponents>,
) -> Option<HitMessage> {
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
                compute_contact_on_penetration: false,
                ignore_origin_penetration: true,
            },
            &filter,
        );

        let Some(hit_data) = maybe_hit else {
            break; // Если попаданий больше нет, поиск закончен
        };

        // В следующий раз исключаем из поиска сущность, в которую попали, чтобы искать дальше.
        filter.excluded_entities.insert(hit_data.entity);

        let Ok((global_transform, layer, collider, is_character)) =
            hit_entity_q.get(hit_data.entity)
        else {
            continue;
        };

        // Собственно, выстрел в выбранный объект. Определяем точку попадания на контуре.
        let origin_local = origin - global_transform.translation().truncate();

        let (outline_point, _is_inside) =
            collider.project_point(Vec2::ZERO, 0.0, origin_local, true);

        // Почему-то `_is_inside`` всегда = false для Polyline. Возможно, баг в движке.
        // Вычисляем через принадлежность точки внутри bounding box. Не очень точно, но сойдёт.
        let aabb = collider.aabb(Vec2::ZERO, 0.0);
        let is_inside = aabb.intersects(&ColliderAabb::from_min_max(origin_local, origin_local));

        // Сдвигаем точку внутрь контура на 2.5 сигмы разброса, чтобы при добавлении погрешности
        // с высокой вероятностью сбитая точка оказалась внутри.
        let ray = outline_point - origin_local;
        let normal = ray.normalize();
        let signum = if is_inside { -1.0 } else { 1.0 };
        let hit_point = outline_point + signum * normal * PRECISION_SIGMA * 3.0;

        // Сбиваем на случайную величину. Чем дальше, тем сильнее.
        let sigma = PRECISION_SIGMA * ray.length().powf(2.0) / MAX_DISTANCE_SQUARE;

        let hit_point = Vec2::new(
            hit_point.x + rand_delta(sigma),
            hit_point.y + rand_delta(sigma),
        );

        hits.push(HitMessage {
            hit_data,
            global_transform: *global_transform,
            layer: *layer,
            origin,
            hit_point,
            is_character,
        });
    }

    // В `hits` достижимые сущности в порядке близости. Выбираем, в кого из них стрелять.
    // Персонажи приоритетнее объектов.
    hits.sort_by_key(|h| -(h.is_character as isize));

    // Выбираем первую незагороженную цель.
    for hit in &hits {
        if !is_obstructed(origin_layer, &hit.layer, hit.hit_point, spatial_q) {
            return Some(hit.clone());
        }
    }

    None
}

/// Определяет, нет ли препятствия между слоями для заданной точки
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

#[inline]
fn rand_delta(sigma: f32) -> f32 {
    let normal = Normal::new(0.0, sigma).expect("normal distribution");
    let mut rng = rand::rng();
    normal.sample(&mut rng)
}

////////////////////////////////////////////////////////////////////////////////

#[cfg(debug_assertions)]
mod debug {
    use super::*;

    const ATTACK_DEBUG_LIFETIME: Duration = Duration::from_millis(300);

    pub fn plugin(app: &mut App) {
        app.add_systems(Update, debug_attack);
        app.add_systems(FixedUpdate, tick_debug_attack_timer);
    }

    #[derive(Component)]
    struct AttackDebugRay {
        timer: Timer,
    }

    impl Default for AttackDebugRay {
        fn default() -> Self {
            Self {
                timer: Timer::from_seconds(ATTACK_DEBUG_LIFETIME.as_secs_f32(), TimerMode::Once),
            }
        }
    }

    fn debug_attack(
        mut message_reader: MessageReader<HitMessage>,
        mut commands: Commands,
        mut meshes: ResMut<Assets<Mesh>>,
        mut materials: ResMut<Assets<ColorMaterial>>,
    ) {
        for message in message_reader.read() {
            commands.spawn((
                Mesh2d(meshes.add(Segment2d::new(
                    message.origin,
                    message.hit_point + message.global_transform.translation().truncate(),
                ))),
                MeshMaterial2d(materials.add(Color::srgb(1.0, 0.0, 0.0))),
                Transform::from_xyz(0.0, 0.0, message.global_transform.translation().z),
                AttackDebugRay::default(),
            ));
        }
    }

    fn tick_debug_attack_timer(
        time: Res<Time>,
        q: Query<(Entity, &mut AttackDebugRay)>,
        mut commands: Commands,
    ) {
        for (entity, mut attack_debug) in q {
            attack_debug.timer.tick(time.delta());

            if attack_debug.timer.is_finished() {
                commands.entity(entity).despawn();
            }
        }
    }
}
