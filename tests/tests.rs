extern crate parent;
extern crate transform;
extern crate fibe;
extern crate snowstorm;
#[macro_use(router)]
extern crate entity;
extern crate cgmath;

use std::collections::HashMap;
use entity::*;
use parent::{parent, Parent};
use transform::*;
use fibe::*;
use snowstorm::channel::*;
use cgmath::*;

fn setup() -> (Frontend,
               Sink,
               TransformOutput) {

    let mut sched = Frontend::new();

    let (parent_tx, rx) = channel();
    let parent_rx = parent(&mut sched, rx);

    let (tinput, toutput) = transform(&mut sched, parent_rx);

    (sched, Sink{parent: parent_tx, transform: tinput}, toutput)
}

router! {
    struct Sink {
        [Entity, Parent] => parent: Sender<parent::Message>,
        [Entity, Delta] => transform: TransformInput
    }
}

impl Sink {
    fn next_frame(&mut self) {
        self.parent.next_frame();
        self.transform.next_frame();
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
    assert_eq!(hm.get(&e0).unwrap().to_mat().mul_v(&vec), Vector4::new(1f32, 1f32, 1f32, 1f32));
    assert_eq!(hm.get(&e1).unwrap().to_mat().mul_v(&vec), Vector4::new(2f32, 2f32, 2f32, 1f32));
    assert_eq!(hm.get(&e2).unwrap().to_mat().mul_v(&vec), Vector4::new(3f32, 3f32, 3f32, 1f32));
    assert_eq!(hm.get(&e3).unwrap().to_mat().mul_v(&vec), Vector4::new(4f32, 4f32, 4f32, 1f32));
    assert_eq!(hm.get(&e4).unwrap().to_mat().mul_v(&vec), Vector4::new(5f32, 5f32, 5f32, 1f32));
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
    assert_eq!(hm.get(&e2).unwrap().to_mat().mul_v(&vec), Vector4::new(2f32, 2f32, 2f32, 1f32));
    assert_eq!(hm.get(&e3).unwrap().to_mat().mul_v(&vec), Vector4::new(0f32, 0f32, 0f32, 1f32));
    assert_eq!(hm.get(&e4).unwrap().to_mat().mul_v(&vec), Vector4::new(0f32, 0f32, 0f32, 1f32));
    assert_eq!(hm.get(&e5).unwrap().to_mat().mul_v(&vec), Vector4::new(-2f32, -2f32, -2f32, 1f32));
    drop(front);
}

#[test]
fn exit() {
    let (front, sink, mut solved) = setup();
    drop(sink);

    assert_eq!(ReceiverError::ChannelClosed, solved.recv().err().unwrap());
    drop(front);
}

fn count(rx: &mut TransformOutput) -> usize {
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