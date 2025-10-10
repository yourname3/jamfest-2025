use ponygame::game;
// /
use ponygame::cgmath::{point3, vec3, Matrix4, SquareMatrix};
use ponygame::cgmath;
use ponygame::log;

use ponygame::video::RenderCtx;
use ponygame::{audio::Sound, gc::{Gp, GpMaybe}, video::{asset_import::import_binary_data, mesh_render_pipeline::{Mesh, MeshInstance}, texture::Texture, PBRMaterial}, PonyGame};

struct Assets {
    horse_mesh: Gp<Mesh>,
    horse_material: Gp<PBRMaterial>,

    node_mix: Gp<Mesh>,
    
    node_mix_mat: Gp<PBRMaterial>,

    sfx0: Sound,
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
                albedo_texture: texture_srgb!(ctx, "./assets/mat/metal_046/albedo.png"),
                metallic_roughness_texture: texture_linear!(ctx, "./assets/mat/metal_046/pbr.png"),
                albedo_decal_texture: texture_srgb!(ctx, "./assets/label_mix.png"),
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
}


pub struct GameplayLogic {
    theta: f32,

    assets: Assets,
}

// meow

impl ponygame::Gameplay for GameplayLogic {
    const GAME_TITLE: &'static str = "JamFest";

    fn new(engine: &mut PonyGame) -> Self {
       let assets = Assets::new(engine);
       let ctx = engine.render_ctx();

        // let transform0 = cgmath::Matrix4::from_translation(vec3(-0.5, 0.0, 0.0));
        // let transform1 = cgmath::Matrix4::from_translation(vec3( 0.5, 0.0, 0.0));

        engine.main_world.set_envmap(&Gp::new(Texture::from_bytes_rgba16unorm(ctx,
            include_bytes!("./assets/horn-koppe_spring_1k.exr"),
            Some("horn-koppe_spring_1k.exr"),
            true).unwrap()));

        for i in 0..5 {
            engine.main_world.push_mesh(assets.node_mix(ctx,
                Matrix4::from_translation(vec3(0.0, 0.0, i as f32 * 2.0)
            )));
        }

        engine.main_camera.position.set(point3(0.0, 15.0, 3.0));
        engine.main_camera.target.set(point3(0.0, 0.0, 0.0));

        GameplayLogic {
            assets,
            theta: 0.0,
        }
    }

    fn tick(&mut self, engine: &mut PonyGame) {
        let ctx = engine.render_ctx();

        self.theta += 0.1;

        //let offset = vec3(0.3 * f32::cos(self.theta), 0.0, 0.3 * f32::sin(self.theta));

        //engine.main_camera.position.set(point3(0.0, 15.0, 3.0) + offset);
        //engine.main_camera.target.set(point3(0.0, 0.0, 0.0) + offset);
        //game.main_camera.position.set(point3(15.0 * f32::cos(self.theta), 15.0 * f32::sin(self.theta), 0.0));
    }
}

game!(GameplayLogic);
