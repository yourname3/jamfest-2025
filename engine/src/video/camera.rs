use std::cell::Cell;

use cgmath::{vec2, InnerSpace, Matrix4, SquareMatrix, Vector2, Vector3};

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

    /// Returns two coordinates: The root of the ray, and the direction vector of
    /// the ray.
    pub fn ray_from_normalized_device(&self, ndc: Vector2<f32>, viewport: &Viewport) -> (Vector3<f32>, Vector3<f32>) {
        let view_proj = self.get_view_projection_matrix(viewport);
        let inverse = view_proj.invert().unwrap();

        let root = inverse * ndc.extend(0.0).extend(1.0);
        let out = inverse * ndc.extend(1.0).extend(1.0);

        let dir = out.truncate() - root.truncate();
        (root.truncate(), out.truncate())
    }

    /// Takes a plane as a (Point, Normal) pair and returns the intersection of
    /// the given ray with that plane.
    pub fn intersect_ray_with_plane_from_ndc(&self, ndc: Vector2<f32>, viewport: &Viewport, plane: (Vector3<f32>, Vector3<f32>)) -> Option<Vector3<f32>> {
        let (ray_point, ray_dir) = self.ray_from_normalized_device(ndc, viewport);
        let denom = plane.1.dot(ray_dir);
        if denom.abs() < 1e-6 {
            return None;
        }
        let t = plane.1.dot(plane.0 - ray_point) / denom;
        if t < 0.0 { return None; }

        Some(ray_point + ray_dir * t)
    }

    pub fn convert_screen_to_normalized_device(&self, viewport: &Viewport, cursor_pos: Vector2<f32>) -> Vector2<f32> {
        let normalized = vec2(
            cursor_pos.x / (0.5 * viewport.width as f32),
            cursor_pos.y / (-0.5 * viewport.height as f32),
        ) + vec2(-1.0, 1.0);
        return normalized;
        // match self.projection.get() {
        //     CameraProjection::Perspective { fovy, znear, zfar } => todo!("uhhhhhh"),
        //     CameraProjection::Orthographic { zoom } => {
        //         //let cursor_pos = (cursor_pos - vec2(viewport.width as f32 * 0.5, viewport.height as f32 * 0.5));

        //         //return (cursor_pos * zoom) / (viewport.height as f32 * 2.0);

                

        //         //let proj = self.get_projection_matrix(viewport);
        //         //let proj = proj * Matrix4::from_scale(1.0 / viewport.width as f32);
        //         //let invert = proj.invert().unwrap();
        //         //(proj * cursor_pos.extend(0.0).extend(1.0)).truncate().truncate()
        //     },
        // }
    }

    pub fn get_view_projection_matrix(&self, viewport: &Viewport) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.position.get(), self.target.get(), self.up);

        let proj = self.get_projection_matrix(viewport);        

        let vp = OPENGL_TO_WGPU_MATRIX * proj * view;

        vp
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

