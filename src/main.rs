use egui_demo::app;
use winit::event_loop::EventLoop;

fn main() -> Result<(), anyhow::Error> {
    pollster::block_on(run())
}

async fn run() ->  Result<(), anyhow::Error> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app::App::new())?;
    Ok(())
}
