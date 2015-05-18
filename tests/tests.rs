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
               Sink,
               Receiver<Operation<Entity, Solved>>) {

    let mut sched = Frontend::new();

    let (parent_tx, rx) = channel();
    let parent_rx = parent(&mut sched, rx);

    let (position_tx, rx) = channel();
    let position_rx = position(&mut sched, rx, parent_rx);

    (sched, Sink{parent: parent_tx, position: position_tx}, position_rx)
}

struct Sink {
    parent: Sender<parent::Message>,
    position: Sender<Operation<Entity, Delta>>
}

impl Sink {
    fn next_frame(&mut self) {
        self.parent.next_frame();
        self.position.next_frame();        
    }
}

impl WriteEntity<Entity, Delta> for Sink {
    fn write(&mut self, eid: Entity, delta: Delta) {
        self.position.send(Operation::Upsert(eid, delta));
    }
}

impl WriteEntity<Entity, Parent> for Sink {
    fn write(&mut self, eid: Entity, parent: Parent) {
        self.parent.send(Operation::Upsert(eid, parent));
    }
}

#[test]
fn children() {
    let (front, mut sink, mut solved) = setup();

    let e0 = Entity::new()
        .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
        .write(&mut sink);
    let e1 = Entity::new()
        .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
        .bind(Parent::Child(e0))
        .write(&mut sink);
    let e2 = Entity::new()
        .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
        .bind(Parent::Child(e1))
        .write(&mut sink);
    let e3 = Entity::new()
        .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
        .bind(Parent::Child(e2))
        .write(&mut sink);
    let e4 = Entity::new()
        .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
        .bind(Parent::Child(e3))
        .write(&mut sink);

    sink.next_frame();

    let mut hm = HashMap::new();
    while let Ok(x) = solved.recv() {
        x.write(&mut hm);
    }

    let vec = Vector4::new(0f32, 0f32, 0f32, 1f32);
    assert_eq!(hm.get(&e0).unwrap().0.mul_v(&vec), Vector4::new(1f32, 1f32, 1f32, 1f32));
    assert_eq!(hm.get(&e1).unwrap().0.mul_v(&vec), Vector4::new(2f32, 2f32, 2f32, 1f32));
    assert_eq!(hm.get(&e2).unwrap().0.mul_v(&vec), Vector4::new(3f32, 3f32, 3f32, 1f32));
    assert_eq!(hm.get(&e3).unwrap().0.mul_v(&vec), Vector4::new(4f32, 4f32, 4f32, 1f32));
    assert_eq!(hm.get(&e4).unwrap().0.mul_v(&vec), Vector4::new(5f32, 5f32, 5f32, 1f32));
    drop(front);
}

#[test]
fn children_tree() {
    let (front, mut sink, mut solved) = setup();

    let e0 = Entity::new()
           .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
           .write(&mut sink);
    let e1 = Entity::new()
           .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(-1f32, -1f32, -1f32)}))
           .write(&mut sink);
    let e2 = Entity::new()
           .bind(Parent::Child(e0))
           .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
           .write(&mut sink);
    let e3 = Entity::new()
           .bind(Parent::Child(e0))
           .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(-1f32, -1f32, -1f32)}))
           .write(&mut sink);
    let e4 = Entity::new()
           .bind(Parent::Child(e1))
           .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
           .write(&mut sink);
    let e5 = Entity::new()
           .bind(Parent::Child(e1))
           .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(-1f32, -1f32, -1f32)}))
           .write(&mut sink);

    sink.next_frame();

    let mut hm = HashMap::new();
    while let Ok(x) = solved.recv() {
        x.write(&mut hm);
    }

    let vec = Vector4::new(0f32, 0f32, 0f32, 1f32);
    assert_eq!(hm.get(&e2).unwrap().0.mul_v(&vec), Vector4::new(2f32, 2f32, 2f32, 1f32));
    assert_eq!(hm.get(&e3).unwrap().0.mul_v(&vec), Vector4::new(0f32, 0f32, 0f32, 1f32));
    assert_eq!(hm.get(&e4).unwrap().0.mul_v(&vec), Vector4::new(0f32, 0f32, 0f32, 1f32));
    assert_eq!(hm.get(&e5).unwrap().0.mul_v(&vec), Vector4::new(-2f32, -2f32, -2f32, 1f32));
    drop(front);
}

#[test]
fn exit() {
    let (front, sink, mut solved) = setup();
    drop(sink);

    assert_eq!(ReceiverError::ChannelClosed, solved.recv().err().unwrap());
    drop(front);
}

fn count(rx: &mut Receiver<Operation<Entity, Solved>>) -> usize {
    let mut count = 0;
    while let Ok(_) = rx.recv() {
        count += 1;
    }
    rx.next_frame();
    count
}

#[test]
fn dirty_count() {
    let (front, mut sink, mut solved) = setup();
    let mut entitys = Vec::new();
    entitys.push(
        Entity::new()
        .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
        .write(&mut sink)
    );
    for i in 0..9 {
        let last = entitys[i];
        entitys.push(
            Entity::new()
            .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
            .bind(Parent::Child(last))
            .write(&mut sink)
        );
    }
    sink.next_frame();
    assert_eq!(10, count(&mut solved));
    for i in 0..10 {
        entitys[i]
            .bind(Delta(Decomposed{scale: 1f32, rot: Quaternion::identity(), disp: Vector3::new(1f32, 1f32, 1f32)}))
            .write(&mut sink);
        sink.next_frame();
        assert_eq!(10-i, count(&mut solved));
    }
    drop(front);
}