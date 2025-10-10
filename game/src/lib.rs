use ponygame::game;
// /
use ponygame::cgmath::{vec3, SquareMatrix};
use ponygame::cgmath;
use ponygame::log;

use ponygame::{audio::Sound, gc::{Gp, GpMaybe}, video::{asset_import::import_binary_data, mesh_render_pipeline::{Mesh, MeshInstance}, texture::Texture, PBRMaterial}, PonyGame};

struct Assets {
    horse_mesh: Gp<Mesh>,
    horse_material: Gp<PBRMaterial>,

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
                metallic_roughness_texture: texture_dummy!(ctx),

                cached_bind_group: GpMaybe::none(),
            }),
            sfx0: sfx!("../test/test_sfx.wav")
        }
    }
}

pub struct GameplayLogic {
    instance0: Gp<MeshInstance>,
    instance1: Gp<MeshInstance>,

    assets: Assets,

    theta: f32,

    theta0: f32,
    theta1: f32,
}

// meow

impl ponygame::Gameplay for GameplayLogic {
    const GAME_TITLE: &'static str = "JamFest";

    fn new(engine: &mut PonyGame) -> Self {
       let assets = Assets::new(engine);
       let ctx = engine.render_ctx();

        let transform0 = cgmath::Matrix4::from_translation(vec3(-0.5, 0.0, 0.0));
        let transform1 = cgmath::Matrix4::from_translation(vec3( 0.5, 0.0, 0.0));

        let instance0 = Gp::new(MeshInstance::new(ctx, assets.horse_mesh.clone(), assets.horse_material.clone(), transform0));
        let instance1 = Gp::new(MeshInstance::new(ctx, assets.horse_mesh.clone(), assets.horse_material.clone(), transform1));

        engine.main_world.push_mesh(instance0.clone());
        engine.main_world.push_mesh(instance1.clone());
        engine.main_world.set_envmap(&Gp::new(Texture::from_bytes_rgba16unorm(ctx,
            include_bytes!("../test/horn-koppe_spring_1k.exr"),
            Some("horn-koppe_spring_1k.exr"),
            true).unwrap()));

        GameplayLogic { theta: 0.0, instance0, instance1, theta0: 0.0, theta1: 0.0, assets }
    }

    fn tick(&mut self, game: &mut PonyGame) {
        let ctx = game.render_ctx();

        let transform0 = cgmath::Matrix4::from_translation(vec3(-0.5, 0.0, 0.0))
            * cgmath::Matrix4::from_angle_x(cgmath::Rad(self.theta0));
        let transform1 = cgmath::Matrix4::from_translation(vec3( 0.5, 0.0, 0.0))
            * cgmath::Matrix4::from_angle_x(cgmath::Rad(self.theta1));

        self.instance0.transform.set(transform0);
        self.instance1.transform.set(transform1);

        self.instance0.update(ctx);
        self.instance1.update(ctx);

        self.theta0 += 0.01;
        self.theta1 -= 0.01;

        game.main_camera.position.set((f32::cos(self.theta) * 2.0, 2.0, f32::sin(self.theta) * 2.0).into());
        self.theta += 0.1;

        if self.theta > 0.3 {
            log::info!("play the sound...");
            game.audio.play(&self.assets.sfx0);
            self.theta -= 6.28;
        }
    }
}

game!(GameplayLogic);
