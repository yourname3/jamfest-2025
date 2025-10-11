use std::f32::consts::PI;

use grid::Grid;
use inline_tweak::tweak;
use ponygame::{game, gc};
// /
use ponygame::cgmath::{point3, vec3, Matrix4, SquareMatrix};
use ponygame::cgmath;
use ponygame::log;

use ponygame::video::camera::CameraProjection;
use ponygame::video::{PBRShader, RenderCtx};
use ponygame::{audio::Sound, gc::{Gp, GpMaybe}, video::{asset_import::import_binary_data, mesh_render_pipeline::{Mesh, MeshInstance}, texture::Texture, PBRMaterial}, PonyGame};

struct Assets {
    horse_mesh: Gp<Mesh>,
    horse_material: Gp<PBRMaterial>,

    node_mix: Gp<Mesh>,
    
    node_mix_mat: Gp<PBRMaterial>,

    laser: Gp<Mesh>,
    laser_mat: Gp<PBRMaterial>,

    sfx0: Sound,
}

#[derive(Clone, Copy, Debug)]
enum DeviceTy {
    Mix,
}

impl DeviceTy {
    pub fn get_cells(&self) -> &'static [(i32, i32)] {
        match self {
            DeviceTy::Mix => &[(0, 0), (0, 1)],
        }
    }

    pub fn mk_mesh_instance(&self, engine: &PonyGame, assets: &Assets, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
        let (mesh, mat) = match self {
            DeviceTy::Mix => (&assets.node_mix, &assets.node_mix_mat),
        };
        Gp::new(MeshInstance::new(
            engine.render_ctx(),
            mesh.clone(),
            mat.clone(),
            transform
        ))
    }
}

struct Device {
    x: i32,
    y: i32,
    ty: DeviceTy,
}
gc!(Device, 0x00080000_u64);

enum GridCell {
    Empty,
    DeviceRoot(Gp<Device>),
    DeviceEtc(Gp<Device>),
}

impl Default for GridCell {
    fn default() -> Self {
        GridCell::Empty
    }
}

struct Level {
    grid: Grid<GridCell>,
}

impl Level {
    pub fn new(width: usize, height: usize) -> Level {
        Level {
            grid: Grid::new(height, width)
        }
    }

    pub fn is_in_bounds_and_empty(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        return matches!(self.grid.get(x as usize, y as usize).unwrap(), GridCell::Empty);
    }

    pub fn try_place(&mut self, x: i32, y: i32, ty: DeviceTy) {
        for cell in ty.get_cells() {
            if !self.is_in_bounds_and_empty(x + cell.0, y + cell.1) {
                return;
            }
        }

        let device = Gp::new(Device {
            x, y, ty
        });

        log::info!("placed {:?} @ {},{}", ty, x, y);

        for cell in ty.get_cells() {
            *self.grid.get_mut((x + cell.0) as usize, (y + cell.1) as usize).unwrap() = 
                if matches!(cell, (0, 0)) { GridCell::DeviceRoot(device.clone()) } 
                else { GridCell::DeviceEtc(device.clone()) };
        }
    }

    pub fn build_meshes(&mut self, engine: &mut PonyGame, assets: &Assets) {
        for ((x, y), cell) in self.grid.indexed_iter() {
            log::info!("cell @ {},{} => {:?}", x, y, std::mem::discriminant(cell));
            match cell {
                GridCell::DeviceRoot(device) => {
                    engine.main_world.push_mesh(device.ty.mk_mesh_instance(engine, assets, 
                        Matrix4::from_translation(vec3(x as f32, 0.0, y as f32))))
                },
                _ => {}
            }
        }
    }
}

macro_rules! mesh {
    ($ctx:expr, $path:expr) => {
        Gp::new(Mesh::new($ctx, &import_binary_data(include_bytes!($path)).unwrap()))
    }
}

macro_rules! texture_srgb {
    ($ctx:expr, $path:expr) => {
        Texture::from_bytes_rgba8srgb($ctx, include_bytes!($path), Some($path), false).unwrap()
    }
}

macro_rules! texture_linear {
    ($ctx:expr, $path:expr) => {
        Texture::from_bytes_rgba8linear($ctx, include_bytes!($path), Some($path), false).unwrap()
    }
}

macro_rules! texture_dummy {
    ($ctx:expr) => {
        Texture::dummy($ctx, Some("texture_dummy"))
    }
}

macro_rules! sfx {
    ($path:expr) => {
        Sound::from_data(include_bytes!($path))
    }
}

impl Assets {
    pub fn new(engine: &mut PonyGame) -> Self {
        let ctx = engine.render_ctx();

        Assets {
            horse_mesh: mesh!(ctx, "../test/horse.glb"),
            horse_material: Gp::new(PBRMaterial {
                albedo: vec3(1.0, 1.0, 1.0),
                metallic: 0.03,
                roughness: 0.95,
                reflectance: 0.0,
                albedo_texture: texture_srgb!(ctx, "../test/horse_albedo.png"),
                ..PBRMaterial::default(ctx)
            }),

            node_mix: mesh!(ctx, "./assets/mix_node.glb"),
            node_mix_mat: Gp::new(PBRMaterial {
                metallic: 1.0,
                roughness: 1.0,
                reflectance: 0.5,
                //albedo: vec3(0.5, 0.5, 0.5),
                albedo_texture: texture_linear!(ctx, "./assets/mat/metal_031/albedo.png"),
                metallic_roughness_texture: texture_linear!(ctx, "./assets/mat/metal_031/pbr.png"),
                albedo_decal_texture: texture_srgb!(ctx, "./assets/label_mix.png"),
                ..PBRMaterial::default(ctx)
            }),

            laser: mesh!(ctx, "./assets/laser.glb"),
            laser_mat: Gp::new(PBRMaterial {
                shader: Gp::new(PBRShader::new(ctx, "laser.wgsl", include_str!("./shaders/laser.wgsl"))),
                ..PBRMaterial::default(ctx)
            }),

            sfx0: sfx!("../test/test_sfx.wav")
        }
    }

    fn node_mix(&self, ctx: &RenderCtx, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
        Gp::new(MeshInstance::new(ctx,
            self.node_mix.clone(),
            self.node_mix_mat.clone(),
            transform))
    }

    fn laser(&self, ctx: &RenderCtx, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
        Gp::new(MeshInstance::new(ctx,
            self.laser.clone(),
            self.laser_mat.clone(),
            transform))
    }
}


pub struct GameplayLogic {
    theta: f32,

    assets: Assets,
    level: Level,
}

// meow

macro_rules! tweak_vec3 {
    ($x:expr, $y:expr, $z:expr) => {
        vec3(inline_tweak::tweak!($x), inline_tweak::tweak!($y), inline_tweak::tweak!($z))
    };
}

impl GameplayLogic {
    #[inline_tweak::tweak_fn]
    pub fn tweak_scene(&mut self, engine: &mut PonyGame) {
        engine.main_world.lights[0].color.set(vec3(5.0, 5.0, 5.0));
    }
}

impl ponygame::Gameplay for GameplayLogic {
    const GAME_TITLE: &'static str = "JamFest";

    fn new(engine: &mut PonyGame) -> Self {
       let assets = Assets::new(engine);
       let ctx = engine.render_ctx();

        // let transform0 = cgmath::Matrix4::from_translation(vec3(-0.5, 0.0, 0.0));
        // let transform1 = cgmath::Matrix4::from_translation(vec3( 0.5, 0.0, 0.0));

        engine.main_world.set_envmap(&Gp::new(Texture::from_bytes_rgba16unorm(ctx,
            //include_bytes!("./assets/envmap_1k.exr"),
            include_bytes!("./assets/preller_drive_1k.exr"),
            Some("horn-koppe_spring_1k.exr"),
            true).unwrap()));

        engine.main_camera.position.set(point3(0.0, 15.0, 3.0));
        engine.main_camera.target.set(point3(0.0, 0.0, 0.0));
        engine.main_camera.projection.set(CameraProjection::Orthographic {
            zoom: 10.0,
        });

        let mut level = Level::new(40, 40);
        level.try_place(0, 0, DeviceTy::Mix);
        level.try_place(1, 1, DeviceTy::Mix);

        GameplayLogic {
            assets,
            theta: 0.0,
            level,
        }
    }

    fn tick(&mut self, engine: &mut PonyGame) {
        let ctx = engine.render_ctx();

        self.theta += 0.1;
        self.tweak_scene(engine);

        engine.main_world.clear_meshes();
        self.level.build_meshes(engine, &self.assets);

        //let offset = vec3(0.3 * f32::cos(self.theta), 0.0, 0.3 * f32::sin(self.theta));

        //engine.main_camera.position.set(point3(0.0, 15.0, 3.0) + offset);
        //engine.main_camera.target.set(point3(0.0, 0.0, 0.0) + offset);
        //game.main_camera.position.set(point3(15.0 * f32::cos(self.theta), 15.0 * f32::sin(self.theta), 0.0));
    }
}

game!(GameplayLogic);
