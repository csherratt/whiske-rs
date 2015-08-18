extern crate fibe as fibers;
extern crate snowstorm;
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glutin;
extern crate glutin;
extern crate time;

#[cfg(feature="virtual_reality")]
extern crate vr;
#[cfg(feature="virtual_reality")]
extern crate gfx_vr;

pub mod event;
use event::WindowEvent;

pub mod fibe {
    pub use fibers::*;
}

use fibe::*;

pub use snowstorm::channel::*;

pub type Window<D, R> = gfx::extra::stream::OwnedStream<D, gfx_window_glutin::Output<R>>;

pub struct Engine<D: gfx::Device, F, R: gfx::Resources> {
    input: (Sender<WindowEvent>, Receiver<WindowEvent>),
    pool: fibe::Frontend,
    window: Window<D, R>,
    render_args: Option<RenderArgs<D, F>>,
    render: Option<Box<FnMut(&mut fibe::Schedule, &mut Window<D, R>)>>,
}

pub struct RenderArgs<D: gfx::Device, F> {
    pub device: D,
    pub factory: F,
    #[cfg(feature="virtual_reality")]
    pub vr: Option<vr::IVRSystem>
}

impl Engine<gfx_device_gl::Device,
            gfx_device_gl::Factory,
            gfx_device_gl::Resources> {


    /// Create a new Engine context
    #[cfg(feature="virtual_reality")]
    pub fn new() -> Engine<gfx_device_gl::Device,
                           gfx_device_gl::Factory,
                           gfx_device_gl::Resources> {

        let vr = vr::IVRSystem::init();

        let window = if let Ok(ref vr) = vr {
            gfx_vr::window::glutin::build(vr)
        } else {
            glutin::WindowBuilder::new()
                .with_title("whiske-rs".to_string())
                .with_dimensions(800, 600)
                .with_gl(glutin::GL_CORE)
                .with_depth_buffer(24)
                .build()
        }.unwrap();

        let (stream, device, factory) = gfx_window_glutin::init(window);

        let ra = RenderArgs {
            vr: vr.ok(),
            device: device,
            factory: factory
        };

        Engine {
            input: channel(),
            pool: fibe::Frontend::new(),
            window: stream,
            render_args: Some(ra),
            render: None
        }
    }

    /// Create a new Engine context
    #[cfg(not(feature="virtual_reality"))]
    pub fn new() -> Engine<gfx_device_gl::Device,
                           gfx_device_gl::Factory,
                           gfx_device_gl::Resources> {

        let window = glutin::WindowBuilder::new()
            .with_title("whiske-rs".to_string())
            .with_dimensions(800, 600)
            .with_gl(glutin::GL_CORE)
            .with_depth_buffer(24)
            .build().unwrap();

        let (stream, device, factory) = gfx_window_glutin::init(window);

        let ra = RenderArgs {
            device: device,
            factory: factory
        };

        Engine {
            input: channel(),
            pool: fibe::Frontend::new(),
            window: stream,
            render_args: Some(ra),
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
        where C: FnOnce(&mut fibe::Schedule, Receiver<WindowEvent>)+Send+'static {
        
        let rx = self.input.1.clone();
        task(|sched| {
            actor(sched, rx);
        }).start(&mut self.pool);
    }

    /// Fetch a copy of the input stream and run actor
    /// with the input stream as a input
    pub fn start_render<C>(&mut self, render: C)
        where C: FnOnce(&mut fibe::Schedule, RenderArgs<D, F>) -> Box<FnMut(&mut fibe::Schedule, &mut Window<D, R>)> {

        let ra = self.render_args.take().expect("Only one render can be created");
        let render = render(&mut self.pool, ra);
        self.render = Some(render);
    }

    /// Get the scheduler to scheduler tasks on it
    pub fn sched(&mut self) -> &mut fibe::Schedule {
        &mut self.pool
    }

    /// Get a copy of the input channel
    pub fn input_channel(&self) -> InputChannel {
        self.input.1.clone()
    }

    /// run the engine
    pub fn run(mut self) {
        let mut run = true;
        let (mut send, recv) = self.input;
        drop(recv);


        let start = time::precise_time_s();
        let mut render = self.render.take().expect("no render installed!");

        while run {
            for event in self.window.out.window.poll_events() {
                match event {
                    glutin::Event::Closed => {
                        run = false;
                    }
                    _ => ()
                }
                WindowEvent::from_glutin(event).map(|e| send.send(e));
            }
            send.send(WindowEvent::TimeStamp(time::precise_time_s() - start));
            send.next_frame();
            render(&mut self.pool, &mut self.window);
        }
    }
}

// The input channel
pub type InputChannel = Receiver<WindowEvent>;