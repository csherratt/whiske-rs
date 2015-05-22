extern crate fibe;
extern crate snowstorm;
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;

use fibe::*;
use gfx_device_gl::{Device, Factory};
use glutin::Event;

pub use snowstorm::channel::*;

pub type Window = gfx::extra::stream::OwnedStream<
    gfx_device_gl::Device,
    gfx_window_glutin::Output<gfx_device_gl::Resources>
>;

pub struct Engine {
    input: (Sender<Event>, Receiver<Event>),
    pool: fibe::Frontend,
    window: Window,
    render_args: Option<(Device, Factory)>,
    render: Option<Box<FnMut(&mut fibe::Schedule, &mut Window)>>
}

impl Engine {
    /// Create a new Engine context
    pub fn new() -> Engine {
        let (stream, device, factory) = gfx_window_glutin::init(
            glutin::Window::new().unwrap()
        );

        Engine {
            input: channel(),
            pool: fibe::Frontend::new(),
            window: stream,
            render_args: Some((device, factory)),
            render: None
        }
    }

    /// Fetch a copy of the input stream and run actor
    /// with the input stream as a input
    pub fn start_input_processor<F>(&mut self, actor: F) where F: FnOnce(&mut fibe::Schedule, Receiver<Event>)+Send+'static {
        let rx = self.input.1.clone();
        task(|sched| {
            actor(sched, rx);
        }).start(&mut self.pool);
    }

    /// Fetch a copy of the input stream and run actor
    /// with the input stream as a input
    pub fn start_render<F>(&mut self, render: F)
        where F: FnOnce(&mut fibe::Schedule, Device, Factory) -> Box<FnMut(&mut fibe::Schedule, &mut Window)> {

        let (device, factory) = self.render_args.take().expect("Only one render can be created");
        let render = render(&mut self.pool, device, factory);
        self.render = Some(render);

    }
    /// run the engine
    pub fn run(mut self) {
        let (mut send, recv) = self.input;
        drop(recv);

        let mut render = self.render.take().expect("no render installed!");

        'main: while !self.window.out.window.is_closed() {
            for event in self.window.out.window.poll_events() {
                send.send(event);
            }
            send.next_frame();
            render(&mut self.pool, &mut self.window);
        }
    }
}