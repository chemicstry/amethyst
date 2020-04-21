use amethyst_core::ecs::{Component, DenseVecStorage};
use amethyst_assets::Handle;
use amethyst_rendy::{
    Mesh, Texture,
};

#[derive(Debug, Default)]
pub struct TerrainMaterial {
    splat_map: Handle<Texture>,
    // Loaded as 2D array texture
    textures: Handle<Texture>,
}

impl Component for TerrainMaterial {
    type Storage = DenseVecStorage<Self>;
}
