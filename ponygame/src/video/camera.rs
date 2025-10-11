use std::cell::Cell;

use cgmath::SquareMatrix;

use crate::video::{texture::Texture, world::{Viewport, ViewportUniform}, RenderCtx, UniformBuffer};

pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

#[derive(Clone, Copy)]
pub enum CameraProjection {
    Perspective {
        fovy: f32,
        znear: f32,
        zfar: f32,
    },
    Orthographic {
        zoom: f32,
    },
}

pub struct Camera {
    pub position: Cell<cgmath::Point3<f32>>,
    pub target: Cell<cgmath::Point3<f32>>,
    pub up: cgmath::Vector3<f32>,

    pub projection: Cell<CameraProjection>,
}

impl Camera {
    pub fn demo() -> Self {
        Camera {
            position: Cell::new((0.0, 0.0, 5.0).into()),
            target: Cell::new((0.0, 0.0, 0.0).into()),
            up: (0.0, 1.0, 0.0).into(),
            projection: Cell::new(CameraProjection::Perspective {
                fovy: 45.0,
                znear: 0.1,
                zfar: 100.0,
            }),
        }
    }

    // TODO: We really should cache this...
    pub fn get_view_matrix(&self) -> cgmath::Matrix4<f32> {
        cgmath::Matrix4::look_at_rh(self.position.get(), self.target.get(), self.up)
    }

    pub fn get_projection_matrix(&self, viewport: &Viewport) -> cgmath::Matrix4<f32> {
        let aspect: f32 = viewport.width as f32 / viewport.height as f32;

        match self.projection.get() {
            CameraProjection::Perspective { fovy, znear, zfar } => {
                cgmath::perspective(cgmath::Deg(fovy),
                    aspect, znear, zfar)
            },
            CameraProjection::Orthographic { zoom } => {
                // So that we don't depend on the screen size, base our projection
                // matrix primarily on the aspect ratio.
                let h = 1.0 * zoom * 0.5;
                let w = aspect * h;

                // TODO: Do we want zfar to be part of the projection?
                cgmath::ortho(-w, w, -h, h, 
                    0.0, 1000.0)
            },
        }
    }

    pub fn to_viewport_uniform(&self, viewport: &Viewport) -> ViewportUniform {
        // TODO: Cache view & projection matrices independently (and also do
        // one projection-matrix per viewport)
        let view = cgmath::Matrix4::look_at_rh(self.position.get(), self.target.get(), self.up);

        let proj = self.get_projection_matrix(viewport);        

        let vp = OPENGL_TO_WGPU_MATRIX * proj * view;

        let mut help = vp;
        help.w.x = 0.0;
        help.w.y = 0.0;
        help.w.z = 0.0;

        ViewportUniform {
            view_proj_matrix: vp.into(),
            view: view.into(),
            proj: proj.into(),
            inv_view_proj_dir: help.invert().unwrap().into()
        }
    }
}

