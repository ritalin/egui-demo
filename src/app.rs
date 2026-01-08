use std::sync::Arc;

use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{self, ElementState, WindowEvent}, event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey}, window::Window};

pub struct App {
    main_window: Option<Arc<Window>>,
}
impl App {
    const DEFAULT_WIDTH: u32 = 1360;
    const DEFAULT_HEIGHT: u32 = 768;

    pub fn new() -> Self {
        Self {
            main_window: None,
        }
    }

    async fn handle_prepare_window_frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), anyhow::Error> {
        let w = Arc::new(event_loop.create_window(Window::default_attributes())?);
        let _ = w.request_inner_size(PhysicalSize::new(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT));
        self.main_window.get_or_insert_with(|| w);
        Ok(())
    }

    fn handle_close_requested(&self, event_loop: &ActiveEventLoop) {
        println!("Terminating App...");
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        pollster::block_on(self.handle_prepare_window_frame(event_loop))
            .map_err(|err| anyhow::anyhow!("faild to create a main window (reason: {err}"))
            .unwrap()
        ;
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent)
    {
        match event {
            WindowEvent::CloseRequested => {
                if self.main_window.is_none() {
                    self.handle_close_requested(event_loop);
                }
            }
            WindowEvent::KeyboardInput { event: event::KeyEvent{ physical_key: PhysicalKey::Code(code), state: key_state, .. }, .. } => {
                match (code, key_state) {
                    (KeyCode::Escape, ElementState::Pressed) => {
                        self.handle_close_requested(event_loop);
                    }
                    _ => {}
                }
            }
            _ => {

            }
        }
    }
}
