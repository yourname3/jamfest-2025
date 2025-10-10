use std::cell::Cell;

use cgmath::SquareMatrix;

use crate::video::{texture::Texture, world::{Viewport, ViewportUniform}, RenderCtx, UniformBuffer};

pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::from_cols(
    cgmath::Vector4::new(1.0, 0.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 1.0, 0.0, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 0.0),
    cgmath::Vector4::new(0.0, 0.0, 0.5, 1.0),
);

pub struct Camera {
    pub position: Cell<cgmath::Point3<f32>>,
    pub target: Cell<cgmath::Point3<f32>>,
    pub up: cgmath::Vector3<f32>,

    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32
}

impl Camera {
    pub fn demo() -> Self {
        Camera {
            position: Cell::new((0.0, 0.0, 5.0).into()),
            target: Cell::new((0.0, 0.0, 0.0).into()),
            up: (0.0, 1.0, 0.0).into(),
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        }
    }

    // TODO: We really should cache this...
    pub fn get_view_matrix(&self) -> cgmath::Matrix4<f32> {
        cgmath::Matrix4::look_at_rh(self.position.get(), self.target.get(), self.up)
    }

    pub fn build_vp(&self, viewport: &Viewport) -> cgmath::Matrix4<f32> {
        // TODO: Cache view & projection matrices independently (and also do
        // one projection-matrix per viewport)
        let view = cgmath::Matrix4::look_at_rh(self.position.get(), self.target.get(), self.up);

        let aspect: f32 = viewport.width as f32 / viewport.height as f32;

        let proj = cgmath::perspective(cgmath::Deg(self.fovy),
            aspect, self.znear, self.zfar);

        return /*OPENGL_TO_WGPU_MATRIX * */proj * view;
    }

    pub fn to_viewport_uniform(&self, viewport: &Viewport) -> ViewportUniform {
        // TODO: Cache view & projection matrices independently (and also do
        // one projection-matrix per viewport)
        let view = cgmath::Matrix4::look_at_rh(self.position.get(), self.target.get(), self.up);

        let aspect: f32 = viewport.width as f32 / viewport.height as f32;

        let proj = cgmath::perspective(cgmath::Deg(self.fovy),
            aspect, self.znear, self.zfar);

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

