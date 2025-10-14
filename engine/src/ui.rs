use egui_wgpu::RendererOptions;

use crate::video::Video;



pub struct Egui {
    pub egui_ctx: egui::Context,
    pub egui_state: egui_winit::State,
    pub egui_renderer: egui_wgpu::Renderer,
}

impl Egui {
    pub fn new(video: &Video) -> Self {
        let window = video.id_map.iter().next().unwrap().1;

        let egui_ctx = egui::Context::default();
        let egui_state = egui_winit::State::new(egui_ctx.clone(), egui::ViewportId::default(),
            &window.sdl, None, None, None);
        let egui_renderer = egui_wgpu::Renderer::new(
            &video.renderer.ctx.device,
            window.renderer.config.format,
            RendererOptions::PREDICTABLE
        );

        Self {
            egui_ctx,
            egui_state,
            egui_renderer,
        }
    }
}