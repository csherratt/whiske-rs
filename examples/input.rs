extern crate engine;
extern crate snowstorm;
extern crate glfw;
extern crate fibe;

use std::thread;
use snowstorm::channel::*;
use glfw::{WindowEvent, Context};

fn process_input(sched: &mut fibe::Schedule, index: u32, mut ch: Receiver<WindowEvent>) {
    loop {
        for msg in ch.iter() {
            println!("{}: {:?} {:?}", index, thread::current(), msg);
        }
        if !ch.next_frame() {
            return;
        }
    }
}

fn main() {
    let mut engine = engine::Engine::new();

    engine.start_input_processor(move |sched, msgs| process_input(sched, 0, msgs));

    engine.start_render(|_,_|{
        println!("to do render here!");
        Box::new(move |_, stream| {
            stream.out.window.swap_buffers();
        })
    });

    engine.run();
}
