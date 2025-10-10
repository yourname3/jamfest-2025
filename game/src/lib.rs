use ponygame::game;
// /
use ponygame::cgmath::{vec3, SquareMatrix};
use ponygame::cgmath;
use ponygame::log;

use ponygame::{audio::Sound, gc::{Gp, GpMaybe}, video::{asset_import::import_binary_data, mesh_render_pipeline::{Mesh, MeshInstance}, texture::Texture, PBRMaterial}, PonyGame};

pub struct GameplayLogic {
    instance0: Gp<MeshInstance>,
    instance1: Gp<MeshInstance>,

    sfx0: Sound,

    theta: f32,

    theta0: f32,
    theta1: f32,
}

// meow

impl ponygame::Gameplay for GameplayLogic {
    const GAME_TITLE: &'static str = "JamFest";

    fn new(engine: &mut PonyGame) -> Self {
        let ctx = engine.render_ctx();
        let sfx0 = Sound::from_data(include_bytes!("../test/test_sfx.wav"));

        let test_mesh = import_binary_data(include_bytes!("../test/horse.glb")).unwrap();
        let test_albedo = Texture::from_bytes_rgba8srgb(ctx, include_bytes!("../test/horse_albedo.png"), Some("horse albedo"), false)
                .unwrap();
        let test_met_rough = Texture::dummy(ctx, Some("dummy"));

        let material = PBRMaterial {
            albedo: vec3(1.0, 1.0, 1.0),
            metallic: 0.03,
            roughness: 0.95,
            reflectance: 0.0,
            albedo_texture: test_albedo,
            metallic_roughness_texture: test_met_rough,

            cached_bind_group: GpMaybe::none(),
        };
        let material = Gp::new(material);

        let transform0 = cgmath::Matrix4::from_translation(vec3(-0.5, 0.0, 0.0));
        let transform1 = cgmath::Matrix4::from_translation(vec3( 0.5, 0.0, 0.0));

        let mesh = Mesh::new(ctx, &test_mesh);
        let mesh = Gp::new(mesh);

        let instance0 = Gp::new(MeshInstance::new(ctx, mesh.clone(), material.clone(), transform0));
        let instance1 = Gp::new(MeshInstance::new(ctx, mesh.clone(), material.clone(), transform1));

        engine.main_world.push_mesh(instance0.clone());
        engine.main_world.push_mesh(instance1.clone());
        engine.main_world.set_envmap(&Gp::new(Texture::from_bytes_rgba16unorm(ctx,
            include_bytes!("../test/horn-koppe_spring_1k.exr"),
            Some("horn-koppe_spring_1k.exr"),
            true).unwrap()));

        GameplayLogic { theta: 0.0, instance0, instance1, theta0: 0.0, theta1: 0.0, sfx0 }
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
            game.audio.play(&self.sfx0);
            self.theta -= 6.28;
        }
    }
}

game!(GameplayLogic);
