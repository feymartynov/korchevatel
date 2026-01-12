use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

pub(super) fn plugin(app: &mut App) {
    app.register_type::<Layer>();
    app.add_systems(Startup, startup);
}

fn startup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands
        .spawn((
            TiledMap(asset_server.load("level.tmx")),
            TilemapAnchor::Center,
        ))
        .observe(
            |layer_created: On<TiledEvent<LayerCreated>>, mut commands: Commands| {
                if let Some(id) = layer_created.get_layer_id()
                    && id > 0
                {
                    commands
                        .entity(layer_created.event().origin)
                        .insert(Layer::new(id));
                }
            },
        )
        .observe(
            |object_created: On<TiledEvent<ObjectCreated>>, mut commands: Commands| {
                if let Some(id) = object_created.event().get_layer_id() {
                    commands
                        .entity(object_created.event().origin)
                        .insert(Layer::new(id));
                }
            },
        )
        .observe(
            |collider_created: On<TiledEvent<ColliderCreated>>,
             mut collision_layers_q: Query<&mut CollisionLayers>,
             mut commands: Commands| {
                commands
                    .entity(collider_created.event().origin)
                    .insert(RigidBody::Static);

                if let Some(layer_id) = collider_created.event().get_layer_id()
                    && let Ok(mut collision_layers) =
                        collision_layers_q.get_mut(collider_created.event().origin)
                {
                    let layer_mask = Layer::new(layer_id).into();
                    collision_layers.memberships = layer_mask;
                    collision_layers.filters = layer_mask;
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
        self.mask >> 1
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
