#![allow(clippy::type_complexity)]

#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate serde;

mod camera;
mod character;
#[cfg(debug_assertions)]
mod debug;
mod hit;
mod level;
mod mob;
mod movement;
mod object;
mod player;

use std::env;

use anyhow::{Context, Result};
use avian2d::prelude::*;
use bevy::asset::AssetMetaCheck;
use bevy::prelude::*;
use bevy_ecs_tiled::prelude::*;

fn main() -> Result<()> {
    self::character::Registry::load().context("Load character registry")?;
    let mut app = App::new();

    app.add_plugins((
        DefaultPlugins
            .set(ImagePlugin::default_nearest())
            .set(AssetPlugin {
                // Wasm builds will check for meta files (that don't exist) if this isn't set.
                // This causes errors and even panics on web build on itch.
                // See https://github.com/bevyengine/bevy_github_ci_template/issues/48.
                meta_check: AssetMetaCheck::Never,
                ..default()
            })
            .set(WindowPlugin {
                primary_window: Window {
                    title: "Korchevatel".to_string(),
                    fit_canvas_to_parent: true,
                    ..default()
                }
                .into(),
                ..default()
            }),
        #[cfg(debug_assertions)]
        debug::DebugPlugin,
        camera::plugin,
        level::plugin,
        character::plugin,
        player::plugin,
        movement::plugin,
        mob::plugin,
        object::plugin,
        hit::plugin,
    ));

    let mut path = env::current_dir().expect("current dir");
    path.push("assets");
    path.push("korchevatel_types.json");

    app.add_plugins((
        TiledPlugin(TiledPluginConfig {
            tiled_types_export_file: Some(path),
            tiled_types_filter: TiledFilter::from(
                regex::RegexSet::new([r"^korchevatel::.*"]).expect("regex"),
            ),
        }),
        TiledPhysicsPlugin::<TiledPhysicsAvianBackend>::default(),
        PhysicsPlugins::default().with_length_unit(200.0),
    ));

    app.insert_resource(ClearColor(Color::srgb_u8(64, 64, 64)));
    app.insert_resource(Gravity(Vec2::NEG_Y * 1000.0));
    app.run();
    Ok(())
}
