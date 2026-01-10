use avian2d::prelude::*;
use bevy::diagnostic::FrameTimeDiagnosticsPlugin;
use bevy::input::common_conditions::input_just_pressed;
use bevy::prelude::*;
// use bevy_inspector_egui::bevy_egui::EguiPlugin;
// use bevy_inspector_egui::quick::{FilterQueryInspectorPlugin, WorldInspectorPlugin};

pub struct DebugPlugin;

impl Plugin for DebugPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PhysicsDiagnosticsPlugin,
            PhysicsDiagnosticsUiPlugin,
            FrameTimeDiagnosticsPlugin::default(),
            // EguiPlugin::default(),
            // FilterQueryInspectorPlugin::<With<TypeToInspect>>::default(),
            // WorldInspectorPlugin::new(),
        ));

        app.insert_resource(PhysicsDiagnosticsUiSettings {
            enabled: false,
            ..default()
        });

        app.add_systems(Startup, setup_key_instructions);

        app.add_systems(
            Update,
            (
                toggle_diagnostics_ui.run_if(input_just_pressed(KeyCode::KeyU)),
                toggle_paused.run_if(input_just_pressed(KeyCode::KeyP)),
                step.run_if(physics_paused.and(input_just_pressed(KeyCode::Enter))),
            ),
        );
    }

    fn finish(&self, app: &mut App) {
        if !app.is_plugin_added::<PhysicsDebugPlugin>() {
            app.add_plugins(PhysicsDebugPlugin);
        }
    }
}

fn toggle_diagnostics_ui(mut settings: ResMut<PhysicsDiagnosticsUiSettings>) {
    settings.enabled = !settings.enabled;
}

fn physics_paused(time: Res<Time<Physics>>) -> bool {
    time.is_paused()
}

fn toggle_paused(mut time: ResMut<Time<Physics>>) {
    if time.is_paused() {
        time.unpause();
    } else {
        time.pause();
    }
}

/// Advances the physics simulation by one `Time<Fixed>` time step.
fn step(mut physics_time: ResMut<Time<Physics>>, fixed_time: Res<Time<Fixed>>) {
    physics_time.advance_by(fixed_time.delta());
}

fn setup_key_instructions(mut commands: Commands) {
    commands.spawn((
        Text::new("U: Diagnostics UI | P: Pause/Unpause | Enter: Step"),
        TextFont {
            font_size: 10.0,
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(5.0),
            right: Val::Px(5.0),
            ..default()
        },
    ));
}
