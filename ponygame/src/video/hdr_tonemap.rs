use crate::video::RenderCtx;

pub struct HdrTonemapPipeline {
    pub pipeline: wgpu::RenderPipeline,
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,

    pub bind_group: wgpu::BindGroup,
}

impl HdrTonemapPipeline {
    pub const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

    fn create_texture(width: u32, height: u32, ctx: &RenderCtx) -> (wgpu::Texture, wgpu::TextureView, wgpu::BindGroup) {
        let width = width.max(1);
        let height = height.max(1);
        log::info!("create HDR texture: {}x{}", width, height);
        
        let texture = ctx.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("HDR texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::COLOR_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = ctx.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("HDR texture bind group"),
            layout: &ctx.layouts.tex_sampler,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view)
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    // Use nearest neighbor sampling for the HDR, as we're
                    // trying to sample individual pixels.
                    resource: wgpu::BindingResource::Sampler(&ctx.samplers.nearest_clamp)
                }
            ],
        });

        (texture, view, bind_group)
    }

    pub fn new(width: u32, height: u32, ctx: &RenderCtx, config: &wgpu::SurfaceConfiguration) -> Self {
        let (texture, view, bind_group) =
            Self::create_texture(width, height, ctx);

        let shader = ctx.device.create_shader_module(wgpu::include_wgsl!("hdr_tonemap.wgsl"));

        let layout = ctx.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("HdrTonemap::layout"),
            bind_group_layouts: &[
                &ctx.layouts.tex_sampler
            ],
            push_constant_ranges: &[]
        });

        // Choose whether the shader needs to do an SRGB-related remapping or not
        // based on the config format.
        
        let tonemapper = if config.format.is_srgb() {
            "tonemap_aces_to_srgb"
        }
        else {
            // Unorm is maybe a bad name here. It really means, we will perform
            // a gamma-correcting curve.
            "tonemap_aces_to_unorm"
        };

        let pipeline = ctx.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("HDR tonemap pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some(tonemapper),
                targets: &[Some(wgpu::ColorTargetState {
                    // TODO:
                    // If we re-use the same pipeline for multiple Surfaces,
                    // how do we make sure this format works correctly?
                    format: config.format,
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
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false
            },
            multiview: None,
            cache: None
        });

        HdrTonemapPipeline {
            pipeline,
            texture,
            view,
            bind_group
        }
    }

    pub fn resize(&mut self, width: u32, height: u32, ctx: &RenderCtx) {
       (self.texture, self.view, self.bind_group) = Self::create_texture(
            width, height, ctx)
    }

    pub fn render(&self, pass: &mut wgpu::RenderPass) {
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}