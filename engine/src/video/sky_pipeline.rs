
use crate::video::{hdr_tonemap::HdrTonemapPipeline, RenderCtx};

pub struct SkyPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl SkyPipeline {
    pub fn new(ctx: &RenderCtx) -> Self {
        let shader = ctx.device.create_shader_module(wgpu::include_wgsl!("sky.wgsl"));

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SkyPipeline::pipeline"),
            layout: Some(&ctx.layouts.pipeline_world),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
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
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                // TODO: Is it more efficient to cull None?
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },

            // We could use a depth stencil and then draw the sky after everything.
            // But that won't work with transparent stuff...

            // Apparently we still have to have a stencil attachment, though,
            // due to the render pass using one. Although I guess we could do
            // a separate render pass.

            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::video::texture::DepthTexture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
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

        SkyPipeline {
            pipeline,
        }
    }

    /// ASSUMPTION: The world bind group is bound to bind group 0.
    pub fn render(&self, pass: &mut wgpu::RenderPass) {
        // Draw a full-screen triangle.
        pass.set_pipeline(&self.pipeline);
        pass.draw(0..3, 0..1)
    }
}