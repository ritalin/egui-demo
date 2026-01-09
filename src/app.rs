use std::sync::Arc;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{self, ElementState, WindowEvent}, event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey}, window::Window};

use crate::render;

struct AppState {}

impl AppState {
    fn update(&self) {}
}

pub struct App {
    main_window: Option<Arc<Window>>,
    raw_handle: Option<render::RawWindow>,
    renderer: Option<render::WgpuRenderer>,
    state: AppState,
}
impl App {
    const DEFAULT_WIDTH: u32 = 1360;
    const DEFAULT_HEIGHT: u32 = 768;

    pub fn new() -> Self {
        Self {
            main_window: None,
            raw_handle: None,
            renderer: None,
            state: AppState {  },
        }
    }

    async fn handle_prepare_window_frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), anyhow::Error> {
        let w = Arc::new(event_loop.create_window(Window::default_attributes())?);
        let _ = w.request_inner_size(PhysicalSize::new(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT));
        let raw_handle = render::RawWindow::create(&w)?;

        self.renderer = Some(render::WgpuRenderer::create(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT, &raw_handle).await?);
        self.main_window.get_or_insert_with(|| w);
        self.raw_handle = Some(raw_handle);
        Ok(())
    }

    fn handle_close_requested(&self, event_loop: &ActiveEventLoop) {
        log::info!("Terminating App...");
        event_loop.exit();
    }

    fn handle_resize(&mut self, _event_loop: &ActiveEventLoop, size: PhysicalSize<u32>) {
        log::info!("Resize requested: width: {width}, height: {height}", width = size.width, height = size.height);
        if let Some(renderer) = self.renderer.as_mut() && (size.width > 0) && (size.height > 0) {
            renderer.request_resize(size.width, size.height);
        }
    }

    fn handle_redraw(&mut self, _event_loop: &ActiveEventLoop) {
        if let (Some(w), Some(r)) = (self.main_window.as_ref(), self.renderer.as_mut()) {
            self.state.update();
            w.request_redraw(); // Reserve the next redrawing
            match r.render() {
                Ok(_) => {},
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    let size = w.inner_size();
                    r.request_resize(size.width, size.height);
                }
                Err(e) => log::error!("Unable to render (reason: {e}"),
            }
        }
        // println!("redraw requested");
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
            WindowEvent::Resized(size) => {
                self.handle_resize(event_loop, size);
            }
            WindowEvent::RedrawRequested => {
                self.handle_redraw(event_loop);
            }
            _ => {

            }
        }
    }
}
