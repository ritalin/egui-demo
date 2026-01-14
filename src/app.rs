use std::sync::Arc;
use winit::{application::ApplicationHandler, dpi::PhysicalSize, event::{self, ElementState, WindowEvent}, event_loop::ActiveEventLoop, keyboard::{KeyCode, PhysicalKey}, window::Window};

use crate::render;

struct AppState {
    zoom_factor: f32,
}
impl AppState {
    fn new() -> Self {
        Self {
            zoom_factor: 1.0,
        }
    }

    fn update(&mut self, window: &winit::window::Window, state: &mut egui_winit::State) -> (bool, egui::FullOutput) {
        let scale_factor = window.scale_factor() as f32;
        let old_zoom = self.zoom_factor;
        let input = state.take_egui_input(window);

        let mut output = state.egui_ctx().run(input, |cx| {
            egui::Area::new(egui::Id::new("winit + egui + wgpu says hello!"))
                .show(cx, |ui| {
                    ui.label("Label!");
                    if ui.button("boom!").clicked() {
                        println!("Boom!");
                    }

                    ui.separator();
                    ui.horizontal(|ui| {
                        ui.label(&format!("ppp: scale ({}) x mag ({}) = {}", scale_factor, old_zoom, scale_factor * old_zoom));
                    });
                    ui.horizontal(|ui| {
                        if ui.button("-").clicked() {
                            self.zoom_factor = (self.zoom_factor - 0.1).max(0.3);
                        }
                        if ui.button("+").clicked() {
                            self.zoom_factor = (self.zoom_factor + 0.1).min(3.0);
                        }
                    });
                })
            ;
        });

        state.egui_ctx().set_pixels_per_point(scale_factor * self.zoom_factor);
        output.pixels_per_point = scale_factor * self.zoom_factor;

        (self.zoom_factor != old_zoom, output)
    }
}

pub struct App {
    main_window: Option<Arc<Window>>,
    raw_handle: Option<render::RawWindow>,
    renderer: Option<render::WgpuRenderer>,
    window_state: Option<egui_winit::State>,
    state: AppState,
}
impl App {
    const DEFAULT_WIDTH: u32 = 1360;
    const DEFAULT_HEIGHT: u32 = 1024;

    pub fn new() -> Self {
        Self {
            main_window: None,
            raw_handle: None,
            renderer: None,
            window_state: None,
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
        renderer.request_resize(&screen);

        self.renderer = Some(renderer);

        self.window_state = Some(egui_winit::State::new(
            egui::Context::default(),
            egui::viewport::ViewportId::ROOT,
            &w,
            Some(w.scale_factor() as f32),
            None,
            None
        ));

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
                pixel_per_point: w.scale_factor() as f32 * self.state.zoom_factor,
                screen_width: size.width,
                screen_height: size.height,
            };
            renderer.request_resize(&screen);
        }
    }

    fn handle_redraw(&mut self, _event_loop: &ActiveEventLoop) {
        if let (Some(w), Some(s), Some(r)) = (self.main_window.as_ref(), self.window_state.as_mut(), self.renderer.as_mut()) {
            if let Some(y) = w.is_minimized() && y {
                log::info!("Skip to render because the window is minimized");
                return;
            }
            let (scale_changed, output) = self.state.update(w, s);
            // dump_output(&output).expect("failed to dump egui output");

            let triangles = s.egui_ctx().tessellate(output.shapes, output.pixels_per_point);

            w.request_redraw(); // Reserve the next redrawing

            let size = w.inner_size();
            let screen = render::ScreenDescriptor {
                pixel_per_point: output.pixels_per_point,
                screen_width: size.width,
                screen_height: size.height,
            };

            if scale_changed {
                r.request_rescale(&screen);
            }

            match r.render(&screen, &triangles, &output.textures_delta) {
                Ok(_) => {},
                Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                    r.request_resize(&screen);
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
        let (Some(w), Some(state)) = (self.main_window.as_ref(), self.window_state.as_mut()) else { return };
        let _ = state.on_window_event(&w, &event);

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
