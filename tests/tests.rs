extern crate parent;
extern crate position;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate cgmath;

use std::collections::HashMap;
use entity::*;
use parent::{parent, Parent};
use position::*;
use fibe::*;
use snowstorm::channel::*;
use cgmath::*;

fn setup() -> (Frontend,
               Sender<parent::Message>,
               Sender<Operation<Delta>>,
               Receiver<Operation<Solved>>) {

    let mut sched = Frontend::new();

    let (parent_tx, rx) = channel();
    let parent_rx = parent(&mut sched, rx);

    let (position_tx, rx) = channel();
    let position_rx = position(&mut sched, rx, parent_rx);

    (sched, parent_tx, position_tx, position_rx)
}

#[test]
fn simple() {
    let (front, mut parent, mut position, mut solved) = setup();

    for _ in 0..100 {
        Entity::new().bind(Delta(Matrix4::identity())).write(&mut position);
    }

    position.next_frame();
    parent.next_frame();

    for _ in 0..100 {
        solved.recv().unwrap();
    }

    drop(front);
}

