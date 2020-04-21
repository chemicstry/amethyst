use amethyst_core::{
    ecs::{
        DispatcherBuilder, Entities, Join, Read, ReadExpect, ReadStorage, System, SystemData, World,
    },
    geometry::{Plane, Ray},
    math::{self, clamp, convert, Matrix4, Point2, Point3, Vector2, Vector3, Vector4},
    transform::Transform,
    Hidden,
};

use amethyst_assets::{AssetStorage, Handle};
use amethyst_rendy::{
    batch::{GroupIterator, OneLevelBatch, OrderedOneLevelBatch},
    bundle::{RenderOrder, RenderPlan, RenderPlugin, Target},
    camera::{ActiveCamera, Camera, Projection},
    pipeline::{PipelineDescBuilder, PipelinesBuilder},
    pod::{IntoPod, VertexArgs},
    rendy::{
        command::{QueueId, RenderPassEncoder},
        factory::Factory,
        graph::{
            render::{PrepareResult, RenderGroup, RenderGroupDesc},
            GraphContext, NodeBuffer, NodeImage,
        },
        hal::{
            self,
            device::Device,
            pso::{self, ShaderStageFlags},
            format::Format,
        },
        mesh::{AsVertex, Position, TexCoord, VertexFormat},
        shader::{Shader, ShaderSetBuilder, SpirvShader},
    },
    resources::Tint as TintComponent,
    sprite::{SpriteRender, SpriteSheet},
    sprite_visibility::SpriteVisibility,
    submodules::{
        gather::CameraGatherer, DynamicUniform, DynamicVertexBuffer, FlatEnvironmentSub, TextureId,
        TextureSub,
    },
    types::{Backend, Texture},
    util, ChangeDetection,
};

use crate::TerrainMaterial;

use glsl_layout::*;

#[cfg(feature = "profiler")]
use thread_profiler::profile_scope;

lazy_static::lazy_static! {
    static ref VERTEX: SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../compiled/terrain.vert.spv"),
        ShaderStageFlags::VERTEX,
        "main",
    ).unwrap();

    static ref FRAGMENT: SpirvShader = SpirvShader::from_bytes(
        include_bytes!("../compiled/terrain.frag.spv"),
        ShaderStageFlags::FRAGMENT,
        "main",
    ).unwrap();

    static ref SHADERS: ShaderSetBuilder = ShaderSetBuilder::default()
        .with_vertex(&*VERTEX).unwrap()
        .with_fragment(&*FRAGMENT).unwrap();
}

#[derive(Clone, Copy, Debug, AsStd140)]
#[repr(C, align(4))]
pub struct UniformArgs {
    pub scale: float,
}

#[derive(Debug, Default)]
pub struct DrawTerrainDesc;

impl DrawTerrainDesc {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<B: Backend> RenderGroupDesc<B, World> for DrawTerrainDesc
{
    fn build(
        self,
        _ctx: &GraphContext<B>,
        factory: &mut Factory<B>,
        _queue: QueueId,
        _aux: &World,
        framebuffer_width: u32,
        framebuffer_height: u32,
        subpass: hal::pass::Subpass<'_, B>,
        _buffers: Vec<NodeBuffer>,
        _images: Vec<NodeImage>,
    ) -> Result<Box<dyn RenderGroup<B, World>>, failure::Error> {
        #[cfg(feature = "profiler")]
        profile_scope!("build");

        let env = DynamicUniform::new(factory, pso::ShaderStageFlags::VERTEX)?;
        let vertex = DynamicVertexBuffer::new();

        let (pipeline, pipeline_layout) = build_terrain_pipeline(
            factory,
            subpass,
            framebuffer_width,
            framebuffer_height,
            vec![env.raw_layout()],
        )?;

        let mut vertex_format = vec![
            Position::vertex(),
            TexCoord::vertex(),
        ];

        Ok(Box::new(DrawTerrain {
            pipeline,
            pipeline_layout,
            env,
            batches: Default::default(),
            vertex_format,
            vertex,
            vertex_count: 0,
            change: Default::default(),
        }))
    }
}

#[derive(Debug)]
pub struct DrawTerrain<B: Backend> {
    pipeline: B::GraphicsPipeline,
    pipeline_layout: B::PipelineLayout,
    env: DynamicUniform<B, UniformArgs>,
    batches: TwoLevelBatch<TerrainMaterial, u32, SmallVec<[VertexArgs; 4]>>,
    vertex_format: Vec<VertexFormat>,
    models: DynamicVertexBuffer<B, VertexArgs>,
    change: ChangeDetection,
}

impl<B: Backend> RenderGroup<B, World> for DrawTerrain<B>
{
    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        world: &World,
    ) -> PrepareResult {
        #[cfg(feature = "profiler")]
        profile_scope!("prepare");

        let (
            mesh_storage,
            mesh_handles,
            terrain_textures,
            transforms,
            tints,
        ) = <(
            Read<'_, AssetStorage<Mesh>>,
            ReadStorage<'_, Handle<Mesh>>,
            ReadStorage<'_, TerrainMaterial>,
            ReadStorage<'_, Transform>,
            ReadStorage<'_, Tint>,
        )>::fetch(resources);

        let mut changed = false;

        (&mesh_handles, &terrain_tiles, &transforms, &tints.maybe())
            .join()
            .map(|(mat, mesh, tform, tint)| {
                ((mat, mesh.id()), VertexArgs::from_object_data(tform, tint))
            })
            .for_each_group(|(mat, mesh_id), data| {
                if mesh_storage.contains_id(mesh_id) {
                    // if let Some((mat, _)) = materials_ref.insert(factory, resources, mat) {
                    //     statics_ref.insert(mat, mesh_id, data.drain(..));
                    // }
                    statics_ref.insert(mat, mesh_id, data.drain(..));
                }
            });

        let uniform = UniformArgs { scale: 1.0 };
        self.env.write(factory, index, scale.std140());

        self.change.prepare_result(index, changed)
    }

    fn draw_inline(
        &mut self,
        mut encoder: RenderPassEncoder<'_, B>,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        _world: &World,
    ) {
        #[cfg(feature = "profiler")]
        profile_scope!("draw");

        let layout = &self.pipeline_layout;
        encoder.bind_graphics_pipeline(&self.pipeline);
        self.env.bind(index, layout, 0, &mut encoder);

        self.vertex.bind(index, 0, 0, &mut encoder);
        // for (&tex, range) in self.sprites.iter() {
        //     if self.textures.loaded(tex) {
        //         self.textures.bind(layout, 1, tex, &mut encoder);

        //         unsafe {
        //             encoder.draw(0..4, range);
        //         }
        //     }
        // }
    }

    fn dispose(self: Box<Self>, factory: &mut Factory<B>, _aux: &World) {
        unsafe {
            factory.device().destroy_graphics_pipeline(self.pipeline);
            factory
                .device()
                .destroy_pipeline_layout(self.pipeline_layout);
        }
    }
}

fn build_terrain_pipeline<B: Backend>(
    factory: &Factory<B>,
    subpass: hal::pass::Subpass<'_, B>,
    framebuffer_width: u32,
    framebuffer_height: u32,
    layouts: Vec<&B::DescriptorSetLayout>,
) -> Result<(B::GraphicsPipeline, B::PipelineLayout), failure::Error> {
    let pipeline_layout = unsafe {
        factory
            .device()
            .create_pipeline_layout(layouts, None as Option<(_, _)>)
    }?;

    // Load the shaders
    let shader_vertex = unsafe { VERTEX.module(factory).unwrap() };
    let shader_fragment = unsafe { FRAGMENT.module(factory).unwrap() };

    // Build the pipeline
    let pipes = PipelinesBuilder::new()
        .with_pipeline(
            PipelineDescBuilder::new()
                // This Pipeline uses our custom vertex description and does not use instancing
                .with_vertex_desc(&[(CustomArgs::vertex(), pso::VertexInputRate::Vertex)])
                .with_input_assembler(pso::InputAssemblerDesc::new(hal::Primitive::TriangleList))
                // Add the shaders
                .with_shaders(util::simple_shader_set(
                    &shader_vertex,
                    Some(&shader_fragment),
                ))
                .with_layout(&pipeline_layout)
                .with_subpass(subpass)
                .with_framebuffer_size(framebuffer_width, framebuffer_height)
                // We are using alpha blending
                .with_blend_targets(vec![pso::ColorBlendDesc {
                    mask: pso::ColorMask::ALL,
                    blend: Some(pso::BlendState::ALPHA),
                }]),
        )
        .build(factory, None);

    // Destoy the shaders once loaded
    unsafe {
        factory.destroy_shader_module(shader_vertex);
        factory.destroy_shader_module(shader_fragment);
    }

    // Handle the Errors
    match pipes {
        Err(e) => {
            unsafe {
                factory.device().destroy_pipeline_layout(pipeline_layout);
            }
            Err(e)
        }
        Ok(mut pipes) => Ok((pipes.remove(0), pipeline_layout)),
    }
}

#[derive(Debug)]
pub struct RenderTerrain {
    target: Target,
}

impl Default for RenderTerrain {
    fn default() -> Self {
        Self {
            target: Target::Main
        }
    }
}

impl RenderTerrain {
    /// Select render target on which Tiles should be rendered.
    #[must_use]
    pub fn with_target(mut self, target: Target) -> Self {
        self.target = target;
        self
    }
}

type SetupData<'a> = (
    ReadStorage<'a, TerrainMaterial>,
    ReadStorage<'a, Hidden>,
);

impl<B: Backend> RenderPlugin<B> for RenderTerrain
{
    fn on_build<'a, 'b>(
        &mut self,
        world: &mut World,
        builder: &mut DispatcherBuilder<'a, 'b>,
    ) -> Result<(), amethyst_error::Error> {
        SetupData::setup(world);

        Ok(())
    }

    fn on_plan(
        &mut self,
        plan: &mut RenderPlan<B>,
        _factory: &mut Factory<B>,
        _res: &World,
    ) -> Result<(), amethyst_error::Error> {
        plan.extend_target(self.target, |ctx| {
            ctx.add(
                RenderOrder::BeforeTransparent,
                DrawTerrainDesc::default().builder(),
            )?;
            Ok(())
        });
        Ok(())
    }
}
