extern crate fibe as fibers;
extern crate snowstorm;
extern crate gfx;
extern crate gfx_device_gl;
extern crate gfx_window_glfw;
extern crate glfw;
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
use glfw::{Context};

pub use snowstorm::channel::*;

pub type Window<D, R> = gfx::extra::stream::OwnedStream<D, gfx_window_glfw::Output<R>>;

pub struct Engine<D: gfx::Device, F, R: gfx::Resources> {
    glfw: glfw::Glfw,
    events: std::sync::mpsc::Receiver<(f64, glfw::WindowEvent)>,
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

fn common_glfw_config(glfw: &mut glfw::Glfw) {
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
}

impl Engine<gfx_device_gl::Device,
            gfx_device_gl::Factory,
            gfx_device_gl::Resources> {


    /// Create a new Engine context
    #[cfg(feature="virtual_reality")]
    pub fn new() -> Engine<gfx_device_gl::Device,
                           gfx_device_gl::Factory,
                           gfx_device_gl::Resources> {

        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
        common_glfw_config(&mut glfw);
        let vr = vr::IVRSystem::init();

        let (mut window, events) = if let Ok(ref vr) = vr {
            gfx_vr::build_window(&mut glfw, vr)
        } else {
            glfw.create_window(800, 600, "whiske-rs", glfw::WindowMode::Windowed)
        }.unwrap();

        window.set_all_polling(true);
        window.make_current();
        glfw.set_swap_interval(1);


        let (stream, device, factory) = gfx_window_glfw::init(window);

        let ra = RenderArgs {
            vr: vr.ok(),
            device: device,
            factory: factory
        };

        Engine {
            glfw: glfw,
            events: events,
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

        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
        common_glfw_config(&mut glfw);

        let (mut window, events) = 
            glfw.create_window(800, 600, "whiske-rs", glfw::WindowMode::Windowed).unwrap();

        window.set_all_polling(true);
        window.make_current();
        glfw.set_swap_interval(0);


        let (stream, device, factory) = gfx_window_glfw::init(window);

        let ra = RenderArgs {
            device: device,
            factory: factory
        };

        Engine {
            glfw: glfw,
            events: events,
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
            self.glfw.poll_events();
            for (_, event) in glfw::flush_messages(&self.events) {
                match event {
                    glfw::WindowEvent::Close => {
                        run = false;
                    }
                    _ => ()
                }
                send.send(WindowEvent::from_glfw(event));
            }
            send.send(WindowEvent::TimeStamp(time::precise_time_s() - start));
            send.next_frame();
            render(&mut self.pool, &mut self.window);
        }
    }
}

// The input channel
pub type InputChannel = Receiver<WindowEvent>;