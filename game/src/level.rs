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
use tiled::{LayerTile, Loader, PropertyValue, ResourceReader};

use crate::{Assets, SelectorState};

#[derive(Clone, Debug)]
pub enum DeviceTy {
    Mix,
    Emitter(LaserValue),
}

impl DeviceTy {
    pub fn get_cells(&self) -> &'static [(i32, i32)] {
        match self {
            DeviceTy::Mix => &[(0, 0), (0, 1)],
            DeviceTy::Emitter(_) => &[(0, 0)],
        }
    }

    pub fn get_bounds(&self) -> (i32, i32) {
        match self {
            DeviceTy::Mix => (1, 2),
            DeviceTy::Emitter(_) => (1, 1)
        }
    }

    pub fn get_selector(&self) -> SelectorState {
        match self {
            DeviceTy::Mix => SelectorState::Vert2,
            // Not selectable
            DeviceTy::Emitter(_) => SelectorState::None,
        }
    }

    pub fn mk_mesh_instance(&self, engine: &PonyGame, assets: &Assets, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
        let (mesh, mat) = match self {
            DeviceTy::Mix => (&assets.node_mix, &assets.node_mix_mat),
            DeviceTy::Emitter(_) => (&assets.emitter, &assets.emitter_mat),
        };
        Gp::new(MeshInstance::new(
            engine.render_ctx(),
            mesh.clone(),
            mat.clone(),
            transform
        ))
    }

    pub fn make_lasers(&self, x: i32, y: i32, lasers: &mut Vec<Laser>, ends_h: &Grid<Option<LaserValue>>) {
        match self {
            DeviceTy::Mix => {
                let Some(Some(left)) = ends_h.get(y as usize, x as usize) else { return; };
                let Some(Some(right)) = ends_h.get(y as usize + 1, x as usize) else { return; };

                let laser = Laser {
                    x, y: y + 1, length: 0,
                    value: LaserValue::color((left.color + right.color) * 0.5)
                };
                lasers.push(laser);
            },
            DeviceTy::Emitter(value) => {
                let laser = Laser { x, y, length: 0, value: value.clone() };
                lasers.push(laser);
            }
        }
    }
}

pub struct Device {
    pub x: i32,
    pub y: i32,
    pub ty: DeviceTy,
}
gc!(Device, 0x00080000_u64);

pub enum GridCell {
    /// The void: We can't actually place anything in the void.
    Void,
    /// Empty tile: We can place things there.
    Empty,
    Wall,
    DeviceRoot(Gp<Device>),
    DeviceEtc(Gp<Device>),
}

impl std::fmt::Debug for GridCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Void => write!(f, "Void"),
            Self::Empty => write!(f, "Empty"),
            Self::Wall => write!(f, "Wall"),
            Self::DeviceRoot(arg0) => write!(f, "DeviceRoot"),
            Self::DeviceEtc(arg0) => write!(f, "DeviceEtc"),
        }
    }
}

impl Default for GridCell {
    fn default() -> Self {
        GridCell::Void
    }
}

#[derive(Clone, Debug)]
pub struct LaserValue {
    pub color: Vector3<f32>,
}

impl LaserValue {
    pub fn color(color: Vector3<f32>) -> Self {
        LaserValue { color }
    }
}

pub struct Laser {
    pub x: i32,
    pub y: i32,
    pub length: i32,
    pub value: LaserValue,
}

pub struct LevelLoader {

}

macro_rules! tiled_file {
    ($path_arg:expr, $path:expr) => {
        if $path_arg == std::path::Path::new($path) {
            return Ok(std::io::Cursor::new(Vec::from(include_bytes!($path))))
        }
    }
}

fn load_level(path: &'static str) -> tiled::Map {
    let mut loader = Loader::with_reader(|path: &std::path::Path| -> std::io::Result<_> {
        tiled_file!(path, "./levels/test.tmx");
        tiled_file!(path, "./levels/tileset.tsx");

        Err(std::io::ErrorKind::NotFound.into())
    });

    loader.load_tmx_map(path).unwrap()
}

pub struct Level {
    pub floor_meshes: Vec<Gp<MeshInstance>>,
    pub grid: Grid<GridCell>,
    pub lasers: Vec<Laser>,
    pub h_laser_ends: Grid<Option<LaserValue>>,
}

impl Level {
    // /// Shouldl be called once, to instantiate all the floor meshes.
    // fn build_floor_meshes(&mut self, engine: &PonyGame, assets: &Assets) {
    //     // For now, just put one every spot on the grid...?
    //     for ((y, x), cell) in self.grid.indexed_iter() {
    //         let (mesh, mat) = match cell {
    //             GridCell::Wall => (&assets.wall_, &assets.wall_mat),
    //             _ => (&assets.floor_tile, &assets.floor_tile_mat)
    //         };

    //         let instance = Gp::new(MeshInstance::new(
    //             engine.render_ctx(),
    //             mesh.clone(),
    //             mat.clone(),
    //             Matrix4::from_translation(vec3(x as f32, 0.0, y as f32))
    //         ));
    //         //log::info!("instantiate floor mesh @ {},{}", x, y);
    //         self.floor_meshes.push(instance);
    //     }
    // }

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

    // pub fn new(width: usize, height: usize, engine: &PonyGame, assets: &Assets) -> Level {
    //     let mut level = Level {
    //         // Use column-major order so that when we iterate over 
    //         floor_meshes: Vec::new(),
    //         grid: Grid::new_with_order(height, width, grid::Order::ColumnMajor),
    //         lasers: Vec::new(),
    //         h_laser_ends: Grid::new_with_order(height, width, grid::Order::ColumnMajor),
    //     };

    //     level.build_debug_border();
    //     level.build_floor_meshes(engine, assets);

    //     level
    // }

    pub fn spawn_static_tile(&mut self, engine: &PonyGame, mesh: &Gp<Mesh>, mat: &Gp<PBRMaterial>, x: i32, y: i32) {
        let instance = Gp::new(MeshInstance::new(
            engine.render_ctx(),
            mesh.clone(),
            mat.clone(),
            Matrix4::from_translation(vec3(x as f32, 0.0, y as f32))
        ));
        //log::info!("instantiate floor mesh @ {},{}", x, y);
        self.floor_meshes.push(instance);
    }

    pub fn new_from_map(map_path: &'static str, engine: &PonyGame, assets: &Assets) -> Level {

        const WALL_TL: u32 = 0;
        const WALL_T : u32 = 1;
        const WALL_TR: u32 = 2;
        const WALL_L : u32 = 16;
        const FLOOR  : u32 = 17;
        const WALL_R : u32 = 18;
        const WALL_BL: u32 = 32;
        const WALL_B : u32 = 33;
        const WALL_BR: u32 = 34;

        const WALL_BR_I: u32 = 48;
        const WALL_BL_I: u32 = 50;
        const WALL_TR_I: u32 = 80;
        const WALL_TL_I: u32 = 82;

        const EMITTER: u32 = 3;

        let map = load_level(map_path);

        let mut level = Level {
            // Use column-major order so that when we iterate over 
            floor_meshes: Vec::new(),
            grid: Grid::new_with_order(map.height as usize, map.width as usize, grid::Order::ColumnMajor),
            lasers: Vec::new(),
            h_laser_ends: Grid::new_with_order(map.height as usize, map.width as usize, grid::Order::ColumnMajor),
        };

        let floors = map.get_layer(0).unwrap().as_tile_layer().unwrap();
        let width = floors.width().unwrap();
        let height = floors.height().unwrap();
        for x in 0..width {
            for y in 0..height {
                if let Some(tile) = floors.get_tile(x as i32, y as i32) {
                    let mesh_mat = match tile.id() {
                        WALL_TL => Some((&assets.wall_tl, &assets.wall_mat)),
                        WALL_T  => Some((&assets.wall_t , &assets.wall_mat)),
                        WALL_TR => Some((&assets.wall_tr, &assets.wall_mat)),
                        WALL_L  => Some((&assets.wall_l , &assets.wall_mat)),

                        WALL_R  => Some((&assets.wall_r , &assets.wall_mat)),
                        WALL_BL => Some((&assets.wall_bl, &assets.wall_mat)),
                        WALL_B  => Some((&assets.wall_b, &assets.wall_mat)),
                        WALL_BR => Some((&assets.wall_br, &assets.wall_mat)),

                        WALL_BR_I => Some((&assets.wall_br_i, &assets.wall_mat)),
                        WALL_BL_I => Some((&assets.wall_bl_i, &assets.wall_mat)),
                        WALL_TR_I => Some((&assets.wall_tr_i, &assets.wall_mat)),
                        WALL_TL_I => Some((&assets.wall_tl_i, &assets.wall_mat)),

                        FLOOR  => Some((&assets.floor_tile, &assets.floor_tile_mat)),
                        _ => None,
                    };

                    if let Some((mesh, mat)) = mesh_mat {
                        level.spawn_static_tile(engine, mesh, mat, x as i32, y as i32);
                    }

                    // Objects are placeable on the floor
                    if matches!(tile.id(), FLOOR) {
                        *level.grid.get_mut(y as u32, x as u32).unwrap() = GridCell::Empty;
                    }
                }
            }
        }

        let objects = map.get_layer(1).unwrap().as_object_layer().unwrap();
        for obj in objects.objects() {
            let id = obj.get_tile().unwrap().id();
            log::info!("object @ {}, {} => {}", obj.x, obj.y, id);
            match id {
                EMITTER => {
                    let color = match obj.properties.get("color") {
                        Some(PropertyValue::ColorValue(color)) => {
                            vec3(color.red as f32 / 255.0, color.green as f32 / 255.0, color.blue as f32 / 255.0)
                        },
                        _ => vec3(1.0, 1.0, 1.0)
                    };
                    // Use force place because level objects can be overlapping
                    // the void / the partial-void.
                    // Note: Tiled's Y positions are very weird. Subtract 32
                    level.force_place(f32::floor(obj.x / 32.0) as i32,f32::floor( (obj.y - 32.0) / 32.0) as i32,
                        DeviceTy::Emitter(LaserValue::color(color)));
                },
                _ => {}
            }
        }

        //level.build_floor_meshes(engine, assets);

        level
    }

    pub fn is_in_bounds_and_empty(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        log::info!("checking @ {},{} => {:?}", x, y, self.grid.get(y as usize, x as usize));
        return matches!(self.grid.get(y as usize, x as usize).unwrap(), GridCell::Empty);
    }

    pub fn is_in_bounds_and_laser_travelable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        return matches!(self.grid.get(y as usize, x as usize).unwrap(), GridCell::Empty | GridCell::Void);
    }

    fn force_place(&mut self, x: i32, y: i32, ty: DeviceTy) {
        log::info!("placed {:?} @ {},{}", ty, x, y);

        let device = Gp::new(Device {
            x, y, ty: ty.clone()
        });

        for cell in ty.get_cells() {
            *self.grid.get_mut((y + cell.1) as usize, (x + cell.0) as usize).unwrap() = 
                if matches!(cell, (0, 0)) { GridCell::DeviceRoot(device.clone()) } 
                else { GridCell::DeviceEtc(device.clone()) };
        }
    }

    pub fn try_place(&mut self, x: i32, y: i32, ty: DeviceTy) {
        log::info!("try_place @ {},{}", x, y);

        for cell in ty.get_cells() {
            if !self.is_in_bounds_and_empty(x + cell.0, y + cell.1) {
                log::info!("failed due to {}, {} being {:?}",  x + cell.0, y + cell.1, self.grid.get(y + cell.1, x + cell.0));
                return;
            }
        }

        self.force_place(x, y, ty);
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
                * Matrix4::from_nonuniform_scale(laser.length as f32, 1.0, 1.0),

                laser.value.color
            ))
        }

        for mesh in &self.floor_meshes {
            engine.main_world.push_mesh(mesh.clone());
        }
    }

    pub fn extend_laser_h(&mut self, idx: usize) {
        let Laser { mut x, y, .. } = self.lasers[idx];
        x += 1;
        while self.is_in_bounds_and_laser_travelable(x, y) {
            self.lasers[idx].length += 1;
            x += 1;
        }
        // Minimum length is 1??
        self.lasers[idx].length += 1;

        if (x as usize) < self.h_laser_ends.cols() {
            *self.h_laser_ends.get_mut(y, x).unwrap() = Some(self.lasers[idx].value.clone());
        }
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
                        device.ty.make_lasers(x as i32, y as i32, &mut self.lasers, &self.h_laser_ends);
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