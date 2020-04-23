use super::base_3d::*;
use crate::{mtl::{TexAlbedo, TexEmission, TexSplat}, skinning::JointCombined};
use rendy::{
    mesh::{AsVertex, Position, TexCoord, VertexFormat, Normal},
    shader::SpirvShader,
};

/// Implementation of `Base3DPassDef` to describe a Shaded 3D pass
#[derive(Debug)]
pub struct TerrainShadedPassDef;
impl Base3DPassDef for TerrainShadedPassDef {
    const NAME: &'static str = "TerrainShaded";
    type TextureSet = (
        TexAlbedo,
        TexEmission,
        TexSplat,
    );
    fn vertex_shader() -> &'static SpirvShader {
        &super::TERRAIN_POS_NORM_TEX_VERTEX
    }
    fn vertex_skinned_shader() -> &'static SpirvShader {
        &super::POS_TEX_SKIN_VERTEX
    }
    fn fragment_shader() -> &'static SpirvShader {
        &super::TERRAIN_SHADED_FRAGMENT
    }
    fn base_format() -> Vec<VertexFormat> {
        vec![Position::vertex(), Normal::vertex(), TexCoord::vertex()]
    }
    fn skinned_format() -> Vec<VertexFormat> {
        vec![
            Position::vertex(),
            TexCoord::vertex(),
            JointCombined::vertex(),
        ]
    }
}

/// Describes a Shaded 3D pass
pub type DrawTerrainShadedDesc<B> = DrawBase3DDesc<B, TerrainShadedPassDef>;
/// Draws a Shaded 3D pass
pub type DrawTerrainShaded<B> = DrawBase3D<B, TerrainShadedPassDef>;
/// Describes a Shaded 3D pass with Transparency
pub type DrawTerrainShadedTransparentDesc<B> = DrawBase3DTransparentDesc<B, TerrainShadedPassDef>;
/// Draws a Shaded 3D pass with transpency.
pub type DrawTerrainShadedTransparent<B> = DrawBase3DTransparent<B, TerrainShadedPassDef>;
