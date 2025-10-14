
use std::cell::Cell;

use cgmath::{vec4, Vector4};

use crate::{gc::Gp, video::{asset_import::MeshData, hdr_tonemap::HdrTonemapPipeline, texture::{self}, IndexBuffer, PBRMaterial, RenderCtx, UniformBuffer, VertexBuffer}};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal  : [f32; 3],
    pub uv      : [f32; 2],
    pub uv2     : [f32; 2],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBS: &[wgpu::VertexAttribute] = &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x2,
            3 => Float32x2,
         ];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBS
        }
    }
}

pub struct Mesh {
    vertex_buffer: VertexBuffer,
    index_buffer: IndexBuffer,

    vertex_count: u32,
    index_count: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MeshInstanceUniform {
    transform: [[f32; 4]; 4],
    modulate: [f32; 4],
}

pub struct MeshInstance {
    mesh: Gp<Mesh>,
    pub material: Gp<PBRMaterial>,

    pub modulate: Cell<cgmath::Vector4<f32>>,

    pub transform: Cell<cgmath::Matrix4<f32>>,

    /// Uniform buffer for our instance.
    uniform_buffer: UniformBuffer,

    /// Bind group for the instance. Contains per-instance uniform (and texture)
    /// data.
    instance_bind_group: wgpu::BindGroup,
}

impl MeshInstance {
    pub fn new(ctx: &RenderCtx, mesh: Gp<Mesh>, material: Gp<PBRMaterial>, transform: cgmath::Matrix4<f32>) -> Self {
        Self::new_modulate(ctx, mesh, material, transform, vec4(1.0, 1.0, 1.0, 1.0))
    }

    pub fn new_modulate(ctx: &RenderCtx, mesh: Gp<Mesh>, material: Gp<PBRMaterial>, transform: cgmath::Matrix4<f32>, modulate: Vector4<f32>) -> Self {
        let uniform_buffer = ctx.create_uniform_buffer_init_zero::<MeshInstanceUniform>("MeshInstance::uniform_buffer");
        
        let instance_bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("MeshInstance::instance_bind_group"),
            layout: &ctx.layouts.mesh_3d,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.0.as_entire_binding()
                }
            ],
        });

        let instance = Self {
            mesh,
            material,

            modulate: Cell::new(modulate),

            transform: Cell::new(transform),

            uniform_buffer,
            instance_bind_group,
        };

        instance.update(ctx);
        instance
    }

    pub fn update(&self, ctx: &RenderCtx) {
        let uniform = MeshInstanceUniform {
            transform: self.transform.get().into(),
            modulate: self.modulate.get().into(),
        };
        ctx.queue.write_buffer(&self.uniform_buffer.0, 0, bytemuck::cast_slice(&[uniform]));
    }
}

pub struct PBRShader {
    pipeline: wgpu::RenderPipeline,   
}

impl Mesh {
    pub fn new(ctx: &RenderCtx, data: &MeshData) -> Self {
        let vertex_buffer = ctx.create_vertex_buffer_init_from("Mesh::vertex_buffer", &data.vertex_data);
        let index_buffer = ctx.create_index_buffer_init_from_u32("Mesh::index_buffer", &data.index_data);

        Mesh {
            vertex_buffer,
            index_buffer,

            vertex_count: data.vertex_data.len() as u32,
            index_count: data.index_data.len() as u32,
        }
    }
}

impl PBRShader {
    pub fn new(ctx: &RenderCtx, label: &str, pbr_fn: &str) -> Self {
        let mut whole_shader = include_str!("mesh.wgsl").to_string();
        whole_shader.push_str(pbr_fn);

        let shader = ctx.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(whole_shader.into()),
        });

        let layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("PBRShader::layout"),
            bind_group_layouts: &[
                &ctx.layouts.world,
                &ctx.layouts.pbr_material,
                &ctx.layouts.mesh_3d,
            ],
            push_constant_ranges: &[],
        });

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("PBRShader::pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[
                    Vertex::desc(),
                ],
                compilation_options: wgpu::PipelineCompilationOptions::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("pbr_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    // TODO:
                    // If we re-use the same pipeline for multiple Surfaces,
                    // how do we make sure this format works correctly?
                    format: HdrTonemapPipeline::COLOR_FORMAT,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::all()
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::DepthTexture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false
            },
            multiview: None,
            cache: None
        });

        PBRShader {
            pipeline,
        }
    }

    pub fn bind(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.pipeline);
    }

    pub fn render(&self, ctx: &RenderCtx, pass: &mut wgpu::RenderPass, mesh: &MeshInstance) {
        pass.set_vertex_buffer(0, mesh.mesh.vertex_buffer.0.slice(..));
        // No instance buffer for now.

        pass.set_index_buffer(mesh.mesh.index_buffer.0.slice(..), wgpu::IndexFormat::Uint32);
    
        // TODO: Sort by material, so we only have to set this bind group once.
        pass.set_bind_group(1, Some(mesh.material.get_bind_group(ctx).as_ref()), &[]);

        pass.set_bind_group(2, Some(&mesh.instance_bind_group), &[]);

        pass.draw_indexed(0..mesh.mesh.index_count, 0, 0..1);
    }
}