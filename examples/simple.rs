extern crate engine;
extern crate fibe;
extern crate snowstorm;
extern crate glutin;

use std::thread;
use fibe::task;
use snowstorm::channel::*;
use glutin::Event;

fn process_input(sched: &mut fibe::Schedule, index: u32, mut ch: Receiver<Event>) {
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

    engine.start_render(|_,_,_|{
        println!("to do render here!");
        Box::new(move |_, stream| {
            stream.out.window.swap_buffers();
        })
    });

    engine.run();
}
