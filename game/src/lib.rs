use std::f32::consts::PI;

mod level;

use grid::Grid;
use inline_tweak::tweak;
use ponygame::video::asset_import::import_mesh_set_as_gc;
use ponygame::{game, gc};
// /
use ponygame::cgmath::{point3, vec3, vec4, Matrix4, SquareMatrix, Vector3, Zero};
use ponygame::cgmath;
use ponygame::log;

use ponygame::video::camera::CameraProjection;
use ponygame::video::{PBRShader, RenderCtx};
use ponygame::{audio::Sound, gc::{Gp, GpMaybe}, video::{asset_import::import_binary_data, mesh_render_pipeline::{Mesh, MeshInstance}, texture::Texture, PBRMaterial}, PonyGame};

use level::*;

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

    sfx0: Sound,
}

enum SelectorState {
    None,
    Vert2,
}

struct Selector {
    mesh_vert_2: Gp<MeshInstance>,

    state: SelectorState,
    object: GpMaybe<Device>,
    x: i32,
    y: i32,

    start_x: i32,
    start_y: i32,

    is_moving: bool,
}

impl Selector {
    pub fn new(ctx: &RenderCtx, assets: &Assets) -> Self {
        Selector {
            mesh_vert_2: Gp::new(MeshInstance::new(ctx,
                assets.select_vert_2.clone(),
                assets.select_mat.clone(),
                Matrix4::identity())),
            state: SelectorState::None,
            object: GpMaybe::none(),
            x: 0,
            y: 0,

            start_x: 0,
            start_y: 0,

            is_moving: false,
        }
    }

    fn get_current_mesh(&self) -> Option<&Gp<MeshInstance>> {
        match self.state {
            SelectorState::None => None,
            SelectorState::Vert2 => Some(&self.mesh_vert_2),
        }
    }

    pub fn push_mesh(&mut self, engine: &mut PonyGame) {
        let transform = Matrix4::from_translation(vec3(self.x as f32, 0.0, self.y as f32));
        let mesh_instance = self.get_current_mesh();

        if let Some(mesh_instance) = mesh_instance {
            mesh_instance.transform.set(transform);
            mesh_instance.update(engine.render_ctx());

            engine.main_world.push_mesh(mesh_instance.clone());
        }
    }

    pub fn do_move(&mut self, engine: &mut PonyGame, level: &mut Level) {
        if matches!(self.state, SelectorState::None) { return; }

        let Some(dev) = self.object.get() else { return; };

        let viewport = engine.get_viewport();

        let pos = engine.get_cursor_position();
        let pos = engine.main_camera.convert_screen_to_normalized_device(viewport, pos);
        
        let intersect = engine.main_camera.intersect_ray_with_plane_from_ndc(pos, viewport, (
            Vector3::zero(), Vector3::unit_y()
        )).unwrap();

        self.x = intersect.x as i32;
        self.y = intersect.z as i32;

        let valid = level.move_from(self.start_x, self.start_y, &dev, self.x, self.y);
        if let Some(mesh) = self.get_current_mesh() {
            mesh.modulate.set(if valid { vec4(1.0, 1.0, 1.0, 1.0) } else { vec4(1.0, 0.0, 0.0, 1.0) });
            mesh.update(engine.render_ctx());
        }

        if !engine.get_main_window().left_mouse_down {
            self.is_moving = false;
            level.finish_move_from(self.start_x, self.start_y, &dev, self.x, self.y);
        }
    }

    pub fn update(&mut self, engine: &mut PonyGame, level: &mut Level) {
        if self.is_moving {
            self.do_move(engine, level);
            return;
        }

        let pos = engine.get_cursor_position();
        let pos = engine.main_camera.convert_screen_to_normalized_device(engine.get_viewport(), pos);
        let vp = engine.main_camera.get_view_projection_matrix(engine.get_viewport());

        //log::info!("cursor pos @ {:?}", pos);

        //let Some(invert) = vp.invert() else { return; };

        self.state = SelectorState::None;
        self.object.set(None);

        for x in 0..level.grid.cols() {
            for y in 0..level.grid.rows() {
                let cell = level.grid.get(y, x).unwrap();
                if let GridCell::DeviceRoot(dev) = cell {
                    let bounds = dev.ty.get_bounds();
                    let low_point = vec3(x as f32, 0.0, y as f32);
                    let high_point = vec3(low_point.x + bounds.0 as f32, 0.0, low_point.z + bounds.1 as f32);

                    let low_point = (vp * low_point.extend(1.0)).truncate().truncate();
                    let high_point = (vp * high_point.extend(1.0)).truncate().truncate();

                    //log::info!("candidate object @ {:?} -> {:?}", low_point, high_point);

                    if pos.x < low_point.x || pos.x > high_point.x { continue; }
                    if pos.y < high_point.y || pos.y > low_point.y { continue; }

                    // Cursor should be overlapping..
                    self.state = dev.ty.get_selector();
                    // If the object was not selectable, keep looking.
                    if matches!(self.state, SelectorState::None) { continue; }

                    self.object.set(Some(dev));
                    self.x = x as i32;
                    self.y = y as i32;
                    self.start_x = self.x;
                    self.start_y = self.y;

                    if engine.get_main_window().left_mouse_down {
                        self.is_moving = true;
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

    fn laser(&self, ctx: &RenderCtx, transform: cgmath::Matrix4<f32>, color: Vector3<f32>) -> Gp<MeshInstance> {
        Gp::new(MeshInstance::new_modulate(ctx,
            self.laser.clone(),
            self.laser_mat.clone(),
            transform, color.extend(1.0)))
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

        let mut level = Level::new_from_map("./levels/test.tmx", engine, &assets);
       // for i in 0..5 {
        level.try_place(2, 2, DeviceTy::Mix);
        
        //}//

        let selector = Selector::new(ctx, &assets);

        level.setup_camera(engine);

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
        self.level.setup_camera(engine);

        self.selector.update(engine, &mut self.level);

        engine.main_world.clear_meshes();
        self.level.build_meshes(engine, &self.assets);
        self.selector.push_mesh(engine);

        //let offset = vec3(0.3 * f32::cos(self.theta), 0.0, 0.3 * f32::sin(self.theta));

        //engine.main_camera.position.set(point3(0.0, 15.0, 3.0) + offset);
        //engine.main_camera.target.set(point3(0.0, 0.0, 0.0) + offset);
        //game.main_camera.position.set(point3(15.0 * f32::cos(self.theta), 15.0 * f32::sin(self.theta), 0.0));
    }

    fn ui(&mut self, ctx: &egui::Context) {
        egui::Area::new(egui::Id::new("test"))
            .fixed_pos((20.0, 20.0))
            .show(ctx, |ui| {
                ui.set_max_width(300.0);
                ui.heading("Test UI");

                ui.button("Press This Button!");
            });
    }
}

game!(GameplayLogic);
