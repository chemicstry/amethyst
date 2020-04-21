use amethyst_rendy::{
    batch::{GroupIterator, OrderedTwoLevelBatch, TwoLevelBatch},
    mtl::{FullTextureSet, Material, StaticTextureSet},
    pipeline::{PipelineDescBuilder, PipelinesBuilder},
    pod::{SkinnedVertexArgs, VertexArgs},
    resources::Tint,
    skinning::JointTransforms,
    submodules::{DynamicVertexBuffer, EnvironmentSub, MaterialId, MaterialSub, SkinningSub},
    transparent::Transparent,
    types::{Backend, Mesh},
    util,
    visibility::Visibility,
};
use amethyst_assets::{AssetStorage, Handle};
use amethyst_core::{
    ecs::{Join, Read, ReadExpect, ReadStorage, SystemData, World},
    transform::Transform,
    Hidden, HiddenPropagate,
};
use derivative::Derivative;
use rendy::{
    command::{QueueId, RenderPassEncoder},
    factory::Factory,
    graph::{
        render::{PrepareResult, RenderGroup, RenderGroupDesc},
        GraphContext, NodeBuffer, NodeImage,
    },
    hal::{self, device::Device, pso},
    mesh::{AsVertex, VertexFormat},
    shader::{Shader, SpirvShader},
};
use smallvec::SmallVec;
use std::marker::PhantomData;

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

macro_rules! profile_scope_impl {
    ($string:expr) => {
        #[cfg(feature = "profiler")]
        let _profile_scope = thread_profiler::ProfileScope::new(format!(
            "{} {}: {}",
            module_path!(),
            <T as Base3DPassDef>::NAME,
            $string
        ));
    };
}

/// Draw opaque 3d meshes with specified shaders and texture set
#[derive(Clone, Derivative)]
#[derivative(Debug(bound = ""), Default(bound = ""))]
pub struct DrawTerrainDesc {}

impl DrawTerrainDesc {
    /// Create pass in default configuration
    pub fn new() -> Self {
        Default::default()
    }
}

impl<B: Backend> RenderGroupDesc<B, World> for DrawTerrainDesc<B> {
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
        profile_scope_impl!("build");

        let env = EnvironmentSub::new(
            factory,
            [
                hal::pso::ShaderStageFlags::VERTEX,
                hal::pso::ShaderStageFlags::FRAGMENT,
            ],
        )?;
        let materials = MaterialSub::new(factory)?;

        let mut vertex_format = vec![
            Position::vertex(),
            TexCoord::vertex(),
        ];

        let (mut pipelines, pipeline_layout) = build_pipelines::<B, T>(
            factory,
            subpass,
            framebuffer_width,
            framebuffer_height,
            &vertex_format,
            vec![
                env.raw_layout(),
                materials.raw_layout(),
            ],
        )?;

        vertex_format.sort();

        Ok(Box::new(DrawTerrain::<B, T> {
            pipeline_basic: pipelines.remove(0),
            pipeline_skinned: pipelines.pop(),
            pipeline_layout,
            static_batches: Default::default(),
            skinned_batches: Default::default(),
            vertex_format_base,
            vertex_format_skinned,
            env,
            materials,
            skinning,
            models: DynamicVertexBuffer::new(),
            skinned_models: DynamicVertexBuffer::new(),
            marker: PhantomData,
        }))
    }
}

/// Base implementation of a 3D render pass which can be consumed by actual 3D render passes,
/// such as [pass::pbr::DrawPbr]
#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct DrawTerrain<B: Backend, T: Base3DPassDef> {
    pipeline: B::GraphicsPipeline,
    pipeline_layout: B::PipelineLayout,
    batches: TwoLevelBatch<MaterialId, u32, SmallVec<[VertexArgs; 4]>>,
    vertex_format: Vec<VertexFormat>,
    env: EnvironmentSub<B>,
    materials: MaterialSub<B, T::TextureSet>,
    models: DynamicVertexBuffer<B, VertexArgs>,
}

impl<B: Backend, T: Base3DPassDef> RenderGroup<B, World> for DrawTerrain<B, T> {
    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        resources: &World,
    ) -> PrepareResult {
        profile_scope_impl!("prepare opaque");

        let (
            mesh_storage,
            visibility,
            transparent,
            hiddens,
            hiddens_prop,
            meshes,
            materials,
            transforms,
            tints,
        ) = <(
            Read<'_, AssetStorage<Mesh>>,
            ReadExpect<'_, Visibility>,
            ReadStorage<'_, Transparent>,
            ReadStorage<'_, Hidden>,
            ReadStorage<'_, HiddenPropagate>,
            ReadStorage<'_, Handle<Mesh>>,
            ReadStorage<'_, Handle<Material>>,
            ReadStorage<'_, Transform>,
            ReadStorage<'_, Tint>,
        )>::fetch(resources);

        // Prepare environment
        self.env.process(factory, index, resources);
        self.materials.maintain();

        self.batches.clear_inner();

        let materials_ref = &mut self.materials;
        let batches_ref = &mut self.batches;

        let static_input = || ((&materials, &meshes, &transforms, tints.maybe()), !&joints);
        {
            profile_scope_impl!("prepare");
            (static_input(), &visibility.visible_unordered)
                .join()
                .map(|(((mat, mesh, tform, tint), _), _)| {
                    ((mat, mesh.id()), VertexArgs::from_object_data(tform, tint))
                })
                .for_each_group(|(mat, mesh_id), data| {
                    if mesh_storage.contains_id(mesh_id) {
                        if let Some((mat, _)) = materials_ref.insert(factory, resources, mat) {
                            batches_ref.insert(mat, mesh_id, data.drain(..));
                        }
                    }
                });
        }

        {
            profile_scope_impl!("write");

            self.batches.prune();

            self.models.write(
                factory,
                index,
                self.batches.count() as u64,
                self.batches.data(),
            );
        }
        PrepareResult::DrawRecord
    }

    fn draw_inline(
        &mut self,
        mut encoder: RenderPassEncoder<'_, B>,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        resources: &World,
    ) {
        profile_scope_impl!("draw opaque");

        let mesh_storage = <Read<'_, AssetStorage<Mesh>>>::fetch(resources);
        let models_loc = self.vertex_format.len() as u32;

        encoder.bind_graphics_pipeline(&self.pipeline);
        self.env.bind(index, &self.pipeline_layout, 0, &mut encoder);

        if self.models.bind(index, models_loc, 0, &mut encoder) {
            let mut instances_drawn = 0;
            for (&mat_id, batches) in self.batches.iter() {
                if self.materials.loaded(mat_id) {
                    self.materials
                        .bind(&self.pipeline_layout, 1, mat_id, &mut encoder);
                    for (mesh_id, batch_data) in batches {
                        debug_assert!(mesh_storage.contains_id(*mesh_id));
                        if let Some(mesh) =
                            B::unwrap_mesh(unsafe { mesh_storage.get_by_id_unchecked(*mesh_id) })
                        {
                            mesh.bind_and_draw(
                                0,
                                &self.vertex_format,
                                instances_drawn..instances_drawn + batch_data.len() as u32,
                                &mut encoder,
                            )
                            .unwrap();
                        }
                        instances_drawn += batch_data.len() as u32;
                    }
                }
            }
        }
    }

    fn dispose(mut self: Box<Self>, factory: &mut Factory<B>, _aux: &World) {
        profile_scope_impl!("dispose");
        unsafe {
            factory
                .device()
                .destroy_graphics_pipeline(self.pipeline);
            factory
                .device()
                .destroy_pipeline_layout(self.pipeline_layout);
        }
    }
}

/// Draw transparent mesh with physically based lighting
#[derive(Clone, Derivative)]
#[derivative(Debug(bound = ""), Default(bound = ""))]
pub struct DrawTerrainTransparentDesc;

impl DrawTerrainTransparentDesc {
    pub fn new() -> Self {
        Default::default()
    }
}

impl<B: Backend> RenderGroupDesc<B, World> for DrawTerrainTransparentDesc {
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
        let env = EnvironmentSub::new(
            factory,
            [
                hal::pso::ShaderStageFlags::VERTEX,
                hal::pso::ShaderStageFlags::FRAGMENT,
            ],
        )?;

        let materials = MaterialSub::new(factory)?;

        let mut vertex_format_base = T::base_format();
        let mut vertex_format_skinned = T::skinned_format();

        let (mut pipeline, pipeline_layout) = build_terrain_pipeline(
            factory,
            subpass,
            framebuffer_width,
            framebuffer_height,
            &vertex_format_base,
            &vertex_format_skinned,
            self.skinning,
            true,
            vec![
                env.raw_layout(),
                materials.raw_layout(),
            ],
        )?;

        vertex_format.sort();

        Ok(Box::new(DrawTerrainTransparent::<B, T> {
            pipeline,
            pipeline_layout,
            batches: Default::default(),
            vertex_format,
            env,
            materials,
            models: DynamicVertexBuffer::new(),
            change: Default::default(),
        }))
    }
}

/// Draw transparent mesh with physically based lighting
#[derive(Derivative)]
#[derivative(Debug(bound = ""))]
pub struct DrawTerrainTransparent<B: Backend> {
    pipeline: B::GraphicsPipeline,
    pipeline_layout: B::PipelineLayout,
    batches: OrderedTwoLevelBatch<MaterialId, u32, VertexArgs>,
    vertex_format: Vec<VertexFormat>,
    env: EnvironmentSub<B>,
    materials: MaterialSub<B, FullTextureSet>,
    models: DynamicVertexBuffer<B, VertexArgs>,
    change: util::ChangeDetection,
}

impl<B: Backend> RenderGroup<B, World> for DrawTerrainTransparent<B> {
    fn prepare(
        &mut self,
        factory: &Factory<B>,
        _queue: QueueId,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        resources: &World,
    ) -> PrepareResult {
        profile_scope_impl!("prepare transparent");

        let (mesh_storage, visibility, meshes, materials, transforms, joints, tints) =
            <(
                Read<'_, AssetStorage<Mesh>>,
                ReadExpect<'_, Visibility>,
                ReadStorage<'_, Handle<Mesh>>,
                ReadStorage<'_, Handle<Material>>,
                ReadStorage<'_, Transform>,
                ReadStorage<'_, JointTransforms>,
                ReadStorage<'_, Tint>,
            )>::fetch(resources);

        // Prepare environment
        self.env.process(factory, index, resources);
        self.materials.maintain();

        self.static_batches.swap_clear();
        self.skinned_batches.swap_clear();

        let materials_ref = &mut self.materials;
        let skinning_ref = &mut self.skinning;
        let batches_ref = &mut self.static_batches;
        let skinned_ref = &mut self.skinned_batches;
        let mut changed = false;

        let mut joined = ((&materials, &meshes, &transforms, tints.maybe()), !&joints).join();
        visibility
            .visible_ordered
            .iter()
            .filter_map(|e| joined.get_unchecked(e.id()))
            .map(|((mat, mesh, tform, tint), _)| {
                ((mat, mesh.id()), VertexArgs::from_object_data(tform, tint))
            })
            .for_each_group(|(mat, mesh_id), data| {
                if mesh_storage.contains_id(mesh_id) {
                    if let Some((mat, this_changed)) = materials_ref.insert(factory, resources, mat)
                    {
                        changed = changed || this_changed;
                        batches_ref.insert(mat, mesh_id, data.drain(..));
                    }
                }
            });

        if self.pipeline_skinned.is_some() {
            let mut joined = (&materials, &meshes, &transforms, tints.maybe(), &joints).join();

            visibility
                .visible_ordered
                .iter()
                .filter_map(|e| joined.get_unchecked(e.id()))
                .map(|(mat, mesh, tform, tint, joints)| {
                    (
                        (mat, mesh.id()),
                        SkinnedVertexArgs::from_object_data(
                            tform,
                            tint,
                            skinning_ref.insert(joints),
                        ),
                    )
                })
                .for_each_group(|(mat, mesh_id), data| {
                    if mesh_storage.contains_id(mesh_id) {
                        if let Some((mat, this_changed)) =
                            materials_ref.insert(factory, resources, mat)
                        {
                            changed = changed || this_changed;
                            skinned_ref.insert(mat, mesh_id, data.drain(..));
                        }
                    }
                });
        }

        self.models.write(
            factory,
            index,
            self.static_batches.count() as u64,
            Some(self.static_batches.data()),
        );

        self.skinned_models.write(
            factory,
            index,
            self.skinned_batches.count() as u64,
            Some(self.skinned_batches.data()),
        );

        self.skinning.commit(factory, index);

        changed = changed || self.static_batches.changed();
        changed = changed || self.skinned_batches.changed();

        self.change.prepare_result(index, changed)
    }

    fn draw_inline(
        &mut self,
        mut encoder: RenderPassEncoder<'_, B>,
        index: usize,
        _subpass: hal::pass::Subpass<'_, B>,
        resources: &World,
    ) {
        profile_scope_impl!("draw transparent");

        let mesh_storage = <Read<'_, AssetStorage<Mesh>>>::fetch(resources);
        let layout = &self.pipeline_layout;
        let encoder = &mut encoder;

        let models_loc = self.vertex_format_base.len() as u32;
        let skin_models_loc = self.vertex_format_skinned.len() as u32;

        encoder.bind_graphics_pipeline(&self.pipeline_basic);
        self.env.bind(index, layout, 0, encoder);

        if self.models.bind(index, models_loc, 0, encoder) {
            for (&mat, batches) in self.static_batches.iter() {
                if self.materials.loaded(mat) {
                    self.materials.bind(layout, 1, mat, encoder);
                    for (mesh, range) in batches {
                        debug_assert!(mesh_storage.contains_id(*mesh));
                        if let Some(mesh) =
                            B::unwrap_mesh(unsafe { mesh_storage.get_by_id_unchecked(*mesh) })
                        {
                            if let Err(error) = mesh.bind_and_draw(
                                0,
                                &self.vertex_format_base,
                                range.clone(),
                                encoder,
                            ) {
                                log::warn!(
                                    "Trying to draw a mesh that lacks {:?} vertex attributes. Pass {} requires attributes {:?}.",
                                    error.not_found.attributes,
                                    T::NAME,
                                    T::base_format(),
                                );
                            }
                        }
                    }
                }
            }
        }

        if let Some(pipeline_skinned) = self.pipeline_skinned.as_ref() {
            encoder.bind_graphics_pipeline(pipeline_skinned);

            if self.skinned_models.bind(index, skin_models_loc, 0, encoder) {
                self.skinning.bind(index, layout, 2, encoder);
                for (&mat, batches) in self.skinned_batches.iter() {
                    if self.materials.loaded(mat) {
                        self.materials.bind(layout, 1, mat, encoder);
                        for (mesh, range) in batches {
                            debug_assert!(mesh_storage.contains_id(*mesh));
                            if let Some(mesh) =
                                B::unwrap_mesh(unsafe { mesh_storage.get_by_id_unchecked(*mesh) })
                            {
                                if let Err(error) = mesh.bind_and_draw(
                                    0,
                                    &self.vertex_format_skinned,
                                    range.clone(),
                                    encoder,
                                ) {
                                    log::warn!(
                                        "Trying to draw a skinned mesh that lacks {:?} vertex attributes. Pass {} requires attributes {:?}.",
                                        error.not_found.attributes,
                                        T::NAME,
                                        T::skinned_format(),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn dispose(mut self: Box<Self>, factory: &mut Factory<B>, _aux: &World) {
        unsafe {
            factory
                .device()
                .destroy_graphics_pipeline(self.pipeline_basic);
            if let Some(pipeline) = self.pipeline_skinned.take() {
                factory.device().destroy_graphics_pipeline(pipeline);
            }
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
    vertex_format: &[VertexFormat],
    layouts: Vec<&B::DescriptorSetLayout>,
) -> Result<(Vec<B::GraphicsPipeline>, B::PipelineLayout), failure::Error> {
    let pipeline_layout = unsafe {
        factory
            .device()
            .create_pipeline_layout(layouts, None as Option<(_, _)>)
    }?;

    let vertex_desc = vertex_format_base
        .iter()
        .map(|f| (f.clone(), pso::VertexInputRate::Vertex))
        .chain(Some((
            VertexArgs::vertex(),
            pso::VertexInputRate::Instance(1),
        )))
        .collect::<Vec<_>>();

    let shader_vertex_basic = unsafe { T::vertex_shader().module(factory).unwrap() };
    let shader_fragment = unsafe { T::fragment_shader().module(factory).unwrap() };
    let pipe_desc = PipelineDescBuilder::new()
        .with_vertex_desc(&vertex_desc)
        .with_shaders(util::simple_shader_set(
            &shader_vertex_basic,
            Some(&shader_fragment),
        ))
        .with_layout(&pipeline_layout)
        .with_subpass(subpass)
        .with_framebuffer_size(framebuffer_width, framebuffer_height)
        .with_face_culling(pso::Face::BACK)
        .with_depth_test(pso::DepthTest {
            fun: pso::Comparison::Less,
            write: !transparent,
        })
        .with_blend_targets(vec![pso::ColorBlendDesc {
            mask: pso::ColorMask::ALL,
            blend: if transparent {
                Some(pso::BlendState::PREMULTIPLIED_ALPHA)
            } else {
                None
            },
        }]);

    let pipelines = if skinning {
        let shader_vertex_skinned = unsafe { T::vertex_skinned_shader().module(factory).unwrap() };

        let vertex_desc = vertex_format_skinned
            .iter()
            .map(|f| (f.clone(), pso::VertexInputRate::Vertex))
            .chain(Some((
                SkinnedVertexArgs::vertex(),
                pso::VertexInputRate::Instance(1),
            )))
            .collect::<Vec<_>>();

        let pipe = PipelinesBuilder::new()
            .with_pipeline(pipe_desc.clone())
            .with_child_pipeline(
                0,
                pipe_desc
                    .with_vertex_desc(&vertex_desc)
                    .with_shaders(util::simple_shader_set(
                        &shader_vertex_skinned,
                        Some(&shader_fragment),
                    )),
            )
            .build(factory, None);

        unsafe {
            factory.destroy_shader_module(shader_vertex_skinned);
        }

        pipe
    } else {
        PipelinesBuilder::new()
            .with_pipeline(pipe_desc)
            .build(factory, None)
    };

    unsafe {
        factory.destroy_shader_module(shader_vertex_basic);
        factory.destroy_shader_module(shader_fragment);
    }

    match pipelines {
        Err(e) => {
            unsafe {
                factory.device().destroy_pipeline_layout(pipeline_layout);
            }
            Err(e)
        }
        Ok(pipelines) => Ok((pipelines, pipeline_layout)),
    }
}
