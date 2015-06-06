extern crate fibe;
extern crate snowstorm;
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;

use fibe::*;
use glutin::Event;

pub use snowstorm::channel::*;

pub type Window<D, R> = gfx::extra::stream::OwnedStream<D, gfx_window_glutin::Output<R>>;

pub struct Engine<D: gfx::Device, F, R: gfx::Resources> {
    input: (Sender<Event>, Receiver<Event>),
    pool: fibe::Frontend,
    window: Window<D, R>,
    render_args: Option<(D, F)>,
    render: Option<Box<FnMut(&mut fibe::Schedule, &mut Window<D, R>)>>
}

impl Engine<gfx_device_gl::Device,
            gfx_device_gl::Factory,
            gfx_device_gl::Resources> {
    /// Create a new Engine context
    pub fn new() -> Engine<gfx_device_gl::Device,
                           gfx_device_gl::Factory,
                           gfx_device_gl::Resources> {
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
}


impl<D, F, R> Engine<D, F, R>
    where D: gfx::Device,
          R: gfx::Resources

 {
    /// Fetch a copy of the input stream and run actor
    /// with the input stream as a input
    pub fn start_input_processor<C>(&mut self, actor: C)
        where C: FnOnce(&mut fibe::Schedule, Receiver<Event>)+Send+'static {
        
        let rx = self.input.1.clone();
        task(|sched| {
            actor(sched, rx);
        }).start(&mut self.pool);
    }

    /// Fetch a copy of the input stream and run actor
    /// with the input stream as a input
    pub fn start_render<C>(&mut self, render: C)
        where C: FnOnce(&mut fibe::Schedule, D, F) -> Box<FnMut(&mut fibe::Schedule, &mut Window<D, R>)> {

        let (device, factory) = self.render_args.take().expect("Only one render can be created");
        let render = render(&mut self.pool, device, factory);
        self.render = Some(render);
    }

    /// Get the scheduler to scheduler tasks on it
    pub fn sched(&mut self) -> &mut fibe::Schedule {
        &mut self.pool
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