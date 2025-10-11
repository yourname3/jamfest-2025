use std::cell::{Cell, RefCell};

use bytemuck::Zeroable;

use crate::{gc::Gp, video::{camera::Camera, hdr_tonemap::HdrTonemapPipeline, mesh_render_pipeline::MeshInstance, texture::{DepthTexture, Texture}, RenderCtx, Renderer, UniformBuffer}};

pub struct Light3D {
    pub direction: Cell<cgmath::Vector3<f32>>,
    pub color: Cell<cgmath::Vector3<f32>>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Light3DUniform {
    direction: [f32; 3],
    _pad: u32,
    color: [f32; 3],
    _pad2: u32
}

impl Light3D {
    pub fn to_uniform(&self) -> Light3DUniform {
        Light3DUniform {
            direction: self.direction.get().into(),
            _pad: 0,
            color: self.color.get().into(),
            _pad2: 0,
        }
    }
    // pub fn to_uniform(&self) -> [u8; 8] {
        

    //     let uni = AsUniform {
    //         direction: self.direction.into(),
    //         _pad: 0,
    //         color: self.color.into(),
    //         _pad2: 0,
    //     };

    //     bytemuck::cast(uni)
    // }
}

/// A renderable world. Contains some number of objects that can be rendered.
pub struct World {
    envmap: Gp<Texture>,

    pub lights: [Light3D; 1],

    meshes: RefCell<Vec<Gp<MeshInstance>>>,
}

impl World {
    pub fn new(ctx: &RenderCtx) -> Self {
        let envmap = Gp::new(Texture::dummy(ctx, Some("World::envmap (null)")));

        let lights = [Light3D {
            color: Cell::new(cgmath::vec3(0.0, 0.0, 0.0)),
            direction: Cell::new(cgmath::vec3(2.0, -10.0, -10.0)),
        }];

        Self {
            envmap,
            lights,

            meshes: RefCell::new(Vec::new()),
        }
    }

    pub fn push_mesh(&self, instance: Gp<MeshInstance>) {
        let mut meshes = self.meshes.borrow_mut();
        meshes.push(instance);
    }

    pub fn clear_meshes(&self) {
        let mut meshes = self.meshes.borrow_mut();
        meshes.clear();
    }

    pub fn set_envmap(&self, texture: &Gp<Texture>) {
        self.envmap.set(texture);
    }

    /// Computes light uniform data based on the given Camera.
    pub fn lights_to_uniform(&self, camera: &Camera) -> [Light3DUniform; 1] {
        let mut data = core::array::from_fn::<_, 1, _>(|_| Light3DUniform::zeroed());

        let view_mat = camera.get_view_matrix();

        for (idx, light) in self.lights.iter().enumerate() {
            let direction = view_mat * light.direction.get().extend(0.0);
            data[idx] = Light3DUniform {
                direction: direction.truncate().into(),
                _pad: 0,
                color: light.color.get().into(),
                _pad2: 0
            };
        }

        data
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewportUniform {
    pub view_proj_matrix: [[f32; 4]; 4],
    pub view: [[f32; 4]; 4],
    pub proj: [[f32; 4]; 4],
    pub inv_view_proj_dir: [[f32; 4]; 4],
}

impl ViewportUniform {
    pub fn identity() -> Self {
        let identity = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0], [0.0, 0.0, 0.0, 1.0]];
        ViewportUniform {
            view_proj_matrix: identity,
            view: identity,
            proj: identity,
            inv_view_proj_dir: identity
        }
    }
}

/// A Viewport is a single, actual surface to be rendered to. It contains
/// its own depth buffer, output buffer, and dimensions.
/// 
/// Each Viewport must be associated with exactly one World and Camera. The
/// World is what is being rendered to the Viewport, while the Camera determines
/// where the World is rendered.
pub struct Viewport {
    pub world: Gp<World>,
    pub camera: Gp<Camera>,

    // TODO: BindGroup is one of the main things we want to change with interior
    // mutability. It should be mostly safe to do so with cloning (?). We should
    // implement a helper type for that that is more efficient than RefCell.
    pub bind_group: RefCell<wgpu::BindGroup>,

    /// The last envrionment map we uploaded to our BindGroup. If the environment
    /// changes, we need to re-build the BindGroup.
    pub last_envmap: Gp<Texture>,

    pub viewport_buffer: UniformBuffer,
    // Right now, the lights buffer is per-viewport, as the lights should ideally
    // be in eye-space.
    pub lights_buffer: UniformBuffer,

    pub width: u32,
    pub height: u32,

    pub depth_texture: DepthTexture,
    /// For now, the Viewport itself performs the HDR pipeline. In the future,
    /// probably we will want the Viewport to simply own its own texture, and
    /// then have the tonemap pipeline stored some other way?
    pub hdr: HdrTonemapPipeline,
}

impl Viewport {
    fn build_bind_group(ctx: &RenderCtx, viewport_buffer: &UniformBuffer, lights_buffer: &UniformBuffer, envmap: &Texture) -> wgpu::BindGroup {
        ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Viewport::bind_group"),
            layout: &ctx.layouts.world,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: viewport_buffer.0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lights_buffer.0.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&envmap.view)
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&ctx.samplers.linear_clamp)
                }
            ],
        })
    }

    pub fn new(ctx: &RenderCtx, world: Gp<World>, camera: Gp<Camera>, dimensions: (u32, u32), config: &wgpu::SurfaceConfiguration) -> Self {
        let viewport_init = ViewportUniform::identity();

        let last_envmap = world.envmap.clone();
        //let lights_init = Light3DUniform::zeroed();

        let lights_init = [Light3DUniform {
            color: [8.0, 8.0, 8.0], _pad: 0,
            direction: [2.0, -10.0, -10.0], _pad2: 0,
        }];

        let viewport_buffer = ctx.create_uniform_buffer_init_from("Viewport::viewport_buffer",
            &[viewport_init]);
        let lights_buffer = ctx.create_uniform_buffer_init_from("Viewport::lights_buffer",
            &[lights_init]);

        let bind_group = Self::build_bind_group(ctx, &viewport_buffer, &lights_buffer, &last_envmap);

        let depth_texture = DepthTexture::new(ctx, dimensions);
        let hdr = HdrTonemapPipeline::new(dimensions.0, dimensions.1, ctx, config);

        Viewport {
            world,
            camera,

            bind_group: RefCell::new(bind_group),

            last_envmap,

            viewport_buffer,
            lights_buffer,

            width: dimensions.0,
            height: dimensions.1,

            depth_texture,
            hdr
        }
    }

    pub fn update(&self, ctx: &RenderCtx) {
        // The viewport must write to:
        // - The ViewportUniform
        // - The Lights array
        // - And, if we change the envmap, the envmap, although I'm not quite
        //   sure how I want to do that yet.

        let data = self.camera.to_viewport_uniform(self);
        ctx.queue.write_buffer(&self.viewport_buffer.0, 0, bytemuck::cast_slice(&[data]));

        let lights = self.world.lights_to_uniform(&self.camera);
        ctx.queue.write_buffer(&self.lights_buffer.0, 0, bytemuck::cast_slice(&lights));
    
        if !self.world.envmap.has_same_id(&self.last_envmap) {
            self.last_envmap.set(&self.world.envmap);
            let mut bind_group = self.bind_group.borrow_mut();
            *bind_group = Self::build_bind_group(ctx, &self.viewport_buffer, &self.lights_buffer, &self.last_envmap);
        }
    }

    pub fn resize(&mut self, ctx: &RenderCtx, width: u32, height: u32) {
        self.depth_texture = DepthTexture::new(ctx, (width, height));

        self.hdr.resize(width, height, ctx);

        self.width  = width;
        self.height = height;
    }

    // TODO: Probably the Viewport itself should contain either a Surface
    // or a TextureView, or maybe an option of either, depending on its usage.

    pub fn render(&self, renderer: &Renderer, encoder: &mut wgpu::CommandEncoder, output_view: &wgpu::TextureView) {
        {
            let mut world_render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world_render_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr.view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // For the viewport, we must set the bind group 0 to our own bind group.
            self.update(&renderer.ctx);
            let group = self.bind_group.borrow();
            world_render_pass.set_bind_group(0, Some(&*group), &[]);
            
            renderer.sky.render(&mut world_render_pass);

           // renderer.mesh_renderer.bind(&mut world_render_pass);
            for mesh in self.world.meshes.borrow().iter() {
                let shader = &mesh.material.shader;
                // TODO: Reduce number of calls to bind(), either by sorting, or
                // maybe by an extra check for which shader is already bound?
                shader.bind(&mut world_render_pass);
                shader.render(&renderer.ctx, &mut world_render_pass, &mesh);
            }
            //renderer.mesh_renderer.render(&mut world_render_pass, &renderer.ctx.queue);
        }

        {
             let mut hdr_tonemap_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hdr_tonemap_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None
            });

            self.hdr.render(&mut hdr_tonemap_pass);
        }
    }
}