use std::sync::Arc;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{self, ElementState, WindowEvent}, event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey}, window::Window};

use crate::render;

struct AppState {
    cx: egui::Context,
    input: egui::RawInput,
}
impl AppState {
    fn new() -> Self {
        let cx = egui::Context::default();

        Self {
            cx,
            input: egui::RawInput {
                viewport_id: egui::viewport::ViewportId::ROOT,
                focused: false,
                ..Default::default()
            }
        }
    }

    fn update(&mut self, scale_factor: f64) -> egui::FullOutput {
        let mut input = self.input.clone();
        let viewport = input.viewports.entry(input.viewport_id).or_default();
        viewport.native_pixels_per_point = Some(scale_factor as f32);

        self.cx.run(input, |cx| {
            egui::Area::new(egui::Id::new("winit + egui + wgpu says hello!"))
                .show(cx, |ui| {
                    ui.label("Label!");
                })
            ;
        })
    }
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
            state: AppState::new(),
        }
    }

    async fn handle_prepare_window_frame(&mut self, event_loop: &ActiveEventLoop) -> Result<(), anyhow::Error> {
        let w = Arc::new(event_loop.create_window(Window::default_attributes())?);
        let _ = w.request_inner_size(PhysicalSize::new(Self::DEFAULT_WIDTH, Self::DEFAULT_HEIGHT));
        let raw_handle = render::RawWindow::create(&w)?;

        let screen = render::ScreenDescriptor {
            pixel_per_point: w.scale_factor() as f32,
            screen_width: Self::DEFAULT_WIDTH,
            screen_height: Self::DEFAULT_HEIGHT,
        };

        let mut renderer = render::WgpuRenderer::create(screen.screen_width, screen.screen_height, &raw_handle).await?;
        renderer.request_resize(screen);

        self.renderer = Some(renderer);

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
        if let (Some(w), Some(renderer)) = (self.main_window.as_ref(), self.renderer.as_mut()) && (size.width > 0) && (size.height > 0) {
            let screen = render::ScreenDescriptor {
                pixel_per_point: w.scale_factor() as f32,
                screen_width: size.width,
                screen_height: size.height,
            };
            renderer.request_resize(screen);
        }
    }

    fn handle_redraw(&mut self, _event_loop: &ActiveEventLoop) {
        if let (Some(w), Some(r)) = (self.main_window.as_ref(), self.renderer.as_mut()) {
            if let Some(y) = w.is_minimized() && y {
                log::info!("Skip to render because the window is minimized");
                return;
            }
            let output = self.state.update(w.scale_factor());
            // dump_output(&output).expect("failed to dump egui output");

            let triangles = self.state.cx.tessellate(output.shapes, output.pixels_per_point);

            w.request_redraw(); // Reserve the next redrawing

            let size = w.inner_size();
            let screen = render::ScreenDescriptor {
                pixel_per_point: output.pixels_per_point,
                screen_width: size.width,
                screen_height: size.height,
            };

            match r.render(&screen, &triangles, &output.textures_delta) {
                Ok(_) => {},
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    r.request_resize(screen);
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
                if self.main_window.is_some() {
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

#[allow(unused)]
fn dump_output(output: &egui::FullOutput) -> Result<(), anyhow::Error> {
    println!("** Dump/ppp: {}", output.pixels_per_point);
    println!("** Dump output for platform");
    println!("{}", serde_json::to_string_pretty(&output.platform_output)?);
    println!();

    println!("** Dump output for viewports");
    for (id, vp) in &output.viewport_output {
        println!("viewport/id: {:?}, parent_id: {:?}, ", id, vp.parent);
        println!("    class: {}, delay: {:?}", serde_json::to_string(&vp.class)?, vp.repaint_delay);
        println!("    commands: {:?}", vp.commands);
    }
    println!();

    println!("** Dump output for texture delta");
    println!("{}", serde_json::to_string_pretty(&output.textures_delta)?);

    println!("** Dump output for shapes");
    for (i, shape) in output.shapes.iter().enumerate() {
        println!("[{i:>4}] {:?}", shape);
    }
    println!("----");

    Ok(())
}
