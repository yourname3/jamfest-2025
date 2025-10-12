use cgmath::Vector2;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy}};

use crate::{gc::Gp, video::{camera::Camera, hdr_tonemap::Tonemap, world::{Viewport, World}, RenderCtx, Video, Window}};

pub mod audio;
pub mod error;
pub mod video;
pub mod gc;
pub mod gc_types;
pub mod ui;

/// Our custom user event for winit. Used in part for asynchronously initializing
/// the app in browser.
pub enum PonyGameAppEvent {
    Initialize(PonyGame)
}

pub struct PonyGame {
    pub video: crate::Video,
    pub audio: crate::audio::Audio,

    // TODO: We really need to be able to access the Window, Viewport, etc...
    pub main_world: Gp<World>,
    pub main_camera: Gp<Camera>,

    pub egui: ui::Egui,

    last_tick: web_time::Instant,
    accumulator: web_time::Duration,
}

pub trait Gameplay {
    const GAME_TITLE: &'static str;
    const DEFAULT_TONEMAP: Tonemap;
    fn new(engine: &mut PonyGame) -> Self;
    fn tick(&mut self, engine: &mut PonyGame);
    fn ui(&mut self, engine: &mut PonyGame, ctx: &egui::Context);
}

struct PonyGameApp<G: Gameplay> {
    inner: Option<(PonyGame, G)>,

    // Keeps track of whether we've fired Video::new() yet or not.
    init_proxy: Option<EventLoopProxy<PonyGameAppEvent>>,
}

impl PonyGame {
    pub fn get_cursor_position(&self) -> Vector2<f32> {
        // TODO: Don't use unwrap...
        self.video.id_map.iter().next().unwrap().1.cursor_position
    }

    pub fn get_main_window(&self) -> &Window {
        self.video.id_map.iter().next().unwrap().1
    }

    pub fn get_viewport(&self) -> &Viewport {
        &self.video.id_map.iter().next().unwrap().1.renderer.viewport
    }

    pub fn render_ctx(&self) -> &RenderCtx {
        &self.video.renderer.ctx
    }

    pub fn maybe_tick<G: Gameplay>(&mut self, gameplay: &mut G) {
        let now = web_time::Instant::now();

        let elapsed = now - self.last_tick;
        let mut total = elapsed + self.accumulator;

        let step_size = web_time::Duration::from_nanos(1_000_000_000 / 60);

        // If we are really far behind, just tick once (?) and then update the
        // instant to now.  The idea being that this happens during loading and such.
        if total >= step_size * 16 {
            gameplay.tick(self);
            self.accumulator = web_time::Duration::from_micros(0);
            self.last_tick = now;
            return;
        }

        if total >= step_size {
            let mut max_loops = 4;
            while total >= step_size {
                total -= step_size;
                gameplay.tick(self);

                max_loops -= 1;
                if max_loops <= 0 { break; }
            }
            self.accumulator = total;
            self.last_tick = now;
        }
    }
}

impl<G: Gameplay> ApplicationHandler<PonyGameAppEvent> for PonyGameApp<G> {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if let Some(init_proxy) = self.init_proxy.take() {
            Video::new::<G>(event_loop, init_proxy, G::GAME_TITLE)
        }
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let Some((engine, gameplay)) = self.inner.as_mut() else { return; };

         #[cfg(target_arch = "wasm32")]
        match event {
            winit::event::WindowEvent::MouseInput { .. } | winit::event::WindowEvent::Touch(_) => {
                engine.audio.resume_on_gesture();
            },
            _ => {}
        }

        let should_exit = Video::handle_win_event::<G>(engine, gameplay, window_id, event);
        if should_exit {
            event_loop.exit();
        }

       
        
        // match event {
        //     WindowEvent::Resized(_) => {
        //         engine.video.handle_win_event(window_id, event);
        //     },
        //     _ => {}
        // }

        engine.maybe_tick(gameplay);

        //engine.video.render();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: PonyGameAppEvent) {
        match event {
            PonyGameAppEvent::Initialize(mut engine) => {
                engine.video.update_all_window_sizes();

                let gameplay = G::new(&mut engine);

                // Remove the loading screen once the initial loading is done.
                Video::wasm_remove_loading_screen();
                
                self.inner = Some((engine, gameplay));
            },
        }
    }
}

pub fn run_game_impl<G: Gameplay>() {
    let event_loop = EventLoop::with_user_event()
        .build().unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();

    // Apparently this can cause a RefCell double-borrow on WASM/Web, although
    // I'm not sure how often that actually happens...
    event_loop.set_control_flow(ControlFlow::Wait);

    let mut app = PonyGameApp::<G> {
        inner: None,
        // Create an initial proxy used for sending the initialized engine back
        // to us, particularly relevant on WASM.
        init_proxy: Some(event_loop.create_proxy()),
    };

    let _ = event_loop.run_app(&mut app);
}

#[cfg(target_arch = "wasm32")]
pub fn run_game_on_web_impl<G: Gameplay>() -> Result<(), wasm_bindgen::JsValue> {
    console_error_panic_hook::set_once();

    console_log::init().unwrap_throw();

    run_game_impl::<G>();

    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen;

pub use cgmath;
pub use log;

#[macro_export]
macro_rules! game {
    ($gameplay:ty) => {
        pub fn run() {
            ponygame::run_game_impl::<$gameplay>();
        }

        #[cfg(target_arch = "wasm32")]
        #[ponygame::wasm_bindgen::prelude::wasm_bindgen(start)]
        pub fn run_web() {
            ponygame::run_game_on_web_impl::<$gameplay>();
        }
    }
}

#[macro_export]
macro_rules! game_main {
    () => {
        pub fn main() {
            crate::run();
        }
    }
}