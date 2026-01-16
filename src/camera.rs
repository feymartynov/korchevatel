use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::level::LocationBoundary;

const EPSILON: f32 = 10.0;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<FocusPoint>();
    app.register_type::<CameraSettings>();
    app.add_systems(Startup, setup_camera);
    app.add_systems(Update, setup_camera_settings);
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

#[derive(Component, Reflect)]
#[reflect(Component)]
#[require(Camera2d)]
struct CameraSettings {
    /// Размер dead zone относительно экрана (для плавности, но не ограничения look-ahead)
    dead_zone_percentage: Vec2,
    /// Максимальное заглядывание (в сторону движения игрока)
    max_look_ahead: Vec2,
    /// Нижний порог сглаживания заглядывания
    low_smooth: f32,
    /// Верхний порог сглаживания заглядывания
    high_smooth: f32,
    /// Плавность следования камеры
    follow_smooth: f32,

    // Вычисляется динамически
    half_view: Vec2,
    dead_zone: Vec2,
    look_ahead_offset: Vec2,
    last_focus_pos: Vec2,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            dead_zone_percentage: Vec2::new(0.25, 0.25),
            max_look_ahead: Vec2::new(800.0, 400.0),
            low_smooth: 1.0,
            high_smooth: 6.0,
            follow_smooth: 2.0,
            half_view: Vec2::ZERO,
            dead_zone: Vec2::ZERO,
            look_ahead_offset: Vec2::ZERO,
            last_focus_pos: Vec2::ZERO,
        }
    }
}

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Name::new("Camera"),
        Camera2d,
        TiledParallaxCamera,
        IsDefaultUiCamera,
        FocusPoint::default(),
        CameraSettings::default(),
    ));
}

fn setup_camera_settings(q: Query<(&Projection, &mut CameraSettings), Changed<Projection>>) {
    for (projection, mut settings) in q {
        if let Projection::Orthographic(orthographic_projection) = projection {
            settings.half_view = orthographic_projection.area.size() * 0.5;
            settings.dead_zone = settings.half_view * settings.dead_zone_percentage;
        }
    }
}

fn camera_follow_focus(
    physics_time: Res<Time<Physics>>,
    graphics_time: Res<Time>,
    mut camera_q: Query<(&mut Transform, &mut CameraSettings, &FocusPoint)>,
    focus_q: Query<&GlobalTransform>,
    boundaries_q: Query<&LocationBoundary>,
) {
    if physics_time.is_paused() {
        return;
    }

    let Ok((mut camera_transform, mut settings, focus)) = camera_q.single_mut() else {
        return;
    };

    let Some(focus_transform) = focus.0.and_then(|e| focus_q.get(e).ok()) else {
        return;
    };

    let dt = graphics_time.delta_secs();
    let focus_pos = focus_transform.translation().truncate();
    let mut camera_pos = camera_transform.translation.truncate();

    let delta_pos = focus_pos - settings.last_focus_pos;
    settings.last_focus_pos = focus_pos;

    // Накопление заглядывания
    let move_dir = delta_pos.normalize_or_zero();
    let target_look_ahead = move_dir * settings.max_look_ahead;
    let dynamic_smooth = settings.low_smooth.lerp(settings.high_smooth, 1.0);
    let look_lerp = 1.0 - (-dynamic_smooth * dt).exp();

    let offset = settings
        .look_ahead_offset
        .lerp(target_look_ahead, look_lerp);

    settings.look_ahead_offset = Vec2::new(
        offset.x.signum() * f32::max(0.0, offset.x.abs() - EPSILON),
        offset.y.signum() * f32::max(0.0, offset.y.abs() - EPSILON),
    );

    // Плавное следование в зависимости от мёртвой зоны
    let delta = focus_pos - camera_pos;
    let factor = (delta.abs() / settings.dead_zone).clamp(Vec2::ZERO, Vec2::ONE);
    let smooth = Vec2::ONE - (-settings.follow_smooth * dt * factor).exp();
    camera_pos += delta * smooth + settings.look_ahead_offset * dt;

    // Ограничение рамкой локации
    if let Some(boundary) = boundaries_q
        .iter()
        .find(|b| b.to_rect().contains(focus_pos))
    {
        let rect = boundary.to_rect();
        let min = rect.min + settings.half_view;
        let max = rect.max - settings.half_view;
        camera_pos = camera_pos.clamp(min, max);
    }

    camera_transform.translation.x = camera_pos.x;
    camera_transform.translation.y = camera_pos.y;
}
