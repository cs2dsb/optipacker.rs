use bevy::{
    prelude::*,
    asset::AssetServerSettings,
};
use bevy_asset_loader::AssetLoader;

mod packed;
use packed::{
    AtlasSprites,
    TextureAtlasInfo,
};

const ASSETS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/assets");

#[derive(Clone, Eq, PartialEq, Debug, Hash)]
enum MyStates {
    AssetLoading,
    Next,
}

fn main() {
    let mut app = App::build();

    app
        .insert_resource(WindowDescriptor {
            title: env!("CARGO_PKG_NAME").to_string().to_string(),
            width: 800.,
            height: 600.,
            .. Default::default()
        })
        // This is required because the assets for the example are not in the root of the crate
        .insert_resource(AssetServerSettings {
            asset_folder: ASSETS_DIR.to_string(),
        })
        .add_plugins(DefaultPlugins)
        .add_state(MyStates::AssetLoading);

    let mut asset_loader = AssetLoader::new(MyStates::AssetLoading, MyStates::Next);
    asset_loader = AtlasSprites::build(&mut app, MyStates::AssetLoading, asset_loader);
    asset_loader.build(&mut app);

    // Just for testing
    app
        .add_system_set(
            SystemSet::on_enter(MyStates::Next)
                .with_system(setup.system())
                .with_system(AtlasSprites::spawn_all.system()))
        .add_system_set(
            SystemSet::on_update(MyStates::Next)
                .with_system(animate.system()));

    app.run();

}

fn setup(mut commands: Commands) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
}

fn animate(
    time: Res<Time>,
    mut query: Query<(&mut TextureAtlasSprite, &mut TextureAtlasInfo)>) {
    for (mut sprite, mut info) in query.iter_mut() {
        info.frame_time += time.delta_seconds();
        if info.frame_time > 0.25 {
            info.frame_time -= 0.25;
            sprite.index = sprite.index + 1;
            if sprite.index >= info.frames {
                sprite.index = 0;
            }
        }
    }
}
