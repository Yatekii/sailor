use std::num::NonZeroU64;
use std::sync::{Arc, Mutex, RwLock};

use glyphon::{Cache, FontSystem, Resolution, SwashCache, TextAtlas, TextRenderer, Viewport};
use nalgebra_glm::vec2;
use osm::cache::{CacheStats, TileCache};
use osm::config::{MAX_FEATURES, MAX_TILES};
use osm::css::RulesCache;
use osm::drawing::as_byte_slice;
use osm::drawing::vertex::Vertex;
use osm::feature::collection::FeatureCollection;
use osm::interaction::collider::{Collider, VisibleTile};
use osm::math::{Coord, Camera, TileId, TileLocal};
use osm::object::Object;
use osm::platform::{FileWatcher, Watcher};
use wgpu::naga::ShaderStage;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use super::{FramePass, Layer, LayerCtx, Selection};
use crate::config::CONFIG;
use crate::drawing::helpers::load_glsl;

/// Highlight slot value meaning "nothing selected in this tile". Distinct from
/// real feature slots (small) and from the background's reserved slot (u32::MAX).
const NOT_SELECTED: u32 = u32::MAX - 1;

// One u32 highlight slot per visible tile (must match `Selected` in the shader).
const SELECTION_BUFFER_SIZE: u64 = MAX_TILES as u64 * 4;
// TODO: Should be u64::from once stabilized for const.
const UNIFORM_BUFFER_SIZE: u64 = 4 * 4 + 12 * 4 * (MAX_FEATURES as u64);

/// Records a named CPU span into the frame's span sink.
macro_rules! span {
    ($sink:expr, $name:expr, $body:expr) => {{
        let __t = web_time::Instant::now();
        let __r = $body;
        $sink.push(($name, __t.elapsed()));
        __r
    }};
}

/// The OSM vector-tile basemap, as a composable layer.
pub struct MapLayer {
    blend_pipeline: RenderPipeline,
    noblend_pipeline: RenderPipeline,
    uniform_buffer: Buffer,
    tile_transform_buffer: (Buffer, u64),
    tile_selection_buffer: Buffer,
    bind_group_layout: BindGroupLayout,
    bind_group: BindGroup,
    shader_watcher: FileWatcher,

    font_system: FontSystem,
    swash_cache: SwashCache,
    viewport: Viewport,
    atlas: TextAtlas,
    text_renderer: TextRenderer,

    /// The map owns its data: the tile cache, the shared feature/style collection
    /// (a handle the app's UI also holds), and the current visible tile set.
    tile_cache: TileCache,
    feature_collection: Arc<RwLock<FeatureCollection>>,
    visible_tiles: Vec<TileId>,
    /// Feature styles snapshot for the current frame, taken in `update`, drawn in `paint`.
    frame_features: FeatureCollection,
    visible: bool,
}

impl MapLayer {
    pub fn new(
        device: &Device,
        queue: &Queue,
        camera: &Camera,
        feature_collection: Arc<RwLock<FeatureCollection>>,
    ) -> Self {
        let shader_watcher = FileWatcher::watch(&[
            &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader,
        ]);

        let (layer_vs_module, layer_fs_module) = Self::load_shader(
            device,
            &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader,
        )
        .expect("Fatal Error. Unable to load shaders.");

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("tile vertex stage bindings"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::VERTEX,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let uniform_buffer = Self::create_uniform_buffer(device);
        let tile_transform_buffer =
            Self::create_tile_transform_buffer(device, camera, std::iter::empty());
        let tile_selection_buffer =
            Self::create_tile_selection_buffer(device, &[NOT_SELECTED; MAX_TILES]);

        let blend_pipeline = Self::create_render_pipeline(
            device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
            BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            false,
        );

        let noblend_pipeline = Self::create_render_pipeline(
            device,
            &bind_group_layout,
            &layer_vs_module,
            &layer_fs_module,
            BlendComponent::REPLACE,
            BlendComponent::REPLACE,
            true,
        );

        let bind_group = Self::create_blend_bind_group(
            device,
            &bind_group_layout,
            &uniform_buffer,
            &tile_transform_buffer,
            &tile_selection_buffer,
        );

        // Load the bundled Ruda font and use it for the default families. This
        // makes text deterministic and works on the web, which has no system fonts.
        let mut font_system = FontSystem::new_with_fonts([
            glyphon::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../../../config/Ruda-Regular.ttf").to_vec(),
            )),
            glyphon::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../../../config/Ruda-Bold.ttf").to_vec(),
            )),
        ]);
        {
            let db = font_system.db_mut();
            db.set_sans_serif_family("Ruda");
            db.set_serif_family("Ruda");
            db.set_monospace_family("Ruda");
        }
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let mut atlas = TextAtlas::new(device, queue, &cache, TextureFormat::Bgra8Unorm);
        let text_renderer = TextRenderer::new(&mut atlas, device, MultisampleState::default(), None);

        Self {
            blend_pipeline,
            noblend_pipeline,
            uniform_buffer,
            tile_transform_buffer,
            tile_selection_buffer,
            bind_group_layout,
            bind_group,
            shader_watcher,
            font_system,
            swash_cache,
            viewport,
            atlas,
            text_renderer,
            tile_cache: TileCache::new(CONFIG.general.data_root.clone()),
            feature_collection,
            visible_tiles: Vec::new(),
            frame_features: FeatureCollection::new(),
            visible: true,
        }
    }

    /// Tile cache stats for the current visible set (for the debug stats view).
    pub fn tile_stats(&self) -> CacheStats {
        self.tile_cache.get_stats(&self.visible_tiles)
    }

    /// Load the tiles covering the viewport at the given camera, dropping tiles
    /// that scrolled out. Restyles features via the app-owned stylesheet cache.
    pub fn load_visible(&mut self, camera: &Camera, css_cache: &mut RulesCache) {
        let zoom = camera.zoom;
        let tile_field = camera.get_tile_boundaries_for_zoom_level(zoom, 1);

        // Remove old bigger tiles which are not in the FOV anymore.
        let old_tile_field = camera.get_tile_boundaries_for_zoom_level(zoom - 1.0, 2);
        for tile_id in &self.visible_tiles.clone() {
            if tile_id.z == (zoom - 1.0) as u32 {
                if !old_tile_field.contains(tile_id) {
                    self.remove_visible_tile(tile_id);
                }
            } else if !tile_field.contains(tile_id) {
                self.remove_visible_tile(tile_id);
            }
        }

        self.tile_cache.finalize_loaded_tiles();
        for tile_id in tile_field.iter() {
            if !self.visible_tiles.contains(&tile_id) {
                self.tile_cache.load_tile(
                    &tile_id,
                    self.feature_collection.clone(),
                    &CONFIG.renderer.selection_tags.clone(),
                );

                if let Some(tile) = self.tile_cache.try_get_tile_mut(&tile_id) {
                    tile.load_collider();

                    self.visible_tiles.push(tile_id);

                    // Remove old bigger tile when all 4 smaller tiles are loaded.
                    let mut count = 0;
                    let num_x = (tile_id.x / 2) * 2;
                    let num_y = (tile_id.y / 2) * 2;
                    for tile_id in &[
                        TileId::new(tile_id.z, num_x, num_y),
                        TileId::new(tile_id.z, num_x + 1, num_y),
                        TileId::new(tile_id.z, num_x + 1, num_y + 1),
                        TileId::new(tile_id.z, num_x, num_y + 1),
                    ] {
                        if !tile_field.contains(tile_id) {
                            count += 1;
                            continue;
                        }
                        if self.visible_tiles.contains(tile_id) {
                            count += 1;
                        }
                    }
                    if count == 4 {
                        let tile_id = TileId::new(tile_id.z - 1, num_x / 2, num_y / 2);
                        self.remove_visible_tile(&tile_id);
                    }

                    // Remove old smaller tiles when all 4 smaller tiles are loaded.
                    for tile_id in &[
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2 + 1),
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2 + 1),
                    ] {
                        self.remove_visible_tile(tile_id);
                    }
                } else {
                    log::trace!("Could not read tile {tile_id} from cache.");
                }
            }
        }

        if let Ok(mut feature_collection) = self.feature_collection.try_write() {
            feature_collection.load_styles(zoom, css_cache);
        }
    }

    /// Load a single explicit tile (used by the `--tile` debug override).
    pub fn load_tile(&mut self, tile_id: TileId, camera: &Camera, css_cache: &mut RulesCache) {
        let zoom = camera.zoom;
        self.tile_cache.finalize_loaded_tiles();
        if !self.visible_tiles.contains(&tile_id) {
            self.tile_cache.load_tile(
                &tile_id,
                self.feature_collection.clone(),
                &CONFIG.renderer.selection_tags.clone(),
            );

            if let Some(tile) = self.tile_cache.try_get_tile_mut(&tile_id) {
                tile.load_collider();
                self.visible_tiles.push(tile_id);
            }
        }

        if let Ok(mut feature_collection) = self.feature_collection.try_write() {
            feature_collection.load_styles(zoom, css_cache);
        }
    }

    #[track_caller]
    fn remove_visible_tile(&mut self, tile_id: &TileId) {
        if let Some(index) = self.visible_tiles.iter().position(|x| x == tile_id) {
            self.visible_tiles.swap_remove(index);
        }
    }

    /// Kick off async hit-testing for the cursor, filling `hovered` with the
    /// objects under the point. The map owns the colliders; the app owns the result.
    pub fn update_hovered_objects(
        &self,
        camera: &Camera,
        point: (f32, f32),
        hovered: Arc<Mutex<Vec<Object>>>,
    ) {
        let camera = camera.clone();
        let mut visible_tiles = Vec::with_capacity(MAX_TILES);
        for tile_id in self.visible_tiles.iter() {
            let tile = self.tile_cache.try_get_tile(tile_id).unwrap();
            visible_tiles.push(VisibleTile {
                tile_id: *tile_id,
                extent: tile.extent() as f32,
                collider: tile.collider(),
                objects: tile.objects(),
            });
        }
        osm::platform::spawn(async move {
            let objects = Collider::get_hovered_objects(&visible_tiles, &camera, point);
            let mut hovered = hovered.lock().unwrap();
            *hovered = objects;
        });
    }

    fn create_render_pipeline(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        vs_module: &ShaderModule,
        fs_module: &ShaderModule,
        color_blend: BlendComponent,
        alpha_blend: BlendComponent,
        depth_write_enabled: bool,
    ) -> RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("osm layer render pipeline layout"),
            bind_group_layouts: &[Some(bind_group_layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("osm layer render pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: vs_module,
                entry_point: Some("main"),
                buffers: &[Some(VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &[
                        VertexAttribute {
                            format: VertexFormat::Sint16x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        VertexAttribute {
                            format: VertexFormat::Sint16x2,
                            offset: 4,
                            shader_location: 1,
                        },
                        VertexAttribute {
                            format: VertexFormat::Uint32,
                            offset: 8,
                            shader_location: 2,
                        },
                        VertexAttribute {
                            format: VertexFormat::Uint32,
                            offset: 12,
                            shader_location: 3,
                        },
                    ],
                })],
                compilation_options: PipelineCompilationOptions::default(),
            },
            fragment: Some(FragmentState {
                module: fs_module,
                entry_point: Some("main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Bgra8Unorm,
                    blend: Some(BlendState {
                        color: color_blend,
                        alpha: alpha_blend,
                    }),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: PipelineCompilationOptions::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32FloatStencil8,
                depth_write_enabled: Some(depth_write_enabled),
                depth_compare: Some(CompareFunction::Greater),
                stencil: StencilState {
                    front: StencilFaceState {
                        compare: CompareFunction::NotEqual,
                        fail_op: StencilOperation::Keep,
                        depth_fail_op: StencilOperation::Replace,
                        pass_op: StencilOperation::Replace,
                    },
                    back: StencilFaceState {
                        compare: CompareFunction::NotEqual,
                        fail_op: StencilOperation::Keep,
                        depth_fail_op: StencilOperation::Replace,
                        pass_op: StencilOperation::Replace,
                    },
                    read_mask: u32::MAX,
                    write_mask: u32::MAX,
                },
                bias: DepthBiasState {
                    constant: 0,
                    slope_scale: 0.0,
                    clamp: 0.0,
                },
            }),
            multisample: MultisampleState {
                count: CONFIG.renderer.msaa_samples,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
            cache: None,
        })
    }

    /// Creates a new bind group containing all the relevant uniform buffers.
    fn create_uniform_buffers(
        device: &Device,
        screen: &Camera,
        feature_collection: &FeatureCollection,
    ) -> [(Buffer, usize); 2] {
        let canvas_size_len = 4 * 4;
        let canvas_size_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("map canvas size data"),
            // screen width, screen height, selected tile, selected object
            contents: as_byte_slice(&[screen.width, screen.height, 0.0, 0.0]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_SRC,
        });

        let buffer = feature_collection.assemble_style_buffer();
        let len = buffer.len();
        let layer_data_len = len.max(1) * 12 * 4;
        let layer_data_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("feature style data"),
            contents: if len == 0 {
                &[0; 48]
            } else {
                as_byte_slice(&buffer)
            },
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_SRC,
        });

        [
            (canvas_size_buffer, canvas_size_len),
            (layer_data_buffer, layer_data_len),
        ]
    }

    fn create_uniform_buffer(device: &Device) -> Buffer {
        let data = [0; UNIFORM_BUFFER_SIZE as usize];

        device.create_buffer_init(&BufferInitDescriptor {
            label: Some("tile data"),
            contents: as_byte_slice(&data),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    }

    /// Creates a new transform buffer from the tile transforms.
    ///
    /// Ensures that the buffer has the size configured in the config, to match the size configured in the shader.
    fn create_tile_transform_buffer(
        device: &Device,
        camera: &Camera,
        visible_tiles: impl Iterator<Item = (TileId, f32)>,
    ) -> (Buffer, u64) {
        #[derive(Copy, Clone, Debug, Default)]
        #[repr(C)]
        pub struct TileData {
            pub transform: [f32; 16],
            pub extent: f32,
            _unused: f32,
            _unused2: f32,
            _unused3: f32,
        }

        const TILE_DATA_SIZE: usize = 20;
        const TILE_DATA_BUFFER_BYTE_SIZE: usize = TILE_DATA_SIZE * 4 * MAX_TILES;
        let mut data = [TileData::default(); MAX_TILES];

        for (i, (tile_id, extent)) in visible_tiles.enumerate() {
            let matrix = camera.tile_to_screen(&tile_id);
            data[i].transform.copy_from_slice(matrix.matrix().as_slice());
            data[i].extent = extent;
        }
        (
            {
                device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("tile transforms buffer"),
                    contents: as_byte_slice(data.as_slice()),
                    usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                })
            },
            TILE_DATA_BUFFER_BYTE_SIZE as u64,
        )
    }

    fn create_tile_selection_buffer(
        device: &Device,
        selected_object_ids: &[u32; MAX_TILES],
    ) -> Buffer {
        device.create_buffer_init(&BufferInitDescriptor {
            label: Some("tile selection buffer"),
            contents: as_byte_slice(selected_object_ids.as_slice()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        })
    }

    fn copy_uniform_buffers(
        encoder: &mut CommandEncoder,
        source: &[(Buffer, usize)],
        destination: &Buffer,
    ) {
        let mut total_bytes = 0;
        for (buffer, len) in source {
            encoder.copy_buffer_to_buffer(buffer, 0, destination, total_bytes, *len as u64);
            total_bytes += *len as u64;
        }
    }

    fn create_blend_bind_group(
        device: &Device,
        bind_group_layout: &BindGroupLayout,
        uniform_buffer: &Buffer,
        tile_transform_buffer: &(Buffer, u64),
        tile_selection_buffer: &Buffer,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("bind vertex stage buffers"),
            layout: bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: uniform_buffer,
                        offset: 0,
                        size: NonZeroU64::new(UNIFORM_BUFFER_SIZE),
                    }),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &tile_transform_buffer.0,
                        offset: 0,
                        size: NonZeroU64::new(tile_transform_buffer.1),
                    }),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: tile_selection_buffer,
                        offset: 0,
                        size: NonZeroU64::new(SELECTION_BUFFER_SIZE),
                    }),
                },
            ],
        })
    }

    /// Loads a shader module from a GLSL vertex and fragment shader each.
    fn load_shader(
        device: &Device,
        vertex_shader: &str,
        fragment_shader: &str,
    ) -> Result<(ShaderModule, ShaderModule), std::io::Error> {
        let vertex_shader = osm::platform::read_to_string(
            vertex_shader,
            include_str!("../../../../config/shader.vert"),
        );
        let fragment_shader = osm::platform::read_to_string(
            fragment_shader,
            include_str!("../../../../config/shader.frag"),
        );

        let vs_bytes = load_glsl(&vertex_shader, ShaderStage::Vertex);
        let vs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("VertexShader"),
            source: vs_bytes,
        });

        let fs_bytes = load_glsl(&fragment_shader, ShaderStage::Fragment);
        let fs_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("FragmentShader"),
            source: fs_bytes,
        });

        Ok((vs_module, fs_module))
    }

    /// Reloads the shader if the file watcher has detected any change to the shader files.
    fn reload_shader_if_changed(&mut self, device: &Device) {
        if !self.shader_watcher.changed() {
            return;
        }
        let Ok((vs_module, fs_module)) = Self::load_shader(
            device,
            &CONFIG.renderer.vertex_shader,
            &CONFIG.renderer.fragment_shader,
        ) else {
            return;
        };
        self.blend_pipeline = Self::create_render_pipeline(
            device,
            &self.bind_group_layout,
            &vs_module,
            &fs_module,
            BlendComponent {
                src_factor: BlendFactor::SrcAlpha,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            BlendComponent {
                src_factor: BlendFactor::One,
                dst_factor: BlendFactor::OneMinusSrcAlpha,
                operation: BlendOperation::Add,
            },
            false,
        );
        self.noblend_pipeline = Self::create_render_pipeline(
            device,
            &self.bind_group_layout,
            &vs_module,
            &fs_module,
            BlendComponent::REPLACE,
            BlendComponent::REPLACE,
            true,
        );
    }

    fn update_uniforms(
        &mut self,
        device: &Device,
        encoder: &mut CommandEncoder,
        camera: &Camera,
        selection: Option<Selection>,
        feature_collection: &FeatureCollection,
    ) {
        Self::copy_uniform_buffers(
            encoder,
            &Self::create_uniform_buffers(device, camera, feature_collection),
            &self.uniform_buffer,
        );

        let mut visible_tile_info = Vec::with_capacity(MAX_TILES);
        for tile_id in self.visible_tiles.clone() {
            let tile = self.tile_cache.get_tile(&tile_id);
            visible_tile_info.push((tile_id, tile.extent() as f32));
        }

        // One highlight slot per visible tile. Sentinel = nothing to highlight in
        // that tile. For the selected feature we translate its stable id into each
        // tile's own local slot, so a feature spanning tiles lights up in all of them.
        let mut selected_object_ids = [NOT_SELECTED; MAX_TILES];
        if let Some(selected) = selection {
            for (i, (id, _)) in visible_tile_info.iter().enumerate() {
                let slot = if *id == selected.tile_id {
                    // Same tile as the click: use the object's slot directly (also
                    // covers features without a stable id, which can't cross tiles).
                    Some(selected.feature_slot)
                } else if selected.feature_id != 0 {
                    self.tile_cache.get_tile(id).feature_slot(selected.feature_id)
                } else {
                    None
                };
                if let Some(slot) = slot {
                    selected_object_ids[i] = slot;
                }
            }
        }

        self.tile_transform_buffer =
            Self::create_tile_transform_buffer(device, camera, visible_tile_info.into_iter());

        self.tile_selection_buffer =
            Self::create_tile_selection_buffer(device, &selected_object_ids);
    }
}

impl Layer for MapLayer {
    fn name(&self) -> &str {
        "map"
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn update(&mut self, ctx: &mut LayerCtx) {
        self.reload_shader_if_changed(ctx.device);

        self.frame_features = span!(
            ctx.spans,
            "cpu.fc_clone",
            self.feature_collection.read().unwrap().clone()
        );

        span!(ctx.spans, "cpu.gpu_upload", {
            for tile_id in &self.visible_tiles {
                let tile = self.tile_cache.try_get_tile_mut(tile_id).unwrap();
                // Mesh is static after tessellation; only upload once. Selection
                // and pan/zoom go through uniforms, not the vertex buffer.
                if !tile.is_loaded_to_gpu() {
                    tile.load_to_gpu(ctx.device);
                }
                self.tile_cache.promote(tile_id);
            }
        });

        let frame_features = self.frame_features.clone();
        span!(ctx.spans, "cpu.uniforms", {
            self.update_uniforms(
                ctx.device,
                ctx.encoder,
                ctx.screen,
                ctx.selection,
                &frame_features,
            );
            self.bind_group = Self::create_blend_bind_group(
                ctx.device,
                &self.bind_group_layout,
                &self.uniform_buffer,
                &self.tile_transform_buffer,
                &self.tile_selection_buffer,
            );
        });

        span!(ctx.spans, "cpu.text_prep", {
            self.viewport.update(
                ctx.queue,
                Resolution {
                    width: ctx.resolution.0,
                    height: ctx.resolution.1,
                },
            );

            for tile_id in &self.visible_tiles {
                let tile = self.tile_cache.get_tile_mut(tile_id);
                tile.prepare_text(&mut self.font_system);
            }
            let camera = ctx.screen;
            let tile_cache = &self.tile_cache;
            let text_areas = self.visible_tiles.iter().flat_map(|tile_id| {
                let tile = tile_cache.get_tile(tile_id);
                tile.queue_text(camera)
            });

            self.text_renderer
                .prepare(
                    ctx.device,
                    ctx.queue,
                    &mut self.font_system,
                    &mut self.atlas,
                    &self.viewport,
                    text_areas,
                    &mut self.swash_cache,
                )
                .unwrap();
        });
    }

    fn paint(&self, frame: &mut FramePass) {
        span!(frame.spans, "cpu.encode_polygons", {
            let poly_ts = if frame.record_gpu {
                frame.gpu_timing.map(|g| g.writes(0, 1))
            } else {
                None
            };
            let mut render_pass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("tile polygons"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    depth_slice: None,
                    view: frame.msaa.unwrap_or(frame.view),
                    resolve_target: frame.msaa.map(|_| frame.view),
                    ops: Operations::<Color> {
                        load: LoadOp::Clear(Color::TRANSPARENT),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: frame.depth_stencil,
                    depth_ops: Some(Operations::<f32> {
                        load: LoadOp::Clear(0.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: Some(Operations::<u32> {
                        load: LoadOp::Clear(255),
                        store: StoreOp::Store,
                    }),
                }),
                timestamp_writes: poly_ts,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render_pass.set_bind_group(0, &self.bind_group, &[]);
            let corner = Coord::<TileLocal>::new(0.0, 0.0);
            let screen_dimensions = vec2(frame.screen.width, frame.screen.height) / 2.0;

            for (i, tile_id) in self.visible_tiles.iter().enumerate() {
                let matrix = frame.screen.tile_to_screen(tile_id);
                let start = matrix.apply(corner).coords() + vec2(1.0, 1.0);
                let s = vec2(
                    (start.x * screen_dimensions.x)
                        .round()
                        .max(0.0)
                        .min(screen_dimensions.x * 2.0),
                    (start.y * screen_dimensions.y)
                        .round()
                        .max(0.0)
                        .min(screen_dimensions.y * 2.0),
                );
                let matrix = frame
                    .screen
                    .tile_to_screen(&(*tile_id + TileId::new(tile_id.z, 1, 1)));
                let end = matrix.apply(corner).coords() + vec2(1.0, 1.0);
                let e = vec2(
                    (end.x * screen_dimensions.x)
                        .round()
                        .max(0.0)
                        .min(screen_dimensions.x * 2.0),
                    (end.y * screen_dimensions.y)
                        .round()
                        .max(0.0)
                        .min(screen_dimensions.y * 2.0),
                );
                let width = (e.x - s.x) as u32;
                let height = (e.y - s.y) as u32;

                if width > 0 && height > 0 {
                    render_pass.set_scissor_rect(s.x as u32, s.y as u32, width, height);
                }

                let tile = self.tile_cache.try_get_tile(tile_id).unwrap();
                let gpu_tile = tile.gpu_tile();
                tile.paint(
                    &mut render_pass,
                    &self.blend_pipeline,
                    gpu_tile,
                    &self.frame_features,
                    i as u32,
                );
            }
        });

        span!(frame.spans, "cpu.text", {
            let text_ts = if frame.record_gpu {
                frame.gpu_timing.map(|g| g.writes(2, 3))
            } else {
                None
            };
            let mut pass = frame.encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("tile text pass"),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: frame.view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: text_ts,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            self.text_renderer
                .render(&self.atlas, &self.viewport, &mut pass)
                .unwrap();
        });
    }
}
