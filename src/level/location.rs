use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

const THICKNESS: f32 = 200.0;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Boundary>();
    app.add_systems(FixedUpdate, compute_boundaries);
}

/// Граница локации, за которую камера и игрок не должны выходить.
/// Берётся из рамки фонового image layer в карте.
#[derive(Component, Default, Reflect)]
#[require(TiledObject)]
#[reflect(Component)]
pub struct Boundary(Rect);

impl Boundary {
    pub fn to_rect(&self) -> Rect {
        self.0
    }
}

fn compute_boundaries(
    q: Query<(Entity, &TiledImage, &Transform), Or<(Added<TiledImage>, Changed<Transform>)>>,
    mut commands: Commands,
) {
    for (entity, tiled_image, transform) in q {
        let top_left = transform.translation.truncate();
        let right_bottom = top_left + Vec2::new(tiled_image.base_size.x, -tiled_image.base_size.y);
        let rect = Rect::from_corners(top_left, right_bottom);

        // 4 виртуальные стены снаружи фоновой картинки, чтобы нельзя было сбежать из локации
        //
        // A----B----------------C----D
        // |    |                |    |
        // E----+----------------+----F
        // |    |////////////////|    |
        // |    |////////////////|    |
        // |    |////////////////|    |
        // G----+----------------+----H
        // |    |                |    |
        // I----J----------------K----L

        let t = THICKNESS;
        let w = rect.width();
        let h = rect.height();

        let w_wall_size = Vec2::new(w + t * 2.0, t);
        let h_wall_size = Vec2::new(t, h + t * 2.0);

        let top_wall = Vec2::new(w / 2.0, t / 2.0);
        let bottom_wall = Vec2::new(w / 2.0, -h - t / 2.0);
        let left_wall = Vec2::new(-t / 2.0, -h / 2.0);
        let right_wall = Vec2::new(w + t / 2.0, -h / 2.0);

        let w_wall = Collider::rectangle(w_wall_size.x, w_wall_size.y);
        let h_wall = Collider::rectangle(h_wall_size.x, h_wall_size.y);

        let collider = Collider::compound(vec![
            (top_wall, 0.0, w_wall.clone()),
            (bottom_wall, 0.0, w_wall),
            (left_wall, 0.0, h_wall.clone()),
            (right_wall, 0.0, h_wall),
        ]);

        commands
            .entity(entity)
            .insert((Boundary(rect), RigidBody::Static, collider));
    }
}
