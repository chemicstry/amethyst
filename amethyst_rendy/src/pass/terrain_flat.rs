use super::base_3d::*;
use crate::{mtl::{TexAlbedo, TexSplat}, skinning::JointCombined};
use rendy::{
    mesh::{AsVertex, Position, TexCoord, VertexFormat},
    shader::SpirvShader,
};

/// Implementation of `Base3DPassDef` to describe a flat 3D pass
#[derive(Debug)]
pub struct TerrainFlatPassDef;
impl Base3DPassDef for TerrainFlatPassDef {
    const NAME: &'static str = "TerrainFlat";
    type TextureSet = (
        TexAlbedo,
        TexSplat,
    );
    fn vertex_shader() -> &'static SpirvShader {
        &super::TERRAIN_POS_TEX_VERTEX
    }
    fn vertex_skinned_shader() -> &'static SpirvShader {
        &super::POS_TEX_SKIN_VERTEX
    }
    fn fragment_shader() -> &'static SpirvShader {
        &super::TERRAIN_FLAT_FRAGMENT
    }
    fn base_format() -> Vec<VertexFormat> {
        vec![Position::vertex(), TexCoord::vertex()]
    }
    fn skinned_format() -> Vec<VertexFormat> {
        vec![
            Position::vertex(),
            TexCoord::vertex(),
            JointCombined::vertex(),
        ]
    }
}

/// Describes a Flat 3D pass
pub type DrawTerrainFlatDesc<B> = DrawBase3DDesc<B, TerrainFlatPassDef>;
/// Draws a Flat 3D pass
pub type DrawTerrainFlat<B> = DrawBase3D<B, TerrainFlatPassDef>;
/// Describes a Flat 3D pass with Transparency
pub type DrawTerrainFlatTransparentDesc<B> = DrawBase3DTransparentDesc<B, TerrainFlatPassDef>;
/// Draws a Flat 3D pass with transpency.
pub type DrawTerrainFlatTransparent<B> = DrawBase3DTransparent<B, TerrainFlatPassDef>;
