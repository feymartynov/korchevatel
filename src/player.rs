use avian2d::prelude::*;
use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
#[require(Transform, Visibility, Collider)]
pub struct Player;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
#[require(Transform)]
#[reflect(Component)]
pub struct PlayerSpawnPoint;
