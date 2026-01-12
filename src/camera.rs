use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

/// Процент размера кадра, в пределах которого, в каждую сторону игрок может перемещаться
/// без движения камеры. Если уйдёт дальше, камера поедет за ним.
const DEAD_ZONE_PERCENTAGE: f32 = 0.25;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<CameraBoundary>();
    app.register_type::<FocusPoint>();
    app.register_type::<CameraCache>();
    app.add_systems(Startup, (setup_camera, setup_camera_cache));
    app.add_systems(FixedUpdate, cache_compute_boundaries);

    app.add_systems(
        PostUpdate,
        camera_follow_focus.before(TransformSystems::Propagate),
    );
}

/// Граница локации, за которую камера не выходит, если игрок подходит близко к ней.
/// Каждая локация должна быть обрисована этим прямоугольником, чтобы не было видно пустоты.
/// Игрок всегда должен перемещаться в границах этого прямоугольника.
/// Но если вдруг он вылезет оттуда, то ограничения перемещения камеры не будет.
#[derive(Component, Default, Reflect)]
#[require(TiledObject)]
#[reflect(Component)]
pub struct CameraBoundary(Rect);

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

fn setup_camera_cache(mut q: Query<(&Projection, &mut CameraCache), Added<CameraCache>>) {
    for (projection, mut cache) in &mut q {
        if let Projection::Orthographic(orthographic_projection) = projection {
            cache.half_view = orthographic_projection.area.size() * 0.5;
            cache.dead_zone = cache.half_view * DEAD_ZONE_PERCENTAGE;
        }
    }
}

fn cache_compute_boundaries(
    q: Query<(&GlobalTransform, &TiledObject, &mut CameraBoundary), Added<CameraBoundary>>,
) {
    for (transform, tiled_object, mut boundary) in q {
        if let TiledObject::Rectangle { width, height } = tiled_object {
            let half_size = Vec2::new(*width, *height) * 0.5;
            let center = transform.translation().truncate();

            boundary.0 = Rect {
                min: center - half_size,
                max: center + half_size,
            };
        }
    }
}

fn camera_follow_focus(
    mut camera_q: Query<(&mut Transform, &CameraCache, &FocusPoint)>,
    focus_q: Query<&GlobalTransform>,
    boundaries_q: Query<&CameraBoundary>,
) {
    let Ok((mut camera_transform, view, focus)) = camera_q.single_mut() else {
        return;
    };

    let Some(focus_entity) = focus.0.and_then(|e| focus_q.get(e).ok()) else {
        return;
    };

    let focus_pos = focus_entity.translation().truncate();
    let mut camera_pos = camera_transform.translation.truncate();

    // Слежение с учётом dead zone
    let delta = focus_pos - camera_pos;

    if delta.x.abs() > view.dead_zone.x {
        camera_pos.x += delta.x - view.dead_zone.x * delta.x.signum();
    }
    if delta.y.abs() > view.dead_zone.y {
        camera_pos.y += delta.y - view.dead_zone.y * delta.y.signum();
    }

    // Находим активную границу по фокусу
    if let Some(boundary) = boundaries_q.iter().find(|b| b.0.contains(focus_pos)) {
        let min = boundary.0.min + view.half_view;
        let max = boundary.0.max - view.half_view;

        camera_pos.x = camera_pos.x.clamp(min.x, max.x);
        camera_pos.y = camera_pos.y.clamp(min.y, max.y);
    }

    camera_transform.translation.x = camera_pos.x;
    camera_transform.translation.y = camera_pos.y;
}
