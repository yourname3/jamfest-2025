use std::cell::Cell;
use std::cmp::max;
use std::f32::consts::PI;

use grid::Grid;
use inline_tweak::tweak;
use ponygame::{game, gc};
// /
use ponygame::cgmath::{point3, vec3, AbsDiffEq, InnerSpace, Matrix4, SquareMatrix, Vector3};
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
    Goal(LaserValue),
    Hook,
    Ingot,
    Mix2,
    Nut,
    Bolt,
    Collect,
    Swap,
    Split,
}

impl DeviceTy {
    pub fn get_cells(&self) -> &'static [(i32, i32)] {
        match self {
            DeviceTy::Mix => &[(0, 0), (0, 1)],
            DeviceTy::Emitter(_) => &[(0, 0)],
            DeviceTy::Goal(_) => &[(0, 0)],
            DeviceTy::Hook => &[(0, 0), (0, 1)],
            DeviceTy::Ingot => &[(0, 0)],
            DeviceTy::Mix2 => &[(0, 0), (0, 2)],
            DeviceTy::Nut => &[(0, 0)],
            DeviceTy::Bolt => &[(0, 0)],
            DeviceTy::Collect => &[(0, 0), (0, 1), (0, 2)],
            DeviceTy::Swap => &[(0, 0), (1, 2)],
            DeviceTy::Split => &[(0, 0), (0, 2), (0, 4)],
        }
    }

    pub fn get_bounds(&self) -> (i32, i32) {
        match self {
            DeviceTy::Mix => (1, 2),
            DeviceTy::Emitter(_) => (1, 1),
            DeviceTy::Goal(_) => (1, 1),
            DeviceTy::Hook => (1, 2),
            DeviceTy::Ingot => (1, 1),
            DeviceTy::Mix2 => (1, 3),
            DeviceTy::Nut => (1, 1),
            DeviceTy::Bolt => (1, 1),
            DeviceTy::Collect => (1, 3),
            DeviceTy::Swap => (2, 3),
            DeviceTy::Split => (1, 5),
        }
    }

    pub fn get_selector(&self) -> SelectorState {
        match self {
            DeviceTy::Mix => SelectorState::Vert2,
            // Not selectable
            DeviceTy::Emitter(_) => SelectorState::Vert1,
            DeviceTy::Goal(_) => SelectorState::Vert1,
            DeviceTy::Hook => SelectorState::Vert2,

            DeviceTy::Ingot => SelectorState::Vert1,
            DeviceTy::Mix2 => SelectorState::OO,
            DeviceTy::Nut => SelectorState::Vert1,
            DeviceTy::Bolt => SelectorState::Vert1,
            DeviceTy::Collect => SelectorState::V3,
            DeviceTy::Swap => SelectorState::Swap,
            DeviceTy::Split => SelectorState::OOO,
        }
    }

    pub fn mk_mesh_instance(&self, engine: &PonyGame, assets: &Assets, locked: bool, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
        let (mesh, mat) = match self {
            DeviceTy::Mix => (&assets.node_mix, &assets.node_mix_mat),
            DeviceTy::Emitter(_) => (&assets.emitter, &assets.emitter_mat),
            DeviceTy::Goal(_) => (&assets.goal, &assets.goal_mat),
            DeviceTy::Hook => (&assets.node_hook, &assets.node_hook_mat),
            DeviceTy::Ingot => (&assets.node_ingot, &assets.node_ingot_mat),
            DeviceTy::Mix2 => (&assets.node_mix2, &assets.node_mix2_mat),
            DeviceTy::Nut => (&assets.node_nut, &assets.node_nut_mat),
            DeviceTy::Bolt => (&assets.node_bolt, &assets.node_bolt_mat),
            DeviceTy::Collect => (&assets.node_collect, &assets.node_collect_mat),
            DeviceTy::Swap => (&assets.node_swap, &assets.node_swap_mat),
            DeviceTy::Split => (&assets.node_split, &assets.node_split_mat),
        };
        Gp::new(MeshInstance::new(
            engine.render_ctx(),
            mesh.clone(),
            if locked { &mat.locked_mat } else { &mat.unlocked_mat }.clone(),
            transform
        ))
    }

    // Returns whether this fulfilled a goal.
    pub fn make_lasers(&self, x: i32, y: i32, lasers: &mut Vec<Laser>, ends_h: &Grid<Option<LaserValue>>) -> bool {
        match self {
            DeviceTy::Mix => {
                let Some(Some(left)) = ends_h.get(y as usize, x as usize) else { return false; };
                let Some(Some(right)) = ends_h.get(y as usize + 1, x as usize) else { return false; };

                let mix = ((left.color + right.color) * 0.5).normalize();

                let laser = Laser {
                    x, y: y + 1, length: 0,
                    value: LaserValue::color(mix)
                };
                lasers.push(laser);
            },
            DeviceTy::Emitter(value) => {
                let laser = Laser { x, y, length: 0, value: value.clone() };
                lasers.push(laser);
            },
            DeviceTy::Goal(value) => {
                let Some(Some(input)) = ends_h.get(y as usize, x as usize) else { return false; };
                // Within 2 rgb's
                let colors_are_eq = value.color.abs_diff_eq(&input.color, 2.0 / 255.0);
                log::info!("goal: {:?} vs {:?}, are they equal? {}", value.color, input.color, colors_are_eq);
                return colors_are_eq;
            },
            DeviceTy::Hook => {
                let Some(Some(input)) = ends_h.get(y as usize, x as usize) else { return false; };

                lasers.push(Laser {
                    x, y, length: 0,
                    value: input.clone()
                });
                lasers.push(Laser {
                    x, y: y + 1, length: 0,
                    value: input.clone()
                });
            },

            // TODO: Implement the gameplay logic.
            _ => {}
        }
        false
    }
}

pub struct Device {
    pub x: Cell<i32>,
    pub y: Cell<i32>,
    pub locked: bool,
    pub ty: DeviceTy,
}
gc!(Device, 0x00080000_u64);

#[derive(Clone)]
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

fn load_level(path: &str) -> tiled::Map {
    let mut loader = Loader::with_reader(|path: &std::path::Path| -> std::io::Result<_> {
        tiled_file!(path, "./levels/test.tmx");
        tiled_file!(path, "./levels/locked_mixers.tmx");
        tiled_file!(path, "./levels/hook_something.tmx");
        tiled_file!(path, "./levels/intro_mix.tmx");
        tiled_file!(path, "./levels/intro.tmx");
        tiled_file!(path, "./levels/intro_mix_simpler.tmx");
        tiled_file!(path, "./levels/intro_mix_constrained.tmx");
        tiled_file!(path, "./levels/tileset.tsx");

        Err(std::io::ErrorKind::NotFound.into())
    });

    loader.load_tmx_map(path).unwrap()
}

pub struct Level {
    pub floor_meshes: Vec<Gp<MeshInstance>>,
    pub grid: Grid<GridCell>,
    pub floor_grid: Grid<GridCell>,
    pub lasers: Vec<Laser>,
    pub h_laser_ends: Grid<Option<LaserValue>>,
    // I guess for now, we won't worry about making the bounds consistently
    // tiles or not.
    pub bounds: (u32, u32, u32, u32),

    pub nr_goals: usize,
    pub nr_goals_fulfilled: usize,
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

    pub fn new_from_map(map_path: &str, engine: &PonyGame, assets: &Assets) -> Level {

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
        const GOAL   : u32 = 4;

        const MIX    : u32 = 5;
        const HOOK   : u32 = 6;
        const INGOT  : u32 = 7;
        const MIX2   : u32 = 8;
        const NUT    : u32 = 9;
        const BOLT   : u32 = 10;
        const COLLECT: u32 = 11;
        const SWAP   : u32 = 12;
        const SPLIT  : u32 = 13;

        let map = load_level(map_path);

        let mut level = Level {
            // Use column-major order so that when we iterate over 
            floor_meshes: Vec::new(),
            grid: Grid::new_with_order(map.height as usize, map.width as usize, grid::Order::ColumnMajor),
            floor_grid: Grid::new_with_order(map.height as usize, map.width as usize, grid::Order::ColumnMajor),
            lasers: Vec::new(),
            h_laser_ends: Grid::new_with_order(map.height as usize, map.width as usize, grid::Order::ColumnMajor),
            bounds: (0, 0, 0, 0),
            nr_goals: 0,
            nr_goals_fulfilled: 0,
        };

        let floors = map.get_layer(0).unwrap().as_tile_layer().unwrap();

        let mut actual_bounds = None;

        let width = floors.width().unwrap();
        let height = floors.height().unwrap();
        for x in 0..width {
            for y in 0..height {
                if let Some(tile) = floors.get_tile(x as i32, y as i32) {
                    let Some((mut min_x, mut min_y, mut max_x, mut max_y)) = actual_bounds else { actual_bounds = Some((x, y, x, y)); continue; };

                    if x < min_x { min_x = x; }
                    if x > max_x { max_x = x; }

                    if y < min_y { min_y = y; }
                    if y > max_y { max_y = y; }

                    actual_bounds = Some((min_x, min_y, max_x, max_y));
                }
            }
        }

        // This will crash if the level doesn't have a single tile, which is fine.
        level.bounds = actual_bounds.unwrap();
 
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
                        *level.floor_grid.get_mut(y as u32, x as u32).unwrap() = GridCell::Empty;
                    }
                }
            }
        }

        let objects = map.get_layer(1).unwrap().as_object_layer().unwrap();
        for obj in objects.objects() {
            let id = obj.get_tile().unwrap().id();
            log::info!("object @ {}, {} => {}", obj.x, obj.y, id);

            let x = f32::floor(obj.x / 32.0) as i32;
            let y = f32::floor( (obj.y - 32.0) / 32.0) as i32;
            let locked = match obj.properties.get("locked") {
                Some(PropertyValue::BoolValue(locked)) => *locked,
                // Default to unlocked.
                _ => false,
            };

            match id {
                EMITTER | GOAL => {
                    let color = match obj.properties.get("color") {
                        Some(PropertyValue::ColorValue(color)) => {
                            vec3(color.red as f32 / 255.0, color.green as f32 / 255.0, color.blue as f32 / 255.0)
                        },
                        _ => vec3(1.0, 1.0, 1.0)
                    };

                    let ty = if id == EMITTER { DeviceTy::Emitter(LaserValue::color(color)) }
                        else { DeviceTy::Goal(LaserValue::color(color)) };

                    // Use force place because level objects can be overlapping
                    // the void / the partial-void.
                    // Note: Tiled's Y positions are very weird. Subtract 32
                    level.force_place(x, y, ty, locked);

                    if id == GOAL {
                        level.nr_goals += 1;
                    }
                },
                MIX => level.force_place(x, y, DeviceTy::Mix, locked),
                HOOK => level.force_place(x, y, DeviceTy::Hook, locked),
                INGOT => level.force_place(x, y, DeviceTy::Ingot, locked),
                MIX2 => level.force_place(x, y, DeviceTy::Mix2, locked),
                NUT => level.force_place(x, y, DeviceTy::Nut, locked),
                BOLT => level.force_place(x, y, DeviceTy::Bolt, locked),
                COLLECT => level.force_place(x, y, DeviceTy::Collect, locked),
                SWAP => level.force_place(x, y, DeviceTy::Swap, locked),
                SPLIT => level.force_place(x, y, DeviceTy::Split, locked),
                _ => {}
            }
        }

        //level.build_floor_meshes(engine, assets);

        level
    }

    pub fn setup_camera(&self, engine: &mut PonyGame) {
        let x_pos = (self.bounds.0 + self.bounds.2 + 1) as f32 / 2.0;
        let y_pos = (self.bounds.1 + self.bounds.3 + 1) as f32 / 2.0;

        let viewport = engine.get_viewport();
        // Zoom is based on height, so base the desired value from horizontal on
        // the height/width conversion factor.
        let desired_zoom_h = ((self.bounds.2 - self.bounds.0) + 2) as f32 * viewport.height as f32 / viewport.width as f32;
        let desired_zoom_w = ((self.bounds.3 - self.bounds.1) as f32 + 2.0 * 0.66);

        engine.main_camera.position.set(point3(x_pos, 15.0, y_pos + 3.0));
        engine.main_camera.target.set(point3(x_pos, 0.0, y_pos + 0.0));
        engine.main_camera.projection.set(CameraProjection::Orthographic {
            zoom: if desired_zoom_h > desired_zoom_w { desired_zoom_h } else { desired_zoom_w },
        });
    }

    pub fn is_in_bounds_and_empty(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        log::info!("checking @ {},{} => {:?}", x, y, self.grid.get(y as usize, x as usize));
        return matches!(self.grid.get(y as usize, x as usize).unwrap(), GridCell::Empty);
    }

    pub fn is_psuedo_in_bounds_and_empty(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        log::info!("checking @ {},{} => {:?}", x, y, self.grid.get(y as usize, x as usize));
        return matches!(self.grid.get(y as usize, x as usize).unwrap(), GridCell::Empty | GridCell::Void);
    }

    pub fn is_in_bounds(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        return true;
    }

    pub fn is_in_bounds_and_laser_travelable(&self, x: i32, y: i32) -> bool {
        if x < 0 || y < 0 { return false; }
        if x as usize >= self.grid.cols() { return false; }
        if y as usize >= self.grid.rows() { return false; }

        // Safety: We've confirmed they're positive.
        return matches!(self.grid.get(y as usize, x as usize).unwrap(), GridCell::Empty | GridCell::Void);
    }

    fn force_place_existing(&mut self, x: i32, y: i32, dev: &Gp<Device>) {
        dev.x.set(x);
        dev.y.set(y);

        for cell in dev.ty.get_cells() {
            *self.grid.get_mut((y + cell.1) as usize, (x + cell.0) as usize).unwrap() = 
                if matches!(cell, (0, 0)) { GridCell::DeviceRoot(dev.clone()) } 
                else { GridCell::DeviceEtc(dev.clone()) };
        }
    }

    fn force_place(&mut self, x: i32, y: i32, ty: DeviceTy, locked: bool) {
        log::info!("placed {:?} @ {},{}", ty, x, y);

        let device = Gp::new(Device {
            x: Cell::new(x), y: Cell::new(y), ty: ty.clone(), locked
        });

        self.force_place_existing(x, y, &device);
    }

    pub fn may_place_at(&mut self, x: i32, y: i32, ty: &DeviceTy) -> bool {
        for cell in ty.get_cells() {
            if !self.is_in_bounds_and_empty(x + cell.0, y + cell.1) {
                log::info!("failed due to {}, {} being {:?}",  x + cell.0, y + cell.1, self.grid.get(y + cell.1, x + cell.0));
                return false;
            }
        }
        return true;
    }

    pub fn may_psuedo_place_at(&mut self, x: i32, y: i32, ty: &DeviceTy) -> bool {
        for cell in ty.get_cells() {
            if !self.is_psuedo_in_bounds_and_empty(x + cell.0, y + cell.1) {
                log::info!("failed due to {}, {} being {:?}",  x + cell.0, y + cell.1, self.grid.get(y + cell.1, x + cell.0));
                return false;
            }
        }
        return true;
    }

    // pub fn try_place(&mut self, x: i32, y: i32, ty: DeviceTy) {
    //     log::info!("try_place @ {},{}", x, y);

    //     if !self.may_place_at(x, y, &ty) { return; }

    //     self.force_place(x, y, ty);
    // }

    /// Removes the given device's bounding box from the given location.
    /// Should only be called if we believe the device is at that loccation.
    fn clear_at(&mut self, x: i32, y: i32, dev: &Gp<Device>) {
        for cell in dev.ty.get_cells() {
            let x = x + cell.0;
            let y = y + cell.1;

            assert!(self.is_in_bounds(x, y));
            *self.grid.get_mut(y as usize, x as usize).unwrap() = self.floor_grid.get(y as usize, x as usize).unwrap().clone();
        }
    }

    /// Returns whether this is a valid placement.
    pub fn move_from(&mut self, x: i32, y: i32, dev: &Gp<Device>, to_x: i32, to_y: i32) -> bool {
        // Remove the device from its starting location and from wherever it is
        // currently located.
        self.clear_at(x, y, dev);
        self.clear_at(dev.x.get(), dev.y.get(), dev);

        if self.may_place_at(to_x, to_y, &dev.ty) {
            self.force_place_existing(to_x, to_y, dev);
            return true;
        }
        else if self.may_psuedo_place_at(to_x, to_y, &dev.ty) {
            self.force_place_existing(to_x, to_y, dev);
            return false;
        }
        else {
            self.force_place_existing(x, y, dev);
            return false;
        }
    }

    /// Like move_from, but doesn't allow for psuedo placements.
    pub fn finish_move_from(&mut self, x: i32, y: i32, dev: &Gp<Device>, to_x: i32, to_y: i32) {
        self.clear_at(x, y, dev);
        self.clear_at(dev.x.get(), dev.y.get(), dev);

        if self.may_place_at(to_x, to_y, &dev.ty) {
            log::info!("finish_move_from: valid placement!");
            self.force_place_existing(to_x, to_y, dev);
        }
        else {
            self.force_place_existing(x, y, dev);
        }
    }

    pub fn build_meshes(&mut self, engine: &mut PonyGame, assets: &Assets) {
        for ((y, x), cell) in self.grid.indexed_iter() {
            //log::info!("cell @ {},{} => {:?}", x, y, std::mem::discriminant(cell));
            match cell {
                GridCell::DeviceRoot(device) => {
                    let mat = Matrix4::from_translation(vec3(x as f32, 0.0, y as f32));

                    if let DeviceTy::Goal(value) = &device.ty {
                        //log::info!("pushing orb with color {:?}", value.color);
                        engine.main_world.push_mesh(assets.goal_light(engine.render_ctx(), mat.clone(),
                            value.color))
                    }

                    engine.main_world.push_mesh(device.ty.mk_mesh_instance(engine, assets, device.locked, mat))
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

        let mut nr_goals_fulfilled = 0;

        for x in 0..self.grid.cols() {
            for y in 0..self.grid.rows() {
                let cell = self.grid.get(y, x).unwrap();
                //log::info!("cell @ {},{} => {:?}", x, y, std::mem::discriminant(cell));
                match cell {
                    GridCell::DeviceRoot(device) => {
                        let goal = device.ty.make_lasers(x as i32, y as i32, &mut self.lasers, &self.h_laser_ends);
                        if goal { nr_goals_fulfilled += 1; }
                        while laser_len < self.lasers.len() {
                            self.extend_laser_h(laser_len);
                            laser_len += 1;
                        }
                    },
                    _ => {}
                }
            }
        }

        self.nr_goals_fulfilled = nr_goals_fulfilled;
    }
}