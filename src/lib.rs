extern crate glutin;
extern crate fibe;
extern crate snowstorm;

use fibe::*;

pub use snowstorm::channel::*;

pub struct Engine {
    input: (Sender<glutin::Event>, Receiver<glutin::Event>),
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

    /// Fetch a copy of the input stream
    pub fn start_input_processor<F>(&mut self, actor: F) where F: FnOnce(&mut fibe::Schedule, Receiver<glutin::Event>)+Send+'static {
        let rx = self.input.1.clone();
        task(|sched| {
            actor(sched, rx);
        }).start(&mut self.pool);
    }

    /// run the engine
    pub fn run<F>(mut self, mut render: F) where F: FnMut(&mut fibe::Schedule, &glutin::Window) {
        let window = glutin::WindowBuilder::new()
            .with_title("Demo".to_string())
            .with_vsync()
            .with_gl(glutin::GlRequest::Specific(glutin::Api::OpenGl, (3, 2)))
            .with_srgb(Some(true))
            .build().unwrap();

        let (mut send, recv)  = self.input;
        drop(recv);

        'main: loop {
            for event in window.poll_events() {
                send.send(event);
            }
            send.next_frame();
            render(&mut self.pool, &window);
        }
    }
}