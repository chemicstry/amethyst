//! Terrain example

use amethyst::{
    prelude::*,
    assets::{PrefabLoader, RonFormat, PrefabLoaderSystemDesc, Handle},
    renderer::{
        rendy::mesh::{Position, TexCoord, Normal},
        plugins::{RenderToWindow, RenderShaded3D, RenderTerrainShaded, RenderSkybox},
        types::DefaultBackend,
        RenderingBundle,
        palette::Srgb,
        Mesh
    },
    controls::{FlyControlBundle, HideCursor},
    core::{
        transform::{Transform, TransformBundle}, Time
    },
    derive::SystemDesc,
    ecs::prelude::{
        System, SystemData, WorldExt, WriteStorage, Join, ReadStorage
    },
    input::{is_key_down, is_mouse_button_down, InputBundle, StringBindings},
    winit::{MouseButton, VirtualKeyCode},
    utils::{application_root_dir, scene::BasicScenePrefab}
};

type MyPrefabData = BasicScenePrefab<(Vec<Position>, Vec<Normal>, Vec<TexCoord>)>;

pub struct ExampleState;

impl SimpleState for ExampleState {
    fn on_start(&mut self, data: StateData<'_, GameData<'_, '_>>) {
        let handle = data.world.exec(|loader: PrefabLoader<'_, MyPrefabData>| {
            loader.load("prefab/terrain.ron", RonFormat, ())
        });
        data.world.create_entity().with(handle).build();
    }

    fn handle_event(
        &mut self,
        data: StateData<'_, GameData<'_, '_>>,
        event: StateEvent,
    ) -> SimpleTrans {
        let StateData { world, .. } = data;
        if let StateEvent::Window(event) = &event {
            if is_key_down(&event, VirtualKeyCode::Escape) {
                let mut hide_cursor = world.write_resource::<HideCursor>();
                hide_cursor.hide = false;
            } else if is_mouse_button_down(&event, MouseButton::Left) {
                let mut hide_cursor = world.write_resource::<HideCursor>();
                hide_cursor.hide = true;
            }
        }
        Trans::None
    }
}

fn main() -> amethyst::Result<()> {
    amethyst::start_logger(Default::default());

    let app_root = application_root_dir()?;
    let display_config_path = app_root.join("examples/terrain/config/display.ron");
    let key_bindings_path = app_root.join("examples/fly_camera/config/input.ron");

    let assets_dir = app_root.join("examples/assets/");

    let game_data = GameDataBuilder::default()
        .with_system_desc(PrefabLoaderSystemDesc::<MyPrefabData>::default(), "", &[])
        .with_system_desc(ExampleSystem::default(), "", &[])
        .with_bundle(
            FlyControlBundle::<StringBindings>::new(
                Some(String::from("move_x")),
                Some(String::from("move_y")),
                Some(String::from("move_z")),
            )
            .with_sensitivity(0.1, 0.1),
        )?
        .with_bundle(TransformBundle::new().with_dep(&["fly_movement"]))?
        .with_bundle(
            InputBundle::<StringBindings>::new().with_bindings_from_file(&key_bindings_path)?,
        )?
        .with_bundle(
            RenderingBundle::<DefaultBackend>::new()
                .with_plugin(
                    RenderToWindow::from_config_path(display_config_path)?
                        .with_clear([0.1, 0.2, 0.3, 1.0]),
                )
                .with_plugin(RenderTerrainShaded::default())
                //.with_plugin(RenderShaded3D::default())
        )?;

    let mut game = Application::new(assets_dir, ExampleState, game_data)?;
    game.run();
    Ok(())
}


#[derive(Default, SystemDesc)]
struct ExampleSystem;

impl<'a> System<'a> for ExampleSystem {
    type SystemData = (
        ReadStorage<'a, Handle<Mesh>>,
        WriteStorage<'a, Transform>,
    );

    fn run(&mut self, data: Self::SystemData) {
        let (handles, mut transforms) = data;
        for (handle, mut transform) in (&handles, &mut transforms).join() {
            //transform.append_rotation_x_axis(0.1);
        }
    }
}
