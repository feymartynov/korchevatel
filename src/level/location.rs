use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

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
    q: Query<
        (&TiledImage, &ChildOf, &GlobalTransform),
        Or<(Added<TiledImage>, Changed<GlobalTransform>)>,
    >,
    mut commands: Commands,
) {
    for (tiled_image, child_of, global_transform) in q {
        let top_left = global_transform.translation().truncate();
        let right_bottom = Vec2::new(tiled_image.base_size.x, -tiled_image.base_size.y) + top_left;
        let rect = Rect::from_corners(top_left, right_bottom);
        commands.entity(child_of.0).insert(Boundary(rect));
    }
}
