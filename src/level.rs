mod location;

use avian2d::prelude::*;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

use crate::object::Object;

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
             mut q: Query<(&TiledName, &mut Transform)>,
             mut commands: Commands| {
                let entity = layer_created.event().origin;
                let Ok((name, mut transform)) = q.get_mut(entity) else {
                    return;
                };

                let Some(layer) = Layer::from_tiled_name(name) else {
                    return;
                };

                commands.entity(entity).insert(layer);
                transform.translation.z = layer.id() as f32;
            },
        )
        .observe(
            |object_created: On<TiledEvent<ObjectCreated>>,
             layer_q: Query<&TiledName, With<TiledLayer>>,
             mut commands: Commands| {
                // Помещаем объект на слой
                if let Some(layer_entity) = object_created.get_layer_entity()
                    && let Ok(name) = layer_q.get(layer_entity)
                    && let Some(layer) = Layer::from_tiled_name(name)
                {
                    commands.entity(object_created.event().origin).insert(layer);
                }
            },
        )
        .observe(
            |collider_created: On<TiledEvent<ColliderCreated>>,
             mut q: Query<(&mut CollisionLayers, Option<&TiledColliderOf>)>,
             layer_q: Query<&TiledName, With<TiledLayer>>,
             parent_q: Query<&TiledObject>,
             mut commands: Commands| {
                // Коллайдер создаётся отдельно дочкой. Помещаем на слой и применяем физику
                let Some(layer_entity) = collider_created.get_layer_entity() else {
                    return;
                };

                let Ok(name) = layer_q.get(layer_entity) else {
                    return;
                };

                let Some(layer) = Layer::from_tiled_name(name) else {
                    return;
                };

                commands.entity(collider_created.event().origin).insert((
                    RigidBody::Static,
                    layer,
                    Object,
                ));

                let Ok((mut collision_layers, maybe_tiled_collider_of)) =
                    q.get_mut(collider_created.event().origin)
                else {
                    return;
                };

                let layer_mask = layer.into();
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

    fn from_tiled_name(name: &TiledName) -> Option<Self> {
        name.0
            .strip_prefix("Layer ")
            .and_then(|id| id.parse::<u32>().ok())
            .map(Self::new)
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

////////////////////////////////////////////////////////////////////////////////

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layer_from_tiled_name_parses_gameplay_layers() {
        assert_eq!(
            Layer::from_tiled_name(&TiledName("Layer 1".to_string())).map(Layer::id),
            Some(1)
        );
        assert_eq!(
            Layer::from_tiled_name(&TiledName("Layer 2".to_string())).map(Layer::id),
            Some(2)
        );
        assert_eq!(
            Layer::from_tiled_name(&TiledName("Layer 3".to_string())).map(Layer::id),
            Some(3)
        );
    }

    #[test]
    fn layer_from_tiled_name_ignores_non_gameplay_layers() {
        assert!(Layer::from_tiled_name(&TiledName("Background".to_string())).is_none());
        assert!(Layer::from_tiled_name(&TiledName("Objects".to_string())).is_none());
        assert!(Layer::from_tiled_name(&TiledName("Layer x".to_string())).is_none());
    }
}
