use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::level::LocationBoundary;

/// Процент размера кадра, в пределах которого, в каждую сторону игрок может перемещаться
/// без движения камеры. Если уйдёт дальше, камера поедет за ним.
const DEAD_ZONE_PERCENTAGE: f32 = 0.25;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<FocusPoint>();
    app.register_type::<CameraCache>();
    app.add_systems(Startup, setup_camera);
    app.add_systems(Update, setup_camera_cache);

    app.add_systems(
        PostUpdate,
        camera_follow_focus.before(TransformSystems::Propagate),
    );
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
#[require(Camera2d)]
pub struct FocusPoint(Option<Entity>);

impl FocusPoint {
    pub fn new(entity: Entity) -> Self {
        Self(Some(entity))
    }
}

#[derive(Component, Default, Reflect)]
#[reflect(Component)]
#[require(Camera2d)]
struct CameraCache {
    half_view: Vec2,
    dead_zone: Vec2,
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera2d,
        TiledParallaxCamera,
        IsDefaultUiCamera,
        FocusPoint::default(),
        CameraCache::default(),
    ));
}

fn setup_camera_cache(q: Query<(&Projection, &mut CameraCache), Changed<Projection>>) {
    for (projection, mut cache) in q {
        if let Projection::Orthographic(orthographic_projection) = projection {
            cache.half_view = orthographic_projection.area.size() * 0.5;
            cache.dead_zone = cache.half_view * DEAD_ZONE_PERCENTAGE;
        }
    }
}

fn camera_follow_focus(
    mut camera_q: Query<(&mut Transform, &CameraCache, &FocusPoint)>,
    focus_q: Query<&GlobalTransform>,
    boundaries_q: Query<&LocationBoundary>,
) {
    let Ok((mut camera_transform, cache, focus)) = camera_q.single_mut() else {
        return;
    };

    let Some(focus_entity) = focus.0.and_then(|e| focus_q.get(e).ok()) else {
        return;
    };

    let focus_pos = focus_entity.translation().truncate();
    let mut camera_pos = camera_transform.translation.truncate();

    // Слежение с учётом dead zone
    let delta = focus_pos - camera_pos;

    if delta.x.abs() > cache.dead_zone.x {
        camera_pos.x += delta.x - cache.dead_zone.x * delta.x.signum();
    }

    if delta.y.abs() > cache.dead_zone.y {
        camera_pos.y += delta.y - cache.dead_zone.y * delta.y.signum();
    }

    // Ограничение перемещения рамкой локации
    if let Some(boundary) = boundaries_q
        .into_iter()
        .find(|b| b.to_rect().contains(focus_pos))
    {
        let boundary_rect = boundary.to_rect();
        let min = boundary_rect.min + cache.half_view;
        let max = boundary_rect.max - cache.half_view;

        camera_pos.x = camera_pos.x.clamp(min.x, max.x);
        camera_pos.y = camera_pos.y.clamp(min.y, max.y);
    }

    camera_transform.translation.x = camera_pos.x;
    camera_transform.translation.y = camera_pos.y;
}
