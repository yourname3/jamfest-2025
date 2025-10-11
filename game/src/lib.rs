use std::f32::consts::PI;

use grid::Grid;
use inline_tweak::tweak;
use ponygame::{game, gc};
// /
use ponygame::cgmath::{point3, vec3, Matrix4, SquareMatrix, Vector3};
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

    emitter: Gp<Mesh>,
    emitter_mat: Gp<PBRMaterial>,

    select_vert_2: Gp<Mesh>,
    select_mat: Gp<PBRMaterial>,

    floor_tile: Gp<Mesh>,
    floor_tile_mat: Gp<PBRMaterial>,

    wall_side: Gp<Mesh>,
    wall_mat: Gp<PBRMaterial>,

    sfx0: Sound,
}

enum SelectorState {
    None,
    Vert2,
}

struct Selector {
    mesh_vert_2: Gp<MeshInstance>,

    state: SelectorState,
    x: i32,
    y: i32,
}

impl Selector {
    pub fn new(ctx: &RenderCtx, assets: &Assets) -> Self {
        Selector {
            mesh_vert_2: Gp::new(MeshInstance::new(ctx,
                assets.select_vert_2.clone(),
                assets.select_mat.clone(),
                Matrix4::identity())),
            state: SelectorState::None,
            x: 0,
            y: 0,
        }
    }

    pub fn push_mesh(&mut self, engine: &mut PonyGame) {
        let transform = Matrix4::from_translation(vec3(self.x as f32, 0.0, self.y as f32));
        let mesh_instance = match self.state {
            SelectorState::None => None,
            SelectorState::Vert2 => Some(&self.mesh_vert_2),
        };

        if let Some(mesh_instance) = mesh_instance {
            mesh_instance.transform.set(transform);
            mesh_instance.update(engine.render_ctx());

            engine.main_world.push_mesh(mesh_instance.clone());
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum DeviceTy {
    Mix,
    Emitter,
}

impl DeviceTy {
    pub fn get_cells(&self) -> &'static [(i32, i32)] {
        match self {
            DeviceTy::Mix => &[(0, 0), (0, 1)],
            DeviceTy::Emitter => &[(0, 0)],
        }
    }

    pub fn mk_mesh_instance(&self, engine: &PonyGame, assets: &Assets, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
        let (mesh, mat) = match self {
            DeviceTy::Mix => (&assets.node_mix, &assets.node_mix_mat),
            DeviceTy::Emitter => (&assets.emitter, &assets.emitter_mat),
        };
        Gp::new(MeshInstance::new(
            engine.render_ctx(),
            mesh.clone(),
            mat.clone(),
            transform
        ))
    }

    pub fn make_lasers(&self, x: i32, y: i32, lasers: &mut Vec<Laser>) {
        match self {
            DeviceTy::Mix => {
                let laser = Laser { x, y: y + 1, length: 0, value: LaserValue { color: vec3(1.0, 0.0, 0.0) } };
                lasers.push(laser);
            },
            DeviceTy::Emitter => {
                let laser = Laser { x, y, length: 0, value: LaserValue { color: vec3(1.0, 0.0, 0.0) } };
                lasers.push(laser);
            }
        }
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
    Wall,
    DeviceRoot(Gp<Device>),
    DeviceEtc(Gp<Device>),
}

impl Default for GridCell {
    fn default() -> Self {
        GridCell::Empty
    }
}

#[derive(Clone)]
struct LaserValue {
    color: Vector3<f32>,
}

struct Laser {
    x: i32,
    y: i32,
    length: i32,
    value: LaserValue,
}

struct Level {
    floor_meshes: Vec<Gp<MeshInstance>>,
    grid: Grid<GridCell>,
    lasers: Vec<Laser>,
    h_laser_ends: Grid<Option<LaserValue>>,
}

impl Level {
    /// Shouldl be called once, to instantiate all the floor meshes.
    fn build_floor_meshes(&mut self, engine: &PonyGame, assets: &Assets) {
        // For now, just put one every spot on the grid...?
        for ((y, x), cell) in self.grid.indexed_iter() {
            let (mesh, mat) = match cell {
                GridCell::Wall => (&assets.wall_side, &assets.wall_mat),
                _ => (&assets.floor_tile, &assets.floor_tile_mat)
            };

            let instance = Gp::new(MeshInstance::new(
                engine.render_ctx(),
                mesh.clone(),
                mat.clone(),
                Matrix4::from_translation(vec3(x as f32, 0.0, y as f32))
            ));
            //log::info!("instantiate floor mesh @ {},{}", x, y);
            self.floor_meshes.push(instance);
        }
    }

    fn build_debug_border(&mut self) {
        for cell in self.grid.iter_row_mut(0) {
            *cell = GridCell::Wall;
        }
        for cell in self.grid.iter_row_mut(self.grid.rows() - 1) {
            *cell = GridCell::Wall;
        }

        for cell in self.grid.iter_col_mut(0) {
            *cell = GridCell::Wall;
        }
        for cell in self.grid.iter_col_mut(self.grid.cols() - 1) {
            *cell = GridCell::Wall;
        }
    }

    pub fn new(width: usize, height: usize, engine: &PonyGame, assets: &Assets) -> Level {
        let mut level = Level {
            // Use column-major order so that when we iterate over 
            floor_meshes: Vec::new(),
            grid: Grid::new_with_order(height, width, grid::Order::ColumnMajor),
            lasers: Vec::new(),
            h_laser_ends: Grid::new_with_order(height, width, grid::Order::ColumnMajor),
        };

        level.build_debug_border();
        level.build_floor_meshes(engine, assets);

        level
    }

    pub fn is_in_bounds_and_empty(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        return matches!(self.grid.get(y as usize, x as usize).unwrap(), GridCell::Empty);
    }

    pub fn try_place(&mut self, x: i32, y: i32, ty: DeviceTy) {
        for cell in ty.get_cells() {
            if !self.is_in_bounds_and_empty(y + cell.1, x + cell.0) {
                return;
            }
        }

        let device = Gp::new(Device {
            x, y, ty
        });

        //log::info!("placed {:?} @ {},{}", ty, x, y);

        for cell in ty.get_cells() {
            *self.grid.get_mut((y + cell.1) as usize, (x + cell.0) as usize).unwrap() = 
                if matches!(cell, (0, 0)) { GridCell::DeviceRoot(device.clone()) } 
                else { GridCell::DeviceEtc(device.clone()) };
        }
    }

    pub fn build_meshes(&mut self, engine: &mut PonyGame, assets: &Assets) {
        for ((y, x), cell) in self.grid.indexed_iter() {
            //log::info!("cell @ {},{} => {:?}", x, y, std::mem::discriminant(cell));
            match cell {
                GridCell::DeviceRoot(device) => {
                    engine.main_world.push_mesh(device.ty.mk_mesh_instance(engine, assets, 
                        Matrix4::from_translation(vec3(x as f32, 0.0, y as f32))))
                },
                _ => {}
            }
        }

        for laser in &self.lasers {
            engine.main_world.push_mesh(assets.laser(engine.render_ctx(),
                // Add a horizontal offset of 0.5 so that the laser is good. 
                Matrix4::from_translation(vec3(laser.x as f32 + 0.5, 0.0, laser.y as f32))
                * Matrix4::from_nonuniform_scale(laser.length as f32, 1.0, 1.0
            )))
        }

        for mesh in &self.floor_meshes {
            engine.main_world.push_mesh(mesh.clone());
        }
    }

    pub fn extend_laser_h(&mut self, idx: usize) {
        let Laser { mut x, y, .. } = self.lasers[idx];
        x += 1;
        while self.is_in_bounds_and_empty(x, y) {
            self.lasers[idx].length += 1;
            x += 1;
        }
        // Minimum length is 1??
        self.lasers[idx].length += 1;

        *self.h_laser_ends.get_mut(y, x - 1).unwrap() = Some(self.lasers[idx].value.clone());
    }

    pub fn build_lasers(&mut self) {
        // Clear the grid to all None.
        self.h_laser_ends.fill(None);
        self.lasers.clear();

        let mut laser_len = 0usize;

        for x in 0..self.grid.cols() {
            for y in 0..self.grid.rows() {
                let cell = self.grid.get(y, x).unwrap();
                //log::info!("cell @ {},{} => {:?}", x, y, std::mem::discriminant(cell));
                match cell {
                    GridCell::DeviceRoot(device) => {
                        device.ty.make_lasers(x as i32, y as i32, &mut self.lasers);
                        while laser_len < self.lasers.len() {
                            self.extend_laser_h(laser_len);
                            laser_len += 1;
                        }
                    },
                    _ => {}
                }
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

        let metal_031_a = texture_linear!(ctx, "./assets/mat/metal_031/albedo.png");
        let metal_031_m = texture_linear!(ctx, "./assets/mat/metal_031/pbr.png");

        let metal_046_a = texture_linear!(ctx, "./assets/mat/metal_046/albedo.png");
        let metal_046_m = texture_linear!(ctx, "./assets/mat/metal_046/pbr.png");

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
                albedo_texture: metal_031_a.clone(),
                metallic_roughness_texture: metal_031_m.clone(),
                albedo_decal_texture: texture_srgb!(ctx, "./assets/label_mix.png"),
                ..PBRMaterial::default(ctx)
            }),

            emitter: mesh!(ctx, "./assets/emitter.glb"),
            emitter_mat: Gp::new(PBRMaterial {
                albedo_texture: texture_linear!(ctx, "./assets/mat/brass_4k/albedo.png"),
                metallic_roughness_texture: texture_linear!(ctx, "./assets/mat/brass_4k/pbr.png"),
                albedo_decal_texture: texture_srgb!(ctx, "./assets/emitter_label.png"),
                ..PBRMaterial::default(ctx)
            }),

            select_vert_2: mesh!(ctx, "./assets/select_vert_2.glb"),
            select_mat: Gp::new(PBRMaterial {
                shader: Gp::new(PBRShader::new(ctx, "select.wgsl", include_str!("./shaders/select.wgsl"))),
                ..PBRMaterial::default(ctx)
            }),

            floor_tile: mesh!(ctx, "./assets/floor_tile.glb"),
            floor_tile_mat: Gp::new(PBRMaterial {
                albedo_texture: metal_046_a.clone(),
                metallic_roughness_texture: metal_046_m.clone(),
                ..PBRMaterial::default(ctx)
            }),

            wall_side: mesh!(ctx, "./assets/wall/wall_side.glb"),
            wall_mat: Gp::new(PBRMaterial {
                albedo_texture: metal_046_a.clone(),
                metallic_roughness_texture: metal_046_m.clone(),
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

    selector: Selector,
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
            include_bytes!("./assets/horn-koppe_spring_1k.exr"),
            Some("horn-koppe_spring_1k.exr"),
            true).unwrap()));

        engine.main_camera.position.set(point3(0.0, 15.0, 3.0));
        engine.main_camera.target.set(point3(0.0, 0.0, 0.0));
        engine.main_camera.projection.set(CameraProjection::Orthographic {
            zoom: 10.0,
        });

        let mut level = Level::new(40, 40, engine, &assets);
        level.try_place(0, 2, DeviceTy::Emitter);
        level.try_place(5, 2, DeviceTy::Mix);

        let selector = Selector::new(ctx, &assets);

        GameplayLogic {
            assets,
            theta: 0.0,
            level,
            selector,
        }
    }

    fn tick(&mut self, engine: &mut PonyGame) {
        let ctx = engine.render_ctx();

        self.theta += 0.1;
        self.tweak_scene(engine);

        self.level.build_lasers();

        engine.main_world.clear_meshes();
        self.level.build_meshes(engine, &self.assets);
        self.selector.push_mesh(engine);

        //let offset = vec3(0.3 * f32::cos(self.theta), 0.0, 0.3 * f32::sin(self.theta));

        //engine.main_camera.position.set(point3(0.0, 15.0, 3.0) + offset);
        //engine.main_camera.target.set(point3(0.0, 0.0, 0.0) + offset);
        //game.main_camera.position.set(point3(15.0 * f32::cos(self.theta), 15.0 * f32::sin(self.theta), 0.0));
    }
}

game!(GameplayLogic);
