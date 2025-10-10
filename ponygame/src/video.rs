pub mod texture;
pub mod mesh_render_pipeline;
pub mod sky_pipeline;
pub mod asset_import;
pub mod hdr_tonemap;
pub mod camera;
pub mod world;

use std::{cell::{Cell, RefCell}, collections::HashMap, rc::Rc};
use cgmath::vec3;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};

use wgpu::util::DeviceExt;
use winit::{dpi::{PhysicalSize, Size}, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoopProxy}, window::{WindowAttributes, WindowId}};

use crate::{gc::{Gp, GpMaybe}, video::{camera::Camera, hdr_tonemap::HdrTonemapPipeline, mesh_render_pipeline::MeshRenderPipeline, sky_pipeline::SkyPipeline, texture::{DepthTexture, Texture}, world::{Viewport, World}}, PonyGame, PonyGameAppEvent};

// Bundles together all the global state that a given part of the renderer might
// need, i.e. the device, queue, etc.
pub struct RenderCtx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    
    pub instance: wgpu::Instance,
    pub adapter: wgpu::Adapter,

    pub layouts: Layouts,
    
    pub samplers: Samplers,
}

pub struct UniformBuffer(pub wgpu::Buffer);
pub struct VertexBuffer(pub wgpu::Buffer);
pub struct IndexBuffer(pub wgpu::Buffer);

impl RenderCtx {
    pub async fn new(initial_window: &winit::window::Window) -> (Self, wgpu::Surface<'static>) {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            // On WASM, we want to target both WebGPU and WebGL2, whatever is available.
            //
            // TODO: We aren't exactly respecting the WebGL limits, which can
            // be lower than the WebGPU limits.
            backends: if cfg!(target_arch = "wasm32") {
                wgpu::Backends::all()
            }
            else {
                wgpu::Backends::PRIMARY
            },
            ..Default::default()
        });

        let surface = unsafe {
            let raw_display_handle = initial_window.display_handle().unwrap().as_raw();
            let raw_window_handle = initial_window.window_handle().unwrap().as_raw();
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle, raw_window_handle })
                .unwrap()
        };

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptionsBase {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface)
            })
            .await
            .unwrap();

        let info = adapter.get_info();
        log::info!("Using graphics adapter: {} - {}\n{}\n{}\nDevice Type: {:?}\nBackend: {}", info.vendor, info.name,
            info.driver, info.driver_info, info.device_type, info.backend);

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: None,
                required_features: if cfg!(target_arch = "wasm32") {
                    wgpu::Features::empty()
                }
                else {
                    wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
                },
                    
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                }
                else {
                    wgpu::Limits::default()
                },
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
            })
            .await
            .unwrap();

        let layouts = Layouts::new(&device);
        let samplers = Samplers::new(&device);

        let ctx = Self {
            device,
            queue,

            instance,
            adapter,

            layouts,
            samplers,
        };

        (ctx, surface)
    }

    pub fn create_uniform_buffer_init(&self, label: &str, data: &[u8]) -> UniformBuffer {
        UniformBuffer(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: data,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        }))
    }

    pub fn create_uniform_buffer_init_from<T: bytemuck::NoUninit>(&self, label: &str, data: &[T]) -> UniformBuffer {
        self.create_uniform_buffer_init(label, bytemuck::cast_slice(data))
    }

    pub fn create_uniform_buffer_init_zero<T: bytemuck::NoUninit + bytemuck::Zeroable>(&self, label: &str) -> UniformBuffer {
        self.create_uniform_buffer_init_from(label, &[T::zeroed()])
    }

    pub fn create_vertex_buffer_init(&self, label: &str, data: &[u8]) -> VertexBuffer {
        VertexBuffer(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: data,
            usage: wgpu::BufferUsages::VERTEX,
        }))
    }

    pub fn create_vertex_buffer_init_from<T: bytemuck::NoUninit>(&self, label: &str, data: &[T]) -> VertexBuffer {
        self.create_vertex_buffer_init(label, bytemuck::cast_slice(data))
    }

    pub fn create_index_buffer_init(&self, label: &str, data: &[u8]) -> IndexBuffer {
        IndexBuffer(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: data,
            usage: wgpu::BufferUsages::INDEX,
        }))
    }

    pub fn create_index_buffer_init_from_u16(&self, label: &str, data: &[u16]) -> IndexBuffer {
        self.create_index_buffer_init(label, bytemuck::cast_slice(data))
    }

    pub fn create_index_buffer_init_from_u32(&self, label: &str, data: &[u32]) -> IndexBuffer {
        self.create_index_buffer_init(label, bytemuck::cast_slice(data))
    }
}

pub struct Renderer {
    pub ctx: RenderCtx,

    pub mesh_renderer: MeshRenderPipeline,
    pub sky: SkyPipeline,
}

struct PerWindowRenderer {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    is_surface_configured: bool,

    viewport: Viewport,
}

struct Samplers {
    linear_clamp: wgpu::Sampler,
    nearest_clamp: wgpu::Sampler,

    // The sampler used for sampling depth textures (specifically our DepthTexture).
    depth_texture_sampler: wgpu::Sampler,
}

impl Samplers {
    pub fn new(device: &wgpu::Device) -> Self {
        let linear_clamp = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }
        );

        // This sampler comes from Learn WGPU.
        // The compare function definitely seems relevant. I'm not sure exactly
        // what the Lod parameters do.
        //
        // Note: This sampler is actually not used anywhere. *shrug*
        let depth_texture_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,

            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,

            mipmap_filter: wgpu::FilterMode::Nearest,

            compare: Some(wgpu::CompareFunction::LessEqual),

            lod_min_clamp: 0.0,
            // ?
            lod_max_clamp: 100.0,

            ..Default::default()
        });

        let nearest_clamp = device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            }
        );

        Self {
            linear_clamp,
            nearest_clamp,
            depth_texture_sampler,
        }
    }
}


// What we'll eventually want to do is have a set of common bind group layouts
// for some number of texture slots, so that textures can easily be re-used
// with the same BindGroup (i.e. one BindGroup per texture per slot).
struct Layouts {
    // TODO

    // Do we actually want to necessarily bind the texture and the sampler together?
    // It might be better to bind them separately so they can be swapped out separately
    tex_sampler: wgpu::BindGroupLayout,

    single_uniform: wgpu::BindGroupLayout,

    pbr_material: wgpu::BindGroupLayout,

    world: wgpu::BindGroupLayout,

    mesh_3d: wgpu::BindGroupLayout,

    /// Layout for a pipeline that uses the World bind group and the PBR bind
    /// group.
    pipeline_world_pbr: wgpu::PipelineLayout,
    /// Layout for a pipeline that uses the World bind group.
    pipeline_world: wgpu::PipelineLayout,
}

pub struct PBRMaterial {
    pub albedo: cgmath::Vector3<f32>,
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,

    //albedo_texture: TextureHandle,
    //metallic_roughness_texture: TextureHandle,

    // TODO: Make these private -- if we change them, we need to re-create the
    // bind group.
    pub albedo_texture: Texture,
    pub metallic_roughness_texture: Texture,

    pub albedo_decal_texture: Texture,
    pub metallic_roughness_decal_texture: Texture,

    pub cached_bind_group: GpMaybe<wgpu::BindGroup>,
}

impl PBRMaterial {
    pub fn default(ctx: &RenderCtx) -> Self {
        PBRMaterial {
            albedo: vec3(1.0, 1.0, 1.0),
            metallic: 0.0,
            roughness: 1.0,
            reflectance: 0.5,
            albedo_texture: Texture::dummy(ctx, Some("Texture::dummy::albedo")),
            metallic_roughness_texture: Texture::dummy(ctx, Some("Texture::dummy::metallic_roughness")),

            // Default decal is totally transparent
            albedo_decal_texture: Texture::dummy_transparent(ctx, Some("Texture::dummy::albedo")),
            metallic_roughness_decal_texture: Texture::dummy(ctx, Some("Texture::dummy::metallic_roughness")),
            cached_bind_group: GpMaybe::none(),
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PBRUniform {
    pub albedo: [f32; 3],
    pub metallic: f32,
    pub roughness: f32,
    pub reflectance: f32,
    pub _pad: u32,
    pub _pad2: u32,
}

impl PerWindowRenderer {
    pub fn new_from_surface_and_ctx(
        window: &winit::window::Window,
        surface: wgpu::Surface<'static>,
        ctx: &RenderCtx
    ) -> Self {
        // TODO: DPI stuff...
        let PhysicalSize { width: win_width, height: win_height } = window.inner_size();

        let surface_caps = surface.get_capabilities(&ctx.adapter);

        let surface_format = surface_caps.formats.iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: win_width,
            height: win_height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let world = World::new(ctx);
        let camera = Camera::demo();

        let viewport = Viewport::new(ctx, Gp::new(world), Gp::new(camera),
        (win_width, win_height), &config);

        Self {
            surface,
            config,
            is_surface_configured: false,

            viewport
        }
    }

    pub fn new(window: &winit::window::Window, ctx: &RenderCtx) -> Self {
         let surface = unsafe {
            let raw_display_handle = window.display_handle().unwrap().as_raw();
            let raw_window_handle = window.window_handle().unwrap().as_raw();
            ctx.instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::RawHandle { raw_display_handle, raw_window_handle })
                .unwrap()
        };

        Self::new_from_surface_and_ctx(window, surface, ctx)
    }

    fn resize(&mut self, renderer: &Renderer, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&renderer.ctx.device, &self.config);
            self.is_surface_configured = true;

            // Recreate depth texture when window is resized
            self.viewport.resize(&renderer.ctx, width, height);
        }
    }

    fn render(&self, renderer: &Renderer, window: &winit::window::Window) -> Result<(), wgpu::SurfaceError> {
        if !self.is_surface_configured { return Ok(()) }

        let output = self.surface.get_current_texture()?;

        let output_view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        log::trace!("render to output_view: {}x{}", output_view.texture().width(), output_view.texture().height());

        let mut encoder = renderer.ctx.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder")
        });

        self.viewport.render(renderer, &mut encoder, &output_view);

        renderer.ctx.queue.submit(std::iter::once(encoder.finish()));

        window.pre_present_notify();

        output.present();

        Ok(())
    }
}


impl PBRMaterial {
    pub fn to_uniform(&self) -> PBRUniform {
        PBRUniform {
            albedo: self.albedo.into(),
            _pad: 0,
            metallic: self.metallic,
            roughness: self.roughness,
            reflectance: self.reflectance,
            _pad2: 0,
        }
    }

    pub fn get_bind_group(&self, ctx: &RenderCtx) -> Gp<wgpu::BindGroup> {
        match self.cached_bind_group.get() {
            Some(cached) => cached,
            None => {
                let uniform = self.to_uniform();
                let as_buffer = ctx.create_uniform_buffer_init_from("PBR Uniform", &[uniform]);
                
                let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("PBR bind group"),
                    layout: &ctx.layouts.pbr_material,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: as_buffer.0.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&self.albedo_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 2,
                            resource: wgpu::BindingResource::TextureView(&self.metallic_roughness_texture.view)
                        },
                        wgpu::BindGroupEntry {
                            binding: 3,
                            resource: wgpu::BindingResource::TextureView(&self.albedo_decal_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 4,
                            resource: wgpu::BindingResource::TextureView(&self.metallic_roughness_decal_texture.view),
                        },
                        wgpu::BindGroupEntry {
                            // TODO: Let use choose sampler?
                            binding: 5,
                            resource: wgpu::BindingResource::Sampler(&ctx.samplers.linear_clamp)
                        }
                    ],
                });

                let in_gc = Gp::new(bind_group);
                self.cached_bind_group.set(Some(&in_gc));

                in_gc
            },
        }
    }
}




impl Layouts {
    fn new(device: &wgpu::Device) -> Self {
        fn simple_uniform(binding: u32) -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None
                },
                count: None,
            }
        }

        fn simple_texture(binding: u32) -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    multisampled: false,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    sample_type: wgpu::TextureSampleType::Float { filterable: true }
                },
                count: None,
            }
        }

        fn simple_sampler(binding: u32) -> wgpu::BindGroupLayoutEntry {
            wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            }
        }

        let world = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Layouts::world"),
            entries: &[
                simple_uniform(0),
                simple_uniform(1),
                simple_texture(2),
                simple_sampler(3),
            ]
        });

        let tex_sampler = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                simple_texture(0),
                simple_sampler(1)
            ],
            label: Some("Layouts::tex_sampler")
        });

        let single_uniform = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Layouts::single_uniform"),
            entries: &[
                simple_uniform(0)
            ],
        });

        let pbr_material = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Layouts::pbr_material"),
            entries: &[
                // PBR
                simple_uniform(0),
                // Albedo Texture
                simple_texture(1),
                // Metallic-roughness texture
                simple_texture(2),
                // Albedo decal
                simple_texture(3),
                // Metallic-roughness decal
                simple_texture(4),
                simple_sampler(5),
            ]
        });

        let pipeline_world_pbr = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Layouts::pipeline_world_pbr"),
            bind_group_layouts: &[
                &world,
                &pbr_material
            ],
            push_constant_ranges: &[]
        });

        let pipeline_world = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Layouts::pipeline_world_pbr"),
            bind_group_layouts: &[
                &world,
            ],
            push_constant_ranges: &[]
        });

        let mesh_3d = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Layouts::mesh_3d"),
            entries: &[
                // Transform
                simple_uniform(0)
            ],
        });

        Self {
            tex_sampler,
            single_uniform,
            pbr_material,
            world,
            mesh_3d,

            pipeline_world_pbr,
            pipeline_world,
        }
    }
}



impl Renderer {
    async fn new(initial_window: &winit::window::Window) -> (Renderer, PerWindowRenderer) {
        let (ctx, surface) = RenderCtx::new(initial_window).await;

        let initial_per_window = PerWindowRenderer::new_from_surface_and_ctx(initial_window,
            surface, &ctx);

        // For our pipelines, we will use the config from the initial_per_window.
        //
        // It should be the case that this pipeline is compatible with other
        // windows (?)
        let mesh_renderer = MeshRenderPipeline::new(&ctx);
        let sky = SkyPipeline::new(&ctx);

        let renderer = Renderer {
            ctx,

            mesh_renderer,
            sky,
        };

        (renderer, initial_per_window)
    }
}

// For now, this is private, because we can't really let stuff look at it.
// Could wrap it as a private member of a public struct.
struct Window {
    sdl: winit::window::Window,
    renderer: PerWindowRenderer,
}

pub struct Video {
    // Remains as None until we create it, the first time we create a window.
    pub renderer: Renderer,

    // TODO: GC integration..?
    id_map: HashMap<WindowId, Window>,
}

impl Video {
    fn finish_initializing(
        renderer: Renderer,
        mut id_map: HashMap<WindowId, Window>,
        mut per: PerWindowRenderer,
        underlying_window: winit::window::Window,
        proxy: EventLoopProxy<PonyGameAppEvent>,
    ) {
        per.resize(&renderer, 800, 600);

        let world = per.viewport.world.clone();
        let camera = per.viewport.camera.clone();

        let id = underlying_window.id();
        let window = Window {
            sdl: underlying_window,
            renderer: per
        };
        id_map.insert(id, window);

        let video = Video {
            id_map,
            renderer,
        };

        let engine = PonyGame {
            video,
            audio: crate::audio::Audio::initial(),
            main_world: world,
            main_camera: camera,

            accumulator: web_time::Duration::from_micros(0),
            last_tick: web_time::Instant::now(),
        };

        assert!(proxy.send_event(PonyGameAppEvent::Initialize(engine))
            .is_ok())
    }

    #[inline(always)]
    pub fn wasm_remove_loading_screen() {
        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            use wasm_bindgen::UnwrapThrowExt;
            
            const LOADING_ID: &str = "loading-tmp";

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let loader = document.get_element_by_id(LOADING_ID).unwrap_throw();
            loader.remove();
        }
    }

    pub fn new(
        event_loop: &ActiveEventLoop,
        proxy: EventLoopProxy<PonyGameAppEvent>,
        // TODO: Bundle properties together better?
        game_title: &str,
    ) {
        let mut attributes = winit::window::Window::default_attributes();
        attributes.title = game_title.to_string();
        attributes.inner_size = Some(PhysicalSize::new(800, 600).into());
        attributes.resizable = true;

        #[cfg(target_arch = "wasm32")]
        {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            use wasm_bindgen::UnwrapThrowExt;
            
            const CANVAS_ID: &str = "canvas";

            // Don't set an initial size here.
            attributes.inner_size = None;

            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let canvas = document.get_element_by_id(CANVAS_ID).unwrap_throw();
            let html_canvas_element = canvas.unchecked_into();
            attributes = attributes.with_canvas(Some(html_canvas_element));
        }

        let window = event_loop.create_window(attributes)
        .expect("Initial window");
    
        let mut id_map = HashMap::new();
        
        // On wasm, we cannot use any executor except for the browser. One
        // particularly notable fact: pollster will use e.g. a condvar which
        // will panic.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (renderer, mut per) = pollster::block_on(Renderer::new(&window));
            Self::finish_initializing(renderer, id_map, per, window, proxy);
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                let (renderer, mut per) = Renderer::new(&window).await;
                Self::finish_initializing(renderer, id_map, per, window, proxy);
            });
        }
    }

    pub fn update_all_window_sizes(&mut self) {
        for window in self.id_map.values_mut() {
            let phys = window.sdl.inner_size();
            window.renderer.resize(&self.renderer, phys.width, phys.height);
            window.sdl.request_redraw();
        }
    }

    /// For now, returns whether we should now close the application.
    pub fn handle_win_event(&mut self, window_id: WindowId, win_event: WindowEvent) -> bool {
        let Some(window) = self.id_map.get_mut(&window_id) else { return false; };

        match win_event {
            WindowEvent::RedrawRequested => {
                //  TODO: How do we handle this per-window?
                self.render();

                self.id_map.get(&window_id).unwrap().sdl.request_redraw();
            }
            WindowEvent::Resized(phys) => {
                window.renderer.resize(&self.renderer, phys.width, phys.height);
                window.sdl.request_redraw();
            },
            WindowEvent::CloseRequested => {
                // This removes the inner 'sdl' object from existing, which results
                // in a DestroyWindow operation.
                self.id_map.remove(&window_id);
                return self.id_map.is_empty();
            }
            _ => {}
        }

        return false;
    }

    pub fn render(&mut self) {
        for window in self.id_map.values_mut() {
            if let Err(err) = window.renderer.render(&self.renderer, &window.sdl) {
                log::warn!("renderer: error: {}", err);
            }
        }
    }
}