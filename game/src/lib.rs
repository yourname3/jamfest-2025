use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;

mod level;

use egui::{Align2};
use engine::input::MouseButton;
use engine::video::asset_import::import_mesh_set_as_gc;
use engine::video::hdr_tonemap::Tonemap;
use engine::winit::event::{TouchPhase, WindowEvent};
use engine::{game, gc};
// /
use engine::cgmath::{point3, vec2, vec3, vec4, Matrix4, SquareMatrix, Vector2, Vector3, Zero};
use engine::cgmath;
use engine::log;

use engine::video::camera::CameraProjection;
use engine::video::{PBRShader, RenderCtx};
use engine::{audio::Sound, gc::{Gp, GpMaybe}, video::{asset_import::import_binary_data, mesh_render_pipeline::{Mesh, MeshInstance}, texture::Texture, PBRMaterial}, Engine};

use level::*;
use smallrand::SmallRng;

// Used cause each material has separate decals, and we need to be able to swap
// out the color.
// 
// TODO / IMPROVEMENT: Make decals a per-meshinstance binding.
pub struct LockUnlockMat {
    pub locked_mat: Gp<PBRMaterial>,
    pub unlocked_mat: Gp<PBRMaterial>,
}

struct InstancePool {
    pools: RefCell<HashMap<(usize, usize), Vec<Gp<MeshInstance>>>>,
    outstanding: RefCell<HashMap<(usize, usize), Vec<Gp<MeshInstance>>>>,
}

impl InstancePool {
    pub fn new() -> Self {
        Self {
            pools: RefCell::new(HashMap::new()),
            outstanding: RefCell::new(HashMap::new()),
        }
    }

    pub fn get(&self, engine: &Engine, mesh: &Gp<Mesh>, mat: &Gp<PBRMaterial>) -> Gp<MeshInstance> {
        let key = (
            mesh.get_gc_value_ptr() as *const _ as usize,
            mat.get_gc_value_ptr() as *const _ as usize);
        let mut pools = self.pools.borrow_mut();
        let the_pool = pools
            .entry(key).or_insert(Vec::new());

        if the_pool.is_empty() {
            the_pool.push(Gp::new(MeshInstance::new(engine.render_ctx(),
                mesh.clone(),
                mat.clone(),
                Matrix4::identity())));
        }

        let retval = the_pool.pop().unwrap();

        let mut oustanding = self.outstanding.borrow_mut();
        oustanding.entry(key).or_insert(Vec::new()).push(retval.clone());

        retval
    }

    pub fn get_at(&self, engine: &Engine, mesh: &Gp<Mesh>, mat: &Gp<PBRMaterial>, transform: Matrix4<f32>) -> Gp<MeshInstance> {
        let mesh = self.get(engine, mesh, mat);
        mesh.transform.set(transform);
        mesh.update(engine.render_ctx());
        mesh
    }

    pub fn get_at2(&self, engine: &Engine, mesh: &Gp<Mesh>, mat: &Gp<PBRMaterial>, transform: Matrix4<f32>, modulate: cgmath::Vector4<f32>) -> Gp<MeshInstance> {
        let mesh = self.get(engine, mesh, mat);
        mesh.transform.set(transform);
        mesh.modulate.set(modulate);
        mesh.update(engine.render_ctx());
        mesh
    }

    pub fn recycle(&self) {
        let mut total: usize = 0;

        let mut outstanding = self.outstanding.borrow_mut();
        let mut pools = self.pools.borrow_mut();
        for (k, v) in outstanding.iter_mut() {
            total += v.len();
            pools.entry(*k).or_insert(Vec::new()).append(v);
            v.clear();
        }

        log::info!("recycled {} mesh instances", total);
    }
}

struct Rng {
    inner: RefCell<SmallRng>,
}

impl Rng {
    pub fn new() -> Self {
        Rng {
            // For this game, we don't care if we get the same output every time.
            inner: RefCell::new(SmallRng::from_seed(0))
        }
    }

    pub fn choose<'a, T>(&self, slice: &'a [T]) -> &'a T {
        let mut rng = self.inner.borrow_mut();
        let idx = rng.range::<usize>(0..slice.len());
        &slice[idx]
    }

    pub fn range(&self, range: Range<f32>) -> f32 {
        let mut rng = self.inner.borrow_mut();
        rng.range(range)
    }
}

struct Assets {
    horse_mesh: Gp<Mesh>,
    horse_material: Gp<PBRMaterial>,

    pool: InstancePool,
    pool_static: InstancePool,

    rng: Rng,

    node_mix: Gp<Mesh>,
    node_mix_mat: LockUnlockMat,
    node_hook: Gp<Mesh>,
    node_hook_mat: LockUnlockMat,

    metal_sfx: [Sound; 5],
    metal_pickup: Sound,
    metal_putdown: Sound,
    move_err: Sound,

    win: Sound,
    click: Sound,

    node_ingot: Gp<Mesh>,
    node_mix2: Gp<Mesh>,
    node_nut: Gp<Mesh>,
    node_bolt: Gp<Mesh>,
    #[expect(unused)]
    node_prism: Gp<Mesh>,
    node_split: Gp<Mesh>,
    node_swap: Gp<Mesh>,
    node_collect: Gp<Mesh>,

    node_ingot_mat: LockUnlockMat,
    node_mix2_mat: LockUnlockMat,
    node_nut_mat: LockUnlockMat,
    node_bolt_mat: LockUnlockMat,
    #[expect(unused)]
    node_prism_mat: LockUnlockMat,
    node_split_mat: LockUnlockMat,
    node_swap_mat: LockUnlockMat,
    node_collect_mat: LockUnlockMat,

    laser: Gp<Mesh>,
    laser_mat: Gp<PBRMaterial>,

    emitter: Gp<Mesh>,
    emitter_mat: LockUnlockMat,

    select_vert_1: Gp<Mesh>,
    select_vert_2: Gp<Mesh>,

    select_swap: Gp<Mesh>,
    select_o_o: Gp<Mesh>,
    select_o_o_o: Gp<Mesh>,
    select_v3: Gp<Mesh>,

    select_mat: Gp<PBRMaterial>,

    floor_tile: Gp<Mesh>,
    floor_tile_mat: Gp<PBRMaterial>,

    goal: Gp<Mesh>,
    goal_mat: LockUnlockMat,
    goal_light: Gp<Mesh>,
    goal_light_mat: Gp<PBRMaterial>,

    wall_tl: Gp<Mesh>,
    wall_t: Gp<Mesh>,
    wall_tr: Gp<Mesh>,
    wall_l: Gp<Mesh>,
    wall_r: Gp<Mesh>,
    wall_bl: Gp<Mesh>,
    wall_b: Gp<Mesh>,
    wall_br: Gp<Mesh>,

    wall_tl_i: Gp<Mesh>,
    wall_tr_i: Gp<Mesh>,
    wall_bl_i: Gp<Mesh>,
    wall_br_i: Gp<Mesh>,

    wall_mat: Gp<PBRMaterial>,
}

enum SelectorState {
    None,
    Vert1,
    Vert2,
    Swap,
    OO,
    OOO,
    V3,
}

enum SelectorMoveState {
    NotMoving,
    MovingWithMouse,
    /// Moving with the given touch ID.
    MovingWithTouch(u64),
}

struct Selector {
    mesh_vert_1: Gp<MeshInstance>,
    mesh_vert_2: Gp<MeshInstance>,
    mesh_swap: Gp<MeshInstance>,
    mesh_o_o: Gp<MeshInstance>,
    mesh_o_o_o: Gp<MeshInstance>,
    mesh_v3: Gp<MeshInstance>,

    state: SelectorState,
    object: GpMaybe<Device>,
    x: i32,
    y: i32,

    start_x: i32,
    start_y: i32,

    offset_x: f32,
    offset_y: f32,

    moving: SelectorMoveState,
    
    /// Contains the last touch start event that happened at the given point.
    touch_started: Option<u64>,
    touch_pos: Vector2<f32>,
}

impl Selector {
    fn mk_mesh_instance(ctx: &RenderCtx, assets: &Assets, mesh: &Gp<Mesh>) -> Gp<MeshInstance> {
        Gp::new(MeshInstance::new(ctx,
            mesh.clone(),
            assets.select_mat.clone(),
            Matrix4::identity()))
    }

    pub fn new(ctx: &RenderCtx, assets: &Assets) -> Self {
        Selector {
            mesh_vert_1: Self::mk_mesh_instance(ctx, assets, &assets.select_vert_1),
            mesh_vert_2: Self::mk_mesh_instance(ctx, assets, &assets.select_vert_2),
            mesh_swap: Self::mk_mesh_instance(ctx, assets, &assets.select_swap),
            mesh_o_o: Self::mk_mesh_instance(ctx, assets, &assets.select_o_o),
            mesh_o_o_o: Self::mk_mesh_instance(ctx, assets, &assets.select_o_o_o),
            mesh_v3: Self::mk_mesh_instance(ctx, assets, &assets.select_v3),
            state: SelectorState::None,
            object: GpMaybe::none(),
            x: 0,
            y: 0,

            start_x: 0,
            start_y: 0,

            offset_x: 0.0,
            offset_y: 0.0,

            moving: SelectorMoveState::NotMoving,
            touch_started: None,
            touch_pos: vec2(0.0, 0.0),
        }
    }

    fn get_current_mesh(&self) -> Option<&Gp<MeshInstance>> {
        match self.state {
            SelectorState::None  => None,
            SelectorState::Vert1 => Some(&self.mesh_vert_1),
            SelectorState::Vert2 => Some(&self.mesh_vert_2),
            SelectorState::Swap  => Some(&self.mesh_swap),
            SelectorState::OO    => Some(&self.mesh_o_o),
            SelectorState::OOO   => Some(&self.mesh_o_o_o),
            SelectorState::V3    => Some(&self.mesh_v3),
        }
    }

    pub fn push_mesh(&mut self, engine: &mut Engine) {
        let transform = Matrix4::from_translation(vec3(self.x as f32, 0.0, self.y as f32));
        let mesh_instance = self.get_current_mesh();

        if let Some(mesh_instance) = mesh_instance {
            mesh_instance.transform.set(transform);
            mesh_instance.update(engine.render_ctx());

            engine.main_world.push_mesh(mesh_instance.clone());
        }
    }

    pub fn do_move(&mut self, engine: &mut Engine, assets: &Assets, level: &mut Level, finish_now: bool) {
        if matches!(self.state, SelectorState::None) { return; }

        let Some(dev) = self.object.get() else { return; };

        let viewport = engine.get_viewport();

        let pos = match self.moving {
            SelectorMoveState::MovingWithMouse => engine.get_cursor_position(),
            SelectorMoveState::MovingWithTouch(id) => {
                // Updated in the engine
                self.touch_pos
            },
            SelectorMoveState::NotMoving => unreachable!(),
        };

        let pos = engine.main_camera.convert_screen_to_normalized_device(viewport, pos);
        
        let intersect = engine.main_camera.intersect_ray_with_plane_from_ndc(pos, viewport, (
            Vector3::zero(), Vector3::unit_y()
        )).unwrap();

        let (last_x, last_y) = (self.x, self.y);
        self.x = f32::round(intersect.x - self.offset_x) as i32;
        self.y = f32::round(intersect.z - self.offset_x) as i32;

        let valid = level.move_from(self.start_x, self.start_y, &dev, self.x, self.y);
        if let Some(mesh) = self.get_current_mesh() {
            mesh.modulate.set(if valid { vec4(1.0, 1.0, 1.0, 1.0) } else { vec4(1.0, 0.0, 0.0, 1.0) });
            mesh.update(engine.render_ctx());
        }

        if last_x != self.x || last_y != self.y {
            if valid {
                engine.audio.play_speed(assets.rng.choose(&assets.metal_sfx), assets.rng.range(0.95..1.05));
            }
            else {
                engine.audio.play_speed(&assets.move_err, assets.rng.range(0.95..1.05));
            }
        }

        if finish_now || !engine.input.is_mouse_pressed(MouseButton::Left) {
            self.moving = SelectorMoveState::NotMoving;
            level.finish_move_from(self.start_x, self.start_y, &dev, self.x, self.y);
            engine.audio.play_speed(&assets.metal_putdown, assets.rng.range(0.95..1.05));
        }
    }

    pub fn update(&mut self, engine: &mut Engine, assets: &Assets, level: &mut Level) {
        if !matches!(self.moving, SelectorMoveState::NotMoving) {
            self.do_move(engine, assets, level, false);
            return;
        }

        let pos = engine.get_cursor_position();
        let pos = engine.main_camera.convert_screen_to_normalized_device(engine.get_viewport(), pos);
        let vp = engine.main_camera.get_view_projection_matrix(engine.get_viewport());

        //log::info!("cursor pos @ {:?}", pos);

        //let Some(invert) = vp.invert() else { return; };

        self.state = SelectorState::None;
        // Take whatever touch event we might have from the last event().
        let touch = self.touch_started.take();
        self.object.set(None);

        for x in 0..level.grid.cols() {
            for y in 0..level.grid.rows() {
                let cell = level.grid.get(y, x).unwrap();
                if let GridCell::DeviceRoot(dev) = cell {
                    // Can't move locked devices.
                    if dev.locked { continue; }

                    let check_square = |subpoint: (i32, i32)| {
                        //let bounds = dev.ty.get_bounds();
                        let low_point = vec3(x as f32 + subpoint.0 as f32, 0.0, y as f32 + subpoint.1 as f32);
                        // Check 1x1 squares
                        let high_point = vec3(low_point.x + 1.0, 0.0, low_point.z + 1.0);

                        let low_point = (vp * low_point.extend(1.0)).truncate().truncate();
                        let high_point = (vp * high_point.extend(1.0)).truncate().truncate();

                        if pos.x < low_point.x || pos.x > high_point.x { return false; }
                        if pos.y < high_point.y || pos.y > low_point.y { return false; }
                        return true;
                    };

                    let check_all_squares = |points: &[(i32, i32)]| {
                        // If the mouse is in any of the device's squares, we
                        // will pick it up.
                        for point  in points {
                            if check_square(*point) { return true; }
                        }
                        return false;
                    };

                    if !check_all_squares(dev.ty.get_cells()) { continue; }

                    //log::info!("candidate object @ {:?} -> {:?}", low_point, high_point);

                    

                    // Cursor should be overlapping..
                    self.state = dev.ty.get_selector();
                    // If the object was not selectable, keep looking.
                    if matches!(self.state, SelectorState::None) { continue; }

                    self.object.set(Some(dev));
                    self.x = x as i32;
                    self.y = y as i32;
                    self.start_x = self.x;
                    self.start_y = self.y;

                    // intersect.xz = cursor poss, want cursor_pos - top left corner
                    // (X, Y)
                    let intersect = engine.main_camera.intersect_ray_with_plane_from_ndc(pos, engine.get_viewport(), (
                        Vector3::zero(), Vector3::unit_y()
                    )).unwrap();

                    self.offset_x = intersect.x - x as f32;
                    self.offset_y = intersect.z - y as f32;

                    // Update the mesh to not be red
                    if let Some(mesh) = self.get_current_mesh() {
                        mesh.modulate.set(vec4(1.0, 1.0, 1.0, 1.0));
                        mesh.update(engine.render_ctx());
                    }

                    let play_pickup_snd = || engine.audio.play_speed(&assets.metal_pickup, assets.rng.range(0.95..1.05));

                    if engine.input.is_mouse_just_pressed(MouseButton::Left) {
                        self.moving = SelectorMoveState::MovingWithMouse;
                        play_pickup_snd();
                    }
                    else if let Some(touch) = touch {
                        self.moving = SelectorMoveState::MovingWithTouch(touch);
                        play_pickup_snd();
                    }
                    return;
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

#[allow(unused)]
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

macro_rules! lock_unlock {
    ($ctx:expr, $lock_data:expr, $decal_path:expr) => {
        {
            let decal_texture = texture_srgb!($ctx, $decal_path);
            LockUnlockMat {
                locked_mat: Gp::new(PBRMaterial {
                    albedo_texture: $lock_data.0.clone(),
                    metallic_roughness_texture: $lock_data.1.clone(),
                    albedo_decal_texture: decal_texture.clone(),
                    ..PBRMaterial::default($ctx)
                }),
                unlocked_mat: Gp::new(PBRMaterial {
                    albedo_texture: $lock_data.2.clone(),
                    metallic_roughness_texture: $lock_data.3.clone(),
                    albedo_decal_texture: decal_texture,
                    ..PBRMaterial::default($ctx)
                }),
            }
        }
    }
}

impl Assets {
    pub fn new(engine: &mut Engine) -> Self {
        let ctx = engine.render_ctx();

        let metal_031_a = texture_linear!(ctx, "./assets/mat/metal_031/albedo.png");
        let metal_031_m = texture_linear!(ctx, "./assets/mat/metal_031/pbr.png");

        let metal_046_a = texture_linear!(ctx, "./assets/mat/metal_046/albedo.png");
        let metal_046_m = texture_linear!(ctx, "./assets/mat/metal_046/pbr.png");

        let metal_028_a = texture_linear!(ctx, "./assets/mat/metal_028/albedo.png");
        let metal_028_m = texture_linear!(ctx, "./assets/mat/metal_028/pbr.png");

        // brass:
        // texture_linear!(ctx, "./assets/mat/brass_4k/albedo.png"),
        // texture_linear!(ctx, "./assets/mat/brass_4k/pbr.png"),

        let lock_data = (
            // locked texture
            metal_028_a.clone(),
            metal_028_m.clone(),
            // unlocked texture
            metal_031_a.clone(),
            metal_031_m.clone(),
        );

        let [
            wall_tl, wall_t, wall_tr,
            wall_l, wall_r,
            wall_bl, wall_b, wall_br,

            wall_tl_i, wall_tr_i, wall_bl_i, wall_br_i,
        ] = import_mesh_set_as_gc(engine, include_bytes!("./assets/walls.glb"), &[
            "wall-tl",
            "wall-t",
            "wall-tr",
            "wall-l",
            "wall-r",
            "wall-bl",
            "wall-b",
            "wall-br",
            "wall-tl-i", "wall-tr-i", "wall-bl-i", "wall-br-i",
        ]).unwrap();

        let [
            node_ingot,
            node_mix2,
            node_nut,
            node_bolt,
            node_prism,
            node_split,
            node_swap,
            node_collect,
        ] = import_mesh_set_as_gc(engine, include_bytes!("./assets/nodes.glb"), &[
            "ingot",
            "mix2",
            "nut",
            "bolt",
            "prism",
            "split",
            "swap",
            "collect",
        ]).unwrap();

        let [
            select_swap,
            select_o_o,
            select_o_o_o,
            select_v3,
        ] = import_mesh_set_as_gc(engine, include_bytes!("./assets/selectors.glb"), &[
            "select_swap",
            "select_o_o",
            "select_o_o_o",
            "select_v3",
        ]).unwrap();

        let laser_shader = Gp::new(PBRShader::new(ctx, "laser.wgsl", include_str!("./shaders/laser.wgsl")));

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

            pool: InstancePool::new(),
            pool_static: InstancePool::new(),

            rng: Rng::new(),

            metal_sfx: [
                sfx!("./assets/metal_1.wav"),
                sfx!("./assets/metal_2.wav"),
                sfx!("./assets/metal_3.wav"),
                sfx!("./assets/metal_4.wav"),
                sfx!("./assets/metal_5.wav"),
            ],
            metal_pickup: sfx!("./assets/metal_pickup.wav"),
            metal_putdown: sfx!("./assets/metal_putdown.wav"),
            move_err: sfx!("./assets/move_err.wav"),

            win: sfx!("./assets/win.wav"),
            click: sfx!("./assets/click.wav"),

            node_mix: mesh!(ctx, "./assets/mix_node.glb"),
            node_mix_mat: lock_unlock!(ctx, lock_data, "./assets/label_mix.png"),
            node_hook: mesh!(ctx, "./assets/hook_node.glb"),
            node_hook_mat: lock_unlock!(ctx, lock_data, "./assets/hook_label.png"),

            node_ingot,
            node_mix2,
            node_nut,
            node_bolt,
            node_prism,
            node_split,
            node_swap,
            node_collect,

            node_ingot_mat: lock_unlock!(ctx, lock_data, "./assets/ingot_label.png"),
            node_mix2_mat: lock_unlock!(ctx, lock_data, "./assets/mix2_label.png"),
            node_nut_mat: lock_unlock!(ctx, lock_data, "./assets/nut_label.png"),
            node_bolt_mat: lock_unlock!(ctx, lock_data, "./assets/bolt_label.png"),
            node_prism_mat: lock_unlock!(ctx, lock_data, "./assets/hook_label.png"),
            node_split_mat: lock_unlock!(ctx, lock_data, "./assets/split_label.png"),
            node_swap_mat: lock_unlock!(ctx, lock_data, "./assets/swap_label.png"),
            node_collect_mat: lock_unlock!(ctx, lock_data, "./assets/collect_label.png"),

            emitter: mesh!(ctx, "./assets/emitter.glb"),
            emitter_mat: lock_unlock!(ctx, lock_data, "./assets/emitter_label.png"),

            select_vert_1: mesh!(ctx, "./assets/select_vert_1.glb"),
            select_vert_2: mesh!(ctx, "./assets/select_vert_2.glb"),

            select_swap,
            select_o_o,
            select_o_o_o,
            select_v3,

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

            goal: mesh!(ctx, "./assets/goal_node.glb"),
            goal_mat: lock_unlock!(ctx, lock_data, "./assets/goal_label.png"),

            goal_light: mesh!(ctx, "./assets/goal_node_light.glb"),
            goal_light_mat: Gp::new(PBRMaterial {
                shader: laser_shader.clone(),
                ..PBRMaterial::default(ctx)
            }),

            wall_tl,
            wall_t,
            wall_tr,
            wall_l,
            wall_r,
            wall_bl,
            wall_b,
            wall_br,
            wall_tl_i,
            wall_tr_i,
            wall_bl_i,
            wall_br_i,
            wall_mat: Gp::new(PBRMaterial {
                albedo_texture: metal_046_a.clone(),
                metallic_roughness_texture: metal_046_m.clone(),
                ..PBRMaterial::default(ctx)
            }),

            laser: mesh!(ctx, "./assets/laser.glb"),
            laser_mat: Gp::new(PBRMaterial {
                shader: laser_shader.clone(),
                ..PBRMaterial::default(ctx)
            }),
        }
    }

    // fn node_mix(&self, ctx: &RenderCtx, transform: cgmath::Matrix4<f32>) -> Gp<MeshInstance> {
    //     Gp::new(MeshInstance::new(ctx,
    //         self.node_mix.clone(),
    //         self.node_mix_mat.clone(),
    //         transform))
    // }

    fn the_pow(v: Vector3<f32>, pow: f32) -> Vector3<f32> {
        vec3(f32::powf(v.x, pow), f32::powf(v.y, pow), f32::powf(v.z, pow))
    }

    fn laser(&self, engine: &Engine, transform: cgmath::Matrix4<f32>, color: Vector3<f32>) -> Gp<MeshInstance> {
        let color = Self::the_pow(color, 2.2);

        self.pool.get_at2(engine,
            &self.laser,
            &self.laser_mat,
            transform, color.extend(1.0))
    }

    fn goal_light(&self, engine: &Engine, transform: cgmath::Matrix4<f32>, color: Vector3<f32>) -> Gp<MeshInstance> {
        let color = Self::the_pow(color, 2.2);

        self.pool.get_at2(engine,
            &self.goal_light,
            &self.goal_light_mat,
            transform, color.extend(1.0))
    }
}

enum GameplayState {
    Level,
    LevelSelect,
    MainMenu,
}

pub struct GameplayLogic {
    theta: f32,

    assets: Assets,
    level: Level,

    cur_level_idx: usize,

    selector: Selector,

    has_won: bool,

    state: GameplayState,

    the_horse: Gp<MeshInstance>,
}

// meow

static LEVELS: [&str; 7] = [
    "intro",
    "intro_mix_simpler",
    "intro_mix",
    "locked_mixers",
    "another_swaps",
    "crazy_swaps",
    "intro_mix_constrained",
];

impl GameplayLogic {
    #[inline_tweak::tweak_fn]
    pub fn tweak_scene(&mut self, engine: &mut Engine) {
        engine.main_world.lights[0].color.set(vec3(5.0, 5.0, 5.0));
    }

    fn open_level(&mut self, engine: &mut Engine, idx: usize) {
        self.assets.pool_static.recycle();
        if let Some(name) = LEVELS.get(idx) {
            self.level = Level::new_from_map(&format!("./levels/{}.tmx", name), engine, &self.assets); 
            self.state = GameplayState::Level;
            self.cur_level_idx = idx;

            // Reset selector
            self.selector.touch_started = None;
            self.selector.moving = SelectorMoveState::NotMoving;
            self.selector.state = SelectorState::None;
        }
        else {
            self.state = GameplayState::LevelSelect;
        }
    }

    fn click(&mut self, engine: &mut Engine) {
        engine.audio.play_speed(&self.assets.click, self.assets.rng.range(0.95..1.05));
    }
}


impl engine::Gameplay for GameplayLogic {
    const GAME_TITLE: &'static str = "ben's beams";
    const DEFAULT_TONEMAP: engine::video::hdr_tonemap::Tonemap = Tonemap::None;

    fn new(engine: &mut Engine) -> Self {
       let assets = Assets::new(engine);
       let ctx = engine.render_ctx();

       let the_horse = Gp::new(MeshInstance::new(ctx,
            assets.horse_mesh.clone(),
            assets.horse_material.clone(),
            Matrix4::identity()));

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

        let level = Level::new_from_map("./levels/hook_something.tmx", engine, &assets);
       // for i in 0..5 {
        //level.try_place(2, 2, DeviceTy::Mix);
        
        //}//

        let selector = Selector::new(ctx, &assets);

        level.setup_camera(engine);

        engine.audio.play_music(include_bytes!("./assets/music.ogg"), 1.6);

        GameplayLogic {
            assets,
            theta: 0.0,
            level,
            cur_level_idx: 0,
            selector,
            has_won: false,
            state: GameplayState::MainMenu,

            the_horse
        }
    }



    fn tick(&mut self, engine: &mut Engine) {
        self.theta += 0.03;
        self.tweak_scene(engine);

        self.level.build_lasers();
        self.level.setup_camera(engine);

        self.selector.update(engine, &self.assets, &mut self.level);

        // We won if we fulfilled all the goals and are not moving anything.
        let had_won = self.has_won;
        self.has_won = (self.level.nr_goals_fulfilled >= self.level.nr_goals)
            && matches!(self.selector.moving, SelectorMoveState::NotMoving);
        log::info!("has won? {}", self.has_won);

        if self.has_won && !had_won {
            engine.audio.play(&self.assets.win);
        }

        engine.main_world.clear_meshes();
        self.assets.pool.recycle();
        match self.state {
            GameplayState::Level => {
                self.level.build_meshes(engine, &self.assets);
                self.selector.push_mesh(engine);
            },
            _ => {
                engine.main_camera.position.set(point3(0.0, 2.0, -2.0));
                engine.main_camera.target.set(point3(0.0, 0.0, 0.0));
                engine.main_camera.projection.set(CameraProjection::Perspective { fovy: 45.0, znear: 0.01, zfar: 20.0 });
                //aengine.main_camera
                self.the_horse.transform.set(Matrix4::from_angle_y(cgmath::Rad(self.theta)));
                self.the_horse.update(engine.render_ctx());

                engine.main_world.push_mesh(self.the_horse.clone());
            }
        }

        //let offset = vec3(0.3 * f32::cos(self.theta), 0.0, 0.3 * f32::sin(self.theta));

        //engine.main_camera.position.set(point3(0.0, 15.0, 3.0) + offset);
        //engine.main_camera.target.set(point3(0.0, 0.0, 0.0) + offset);
        //game.main_camera.position.set(point3(15.0 * f32::cos(self.theta), 15.0 * f32::sin(self.theta), 0.0));
    }

    fn event(&mut self, engine: &mut Engine, event: &WindowEvent) {
        match event {
            WindowEvent::Touch(touch) => {
                // If we are already moving with touch, handle those updates.
                if let SelectorMoveState::MovingWithTouch(id) = self.selector.moving {
                    if touch.id == id {
                        self.selector.touch_pos = vec2(touch.location.x as f32, touch.location.y as f32);
                        if touch.phase == TouchPhase::Cancelled || touch.phase == TouchPhase::Ended {
                            // Finish the touch right now.
                            self.selector.do_move(engine, &self.assets, &mut self.level, true);
                        }
                    }
                }
                else if let SelectorMoveState::NotMoving = self.selector.moving {
                    if touch.phase == TouchPhase::Started {
                        // Let the selector start moving from this touch.
                        self.selector.touch_started = Some(touch.id);
                        self.selector.touch_pos = vec2(touch.location.x as f32, touch.location.y as f32);
                    }
                }
            },
            _ => {}
        }
    }

    fn ui(&mut self, engine: &mut Engine, ctx: &egui::Context) {
        // We have to set this on the engine's Window object
        // ctx.set_zoom_factor(4.0);

        let mut desired_scale = 3.0;

        match self.state {
            GameplayState::Level => {
                if self.has_won {
                    egui::Area::new(egui::Id::new("win_menu"))
                        .anchor(Align2::CENTER_CENTER, (0.0, 0.0))
                        .show(ctx, |ui| {
                            egui::Frame::default()
                                .fill(ui.visuals().panel_fill)
                                .corner_radius(5.0)
                                .inner_margin(5.0)
                                .show(ui, |ui| {
                                    ui.vertical_centered(|ui| {
                                        ui.heading("you win!");
                                        if ui.button("next level").clicked() {
                                            self.open_level(engine, self.cur_level_idx + 1);
                                            self.click(engine);
                                        }
                                        if ui.button("quit").clicked() {
                                            self.state = GameplayState::LevelSelect;
                                            self.click(engine);
                                        }
                                    });
                                });
                            
                            //.heading("Test UI");

                            //ui.button("Press This Button!");
                        });
                }
                else {
                     egui::Area::new(egui::Id::new("lvl_menu"))
                    .fixed_pos((4.0, 4.0))
                    .show(ctx, |ui| {
                        ui.heading(format!("level {}", (self.cur_level_idx + 1)));
                        if ui.button("quit").clicked() {
                            self.state = GameplayState::LevelSelect;
                            self.click(engine);
                        }
                    });
                }
            }

            GameplayState::LevelSelect => {
                 egui::Area::new(egui::Id::new("level_select"))
                    .anchor(Align2::CENTER_CENTER, (0.0, 0.0))
                    .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.heading("choose a level...");
                        let mut nr = 1;
                        for level in LEVELS {
                            log::info!("level select: {} {}", nr, level);
                            let label = format!("level {}", nr);
                            if ui.button(label).clicked() {
                                self.open_level(engine, nr - 1);
                                self.click(engine);
                            }

                            nr += 1;
                        }
                    });
                });
            }

            GameplayState::MainMenu => {
                desired_scale = 5.0;
                egui::Area::new(egui::Id::new("main_menu"))
                    .anchor(Align2::CENTER_CENTER, (0.0, 0.0))
                    .show(ctx, |ui| {
                     ui.vertical_centered(|ui| {
                        ui.heading("ben's beams");
                        if ui.button("play!").clicked() {
                            self.state = GameplayState::LevelSelect;
                        }
                    });
                });
            }
        }
        
        engine.get_main_window_mut().egui_scale_factor = desired_scale;
        ctx.set_zoom_factor(desired_scale as f32);
    }
}

game!(GameplayLogic);
