extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;

use std::collections::HashMap;
use entity::*;
use parent::{parent, Parent};
use fibe::*;
use snowstorm::channel::*;


#[test]
fn add_children() {
    let mut hm = HashMap::new();
    let mut sched = Frontend::new();
    let (mut tx, rx) = channel();

    let mut output = parent(&mut sched, rx);
    let root = Entity::new().bind(Parent::Root).write(&mut tx);
    let child = Entity::new().bind(Parent::Child(root)).write(&mut tx);
    tx.next_frame();

    while let Ok(&msg) = output.recv() {
        msg.write(&mut hm);
    }

    assert_eq!(hm.get(&root).unwrap(), &Parent::Root);
    assert_eq!(hm.get(&child).unwrap(), &Parent::Child(root));
}

#[test]
fn delete_children() {
    let mut hm = HashMap::new();
    let mut sched = Frontend::new();
    let (mut tx, rx) = channel();

    let mut output = parent(&mut sched, rx);
    let root0 = Entity::new().bind(Parent::Root).write(&mut tx);
    let child0 = Entity::new().bind(Parent::Child(root0)).write(&mut tx);
    root0.delete(&mut tx);
    
    let root1 = Entity::new().bind(Parent::Root).write(&mut tx);
    let child1 = Entity::new().bind(Parent::Child(root1)).write(&mut tx);
    child1.delete(&mut tx);
    
    tx.next_frame();

    while let Ok(&msg) = output.recv() {
        msg.write(&mut hm);
    }

    assert_eq!(hm.get(&root0), None);
    assert_eq!(hm.get(&child0), None);
    assert_eq!(hm.get(&root1).unwrap(), &Parent::Root);
    assert_eq!(hm.get(&child1), None);
}


#[test]
fn huge_number_of_children() {
    let mut hm = HashMap::new();
    let mut sched = Frontend::new();
    let (mut tx, rx) = channel();

    let mut output = parent(&mut sched, rx);
    let root = Entity::new().bind(Parent::Root).write(&mut tx);
    let mut parent = root;

    let mut children = Vec::new();
    for _ in 0..1_000 {
        let child = Entity::new().bind(Parent::Child(parent)).write(&mut tx);
        parent = child;
        children.push(child);
    }

    tx.next_frame();
    while let Ok(&msg) = output.recv() {
        msg.write(&mut hm);
    }
    output.next_frame();

    assert_eq!(hm.get(&root).unwrap(), &Parent::Root);
    parent = root;
    for child in children.iter() {
        assert_eq!(hm.get(child).unwrap(), &Parent::Child(parent));
        parent = *child;
    }

    root.delete(&mut tx);
    tx.next_frame();
    while let Ok(&msg) = output.recv() {
        msg.write(&mut hm);
    }

    assert_eq!(hm.get(&root), None);
    for child in children.iter() {
        assert_eq!(hm.get(child), None);
    }
}
