mod location;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

pub use self::location::Boundary as LocationBoundary;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Layer>();
    app.add_plugins(location::plugin);
    app.add_systems(Startup, startup);
}

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            TiledMap(asset_server.load("level.tmx")),
            TilemapAnchor::Center,
        ))
        .observe(
            |layer_created: On<TiledEvent<LayerCreated>>,
             mut q: Query<&mut Transform>,
             mut commands: Commands| {
                if let Some(id) = layer_created.get_layer_id()
                    && id > 0
                {
                    let entity = layer_created.event().origin;
                    commands.entity(entity).insert(Layer::new(id));

                    if let Ok(mut transform) = q.get_mut(entity) {
                        transform.translation.z = id as f32;
                    }
                }
            },
        )
        .observe(
            |object_created: On<TiledEvent<ObjectCreated>>, mut commands: Commands| {
                // Помещаем объект на слой
                if let Some(id) = object_created.event().get_layer_id() {
                    commands
                        .entity(object_created.event().origin)
                        .insert(Layer::new(id));
                }
            },
        )
        .observe(
            |collider_created: On<TiledEvent<ColliderCreated>>,
             mut q: Query<(&mut CollisionLayers, Option<&TiledColliderOf>)>,
             parent_q: Query<&TiledObject>,
             mut commands: Commands| {
                // Коллайдер создаётся отдельно дочкой. Помещаем на слой и применяем физику
                let Some(layer_id) = collider_created.event().get_layer_id() else {
                    return;
                };

                commands
                    .entity(collider_created.event().origin)
                    .insert((RigidBody::Static, Layer::new(layer_id)));

                let Ok((mut collision_layers, maybe_tiled_collider_of)) =
                    q.get_mut(collider_created.event().origin)
                else {
                    return;
                };

                let layer_mask = Layer::new(layer_id).into();
                collision_layers.memberships = layer_mask;
                collision_layers.filters = layer_mask;

                // Центр массы ошибочно считается как левый нижний угол. Ставим на центр объекта
                if let Some(tiled_collider_of) = maybe_tiled_collider_of
                    && let Ok(
                        TiledObject::Tile { width, height }
                        | TiledObject::Rectangle { width, height }
                        | TiledObject::Ellipse { width, height },
                    ) = parent_q.get(tiled_collider_of.entity())
                {
                    let center = Vec2::new(width / 2.0, height / 2.0);

                    commands
                        .entity(collider_created.event().origin)
                        .insert((CenterOfMass(center), NoAutoCenterOfMass));
                }
            },
        );
}

/// Слой, на котором находится объект
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Reflect)]
#[reflect(Component)]
pub struct Layer {
    mask: u32,
}

impl Layer {
    pub const BOTTOM: Layer = Layer { mask: 0b10 };

    pub fn new(id: u32) -> Self {
        debug_assert!((1..=32).contains(&id), "Layer id must be in range 1..=32");
        Self { mask: (1 << id) }
    }

    pub fn id(self) -> u32 {
        self.mask.trailing_zeros()
    }

    pub fn next(self) -> Self {
        Self {
            mask: self.mask.rotate_left(1),
        }
    }

    pub fn prev(self) -> Self {
        Self {
            mask: self.mask.rotate_right(1),
        }
    }
}

impl PhysicsLayer for Layer {
    fn to_bits(&self) -> u32 {
        self.mask
    }

    fn all_bits() -> u32 {
        u32::MAX
    }
}
