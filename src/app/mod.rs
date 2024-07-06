use std::sync::Arc;

use winit::{application::ApplicationHandler, event::WindowEvent, event_loop::{ActiveEventLoop, EventLoop}, window::WindowAttributes};

use crate::graphics::State;

#[derive(Default)]
pub struct App<'a> {
    state: Option<State<'a>>,
}

impl<'a> ApplicationHandler for App<'a> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(event_loop.create_window(WindowAttributes::default()).expect("Failed to create window"));

        let state_future = State::new(window);
        let state = futures::executor::block_on(state_future);

        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        let state = self.state.as_mut().expect("State not created");

        if window_id == state.window().id() {
            match event {
                WindowEvent::CloseRequested => {
                    event_loop.exit();
                }
                WindowEvent::RedrawRequested => {
                    state.update();

                    match state.render() {
                        Ok(_) => (),
                        Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(e) => eprintln!("{:?}", e),
                    }

                    state.window().request_redraw();
                }
                WindowEvent::Resized(physical_size) => {
                    state.resize(physical_size);
                }
                _ => ()
            }
        }
    }
}

impl App<'_> {
    pub fn run(&mut self) {
        let event_loop = EventLoop::new().expect("Failed to create event loop");
        let _ = event_loop.run_app(self);
    }

}
