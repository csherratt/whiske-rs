extern crate engine;
extern crate snowstorm;
extern crate glutin;
extern crate fibe;

use std::thread;
use snowstorm::channel::*;
use glutin::Event;

fn process_input(sched: &mut fibe::Schedule, index: u32, mut ch: Receiver<Event>) {
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

    engine.start_input_processor(move |sched, msgs| process_input(sched, i, msgs));

    engine.start_render(|_,_,_|{
        println!("to do render here!");
        Box::new(move |_, stream| {
            stream.out.window.swap_buffers();
        })
    });

    engine.run();
}
