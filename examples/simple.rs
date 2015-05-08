extern crate engine;
extern crate glutin;
extern crate fibe;
extern crate snowstorm;

use fibe::task;
use snowstorm::channel::*;

fn process_input(sched: &mut fibe::Schedule, mut ch: Receiver<glutin::Event>) {
    // Print out the messages
    while let Some(msg) = ch.try_recv() {
        println!("{:?}", msg);
    }

    // Indicate that this can migrate to the next frame
    if ch.closed() {
        println!("Next Frame");
        ch.next_frame();
    }

    let signal = ch.signal();
    task(|sched| {
        process_input(sched, ch);
    }).after(signal).start(sched);
}

fn main() {
    let mut engine = engine::Engine::new();

    engine.start_input_processor(|sched, msgs| process_input(sched, msgs));

    engine.run(|_, window| {
        window.swap_buffers();
    });
}