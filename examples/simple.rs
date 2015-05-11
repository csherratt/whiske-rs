extern crate piston;
extern crate engine;
extern crate input;
extern crate fibe;
extern crate snowstorm;

use std::thread;
use piston::window::Window;
use fibe::task;
use snowstorm::channel::*;

fn process_input(sched: &mut fibe::Schedule, index: u32, mut ch: Receiver<input::Input>) {
    // Print out the messages
    while let Some(msg) = ch.try_recv() {
        println!("{}: {:?} {:?}", index, thread::current(), msg);
    }

    // Indicate that this can migrate to the next frame
    if ch.closed() {
        ch.next_frame();
    }

    let signal = ch.signal();
    task(move |sched| process_input(sched, index, ch)).after(signal).start(sched);
}

fn main() {
    let mut engine = engine::Engine::new();

    for i in 0..1 {
        engine.start_input_processor(move |sched, msgs| process_input(sched, i, msgs));
    }

    engine.run(|_, window|  window.swap_buffers() );
}
