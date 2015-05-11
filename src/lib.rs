extern crate piston;
extern crate glutin_window;
extern crate fibe;
extern crate snowstorm;
extern crate input;

use fibe::*;
use piston::window::{Window, WindowSettings};
use glutin_window::{GlutinWindow, OpenGL};

pub use snowstorm::channel::*;

pub struct Engine {
    input: (Sender<input::Input>, Receiver<input::Input>),
    pool: fibe::Frontend
}

impl Engine {
    /// Create a new Engine context
    pub fn new() -> Engine {
        Engine {
            input: channel(),
            pool: fibe::Frontend::new()
        }
    }

    /// Fetch a copy of the input stream and run actor
    /// with the input stream as a input
    pub fn start_input_processor<F>(&mut self, actor: F) where F: FnOnce(&mut fibe::Schedule, Receiver<input::Input>)+Send+'static {
        let rx = self.input.1.clone();
        task(|sched| {
            actor(sched, rx);
        }).start(&mut self.pool);
    }

    /// run the engine
    pub fn run<F>(mut self, mut render: F) where F: FnMut(&mut fibe::Schedule, &mut GlutinWindow) {
        let mut window = GlutinWindow::new(
            OpenGL::_3_2,
            WindowSettings::new("snowmew", [640, 480])
        );

        let (mut send, recv)  = self.input;
        drop(recv);

        'main: while !window.should_close() {
            while let Some(event) = Window::poll_event(&mut window) {
                send.send(event);
            }
            send.next_frame();
            render(&mut self.pool, &mut window);
        }
    }
}