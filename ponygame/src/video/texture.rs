use std::ops::Deref;

use image::{metadata::Cicp, EncodableLayout, GenericImage, GenericImageView, ImageBuffer, Rgba};
use wgpu::Extent3d;

use crate::{error::PonyResult, video::RenderCtx};

pub struct DepthTexture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl DepthTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(ctx: &RenderCtx, dimensions: (u32, u32)) -> Self {
        let size = wgpu::Extent3d {
            width: dimensions.0.max(1),
            height: dimensions.1.max(1),
            depth_or_array_layers: 1
        };

        let desc = wgpu::TextureDescriptor {
            label: Some("DepthTexture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[]
        };

        let texture = ctx.device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self { texture, view }
    }
}

pub struct Texture {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
}

impl Texture {
    pub fn from_bytes_rgba8srgb(
        ctx: &RenderCtx,
        bytes: &[u8],
        label: Option<&str>,
        generate_mipmaps: bool,
    ) -> PonyResult<Self> {
        let img = image::load_from_memory(bytes)?;
        Ok(Self::from_image_rgba8srgb(ctx, &img, label, generate_mipmaps))
    }

    pub fn from_bytes_rgba16unorm(
        ctx: &RenderCtx,
        bytes: &[u8],
        label: Option<&str>,
        generate_mipmaps: bool,
    ) -> PonyResult<Self> {
        let img = image::load_from_memory(bytes)?;
        Ok(Self::from_image_rgba16unorm(ctx, &img, label, generate_mipmaps))
    }

    pub fn dummy(
        ctx: &RenderCtx,
        label: Option<&str>,
    ) -> Self {
        let mut rgb = image::RgbaImage::new(1, 1);
        *rgb.get_pixel_mut(0, 0) = image::Rgba::<u8>([255, 255, 255, 255]);
        let dynamic = image::DynamicImage::ImageRgba8(rgb);

        Self::from_image_rgba8srgb(ctx, &dynamic, label, false)
    }

    pub fn from_image_rgba16unorm(
        ctx: &RenderCtx,
        image: &image::DynamicImage,
        label: Option<&str>,
        generate_mipmaps: bool,
    ) -> Self {
        // On WASM32, fall back to rgba8 unorm for now...
        if cfg!(target_arch = "wasm32") {
            // let mut hack = image.to_rgba16();
            // let mut hack2 = ImageBuffer::<Rgba<_>, Vec<u8>>::new(hack.width(), hack.height());
            // for (in_p, out_p) in std::iter::zip(hack.pixels(), hack2.pixels_mut()) {
            //     for i in 0..4 {
            //         // WEird jank? Colors are still wrong...
            //         let f = in_p.0[i] as f32 / 65535.0;
            //         //let f = f32::sqrt(f);
            //         let f = f32::powf(f, 1.0 / 2.2);
            //         out_p.0[i] = (f * 255.0) as u8;
            //     }
            // }

            return Self::from_image_generic::<ImageBuffer<Rgba<u8>, Vec<u8>>>(
                ctx,
                &image.to_rgba8(),
                wgpu::TextureFormat::Rgba8Unorm,
                4,
                label,
                generate_mipmaps
            );
        }

        Self::from_image_generic::<ImageBuffer<Rgba<u16>, Vec<u16>>>(
            ctx,
            &image.to_rgba16(),
            wgpu::TextureFormat::Rgba16Unorm,
            8,
            label,
            generate_mipmaps
        )
    }

    pub fn from_image_rgba8srgb(
        ctx: &RenderCtx,
        image: &image::DynamicImage,
        label: Option<&str>,
        generate_mipmaps: bool,
    ) -> Self {
        Self::from_image_generic::<ImageBuffer<Rgba<u8>, Vec<u8>>>(
            ctx,
            &image.to_rgba8(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
            4,
            label,
            generate_mipmaps
        )
    }

    fn from_image_generic<Data>(
        ctx: &RenderCtx,
        data: &Data,
        // Must match the Data type.
        format: wgpu::TextureFormat,
        // Must match the Data type.
        bytes_per_pixel: u32,
        label: Option<&str>,
        generate_mipmaps: bool,
    ) -> Self
    where
        Data: std::convert::From<image::DynamicImage> + Deref<Target: EncodableLayout> + GenericImageView,
        <Data as GenericImageView>::Pixel: 'static,
        ImageBuffer<<Data as GenericImageView>::Pixel, Vec<<<Data as GenericImageView>::Pixel as image::Pixel>::Subpixel>>: Deref<Target: EncodableLayout>,
    {
        let dimensions = data.dimensions();

        log::info!("load texture '{:?}': {}x{}, {:?}", label, dimensions.0, dimensions.1, format);

        let mut extra_levels = vec![];

        if generate_mipmaps {
            let mut cur_dimen = dimensions;
            cur_dimen.0 /= 2;
            cur_dimen.1 /= 2;
            while cur_dimen.0 >= 1 && cur_dimen.1 >= 1 {
                let mip = image::imageops::resize(data,
                    cur_dimen.0, cur_dimen.1,
                    image::imageops::FilterType::CatmullRom);

                extra_levels.push(mip);

                cur_dimen.0 /= 2;
                cur_dimen.1 /= 2;
            }
        }

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = ctx.device.create_texture(
            &wgpu::TextureDescriptor {
                label,
                size,
                mip_level_count: (1 + extra_levels.len()) as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            }
        );

        ctx.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                aspect: wgpu::TextureAspect::All,
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
            },
            data.as_bytes(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_pixel * dimensions.0),
                rows_per_image: Some(dimensions.1)
            },
            size,
        );

        for (idx, level) in extra_levels.iter().enumerate() {
            let dimensions = level.dimensions();
            let size = Extent3d {
                width: dimensions.0,
                height: dimensions.1,
                depth_or_array_layers: 1,
            };
            log::info!("write dimensions: {:?} to mip level: {}", dimensions, (idx + 1));
            ctx.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    aspect: wgpu::TextureAspect::All,
                    texture: &texture,
                    mip_level: (idx + 1) as u32,
                    origin: wgpu::Origin3d::ZERO,
                },
                level.as_bytes(),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_pixel * dimensions.0),
                    rows_per_image: Some(dimensions.1)
                },
                size,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Texture {
            texture,
            view,
        }
    }
}

pub struct Sampler {
    sampler: wgpu::Sampler
}

impl Sampler {
    pub fn new(ctx: &RenderCtx) -> Self {
        let sampler = ctx.device.create_sampler(
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

        Sampler { sampler }
    }
}