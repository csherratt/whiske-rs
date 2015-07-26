extern crate parent;
extern crate fibe;
extern crate entity;

use entity::*;
use parent::{parent, Parent};
use fibe::*;

#[test]
fn add_children() {
    let mut sched = Frontend::new();
    let mut parent = parent(&mut sched);

    let root = Entity::new().bind(Parent::Root).write(&mut parent);
    let child = Entity::new().bind(Parent::Child(root)).write(&mut parent);

    parent = parent.next_frame().get().unwrap();

    assert_eq!(parent.read(&root).unwrap(), &Parent::Root);
    assert_eq!(parent.read(&child).unwrap(), &Parent::Child(root));
}

#[test]
fn delete_children() {
    let mut sched = Frontend::new();
    let mut parent = parent(&mut sched);

    let root0 = Entity::new().bind(Parent::Root).write(&mut parent);
    let child0 = Entity::new().bind(Parent::Child(root0)).write(&mut parent);
    root0.delete(&mut parent);
    
    let root1 = Entity::new().bind(Parent::Root).write(&mut parent);
    let child1 = Entity::new().bind(Parent::Child(root1)).write(&mut parent);
    child1.delete(&mut parent);
    
    parent = parent.next_frame().get().unwrap();

    assert_eq!(parent.read(&root0), None);
    assert_eq!(parent.read(&child0), None);
    assert_eq!(parent.read(&root1).unwrap(), &Parent::Root);
    assert_eq!(parent.read(&child1), None);
}

#[test]
fn huge_number_of_children() {
    let mut sched = Frontend::new();
    let mut parent = parent(&mut sched);

    let root = Entity::new().bind(Parent::Root).write(&mut parent);
    let mut p = root;

    let mut children = Vec::new();
    for _ in 0..1_000 {
        let child = Entity::new().bind(Parent::Child(p)).write(&mut parent);
        p = child;
        children.push(child);
    }

    parent = parent.next_frame().get().unwrap();

    assert_eq!(parent.read(&root).unwrap(), &Parent::Root);
    p = root;
    for child in children.iter() {
        assert_eq!(parent.read(child).unwrap(), &Parent::Child(p));
        p = *child;
    }

    root.delete(&mut parent);

    parent = parent.next_frame().get().unwrap();

    assert_eq!(parent.read(&root), None);
    for child in children.iter() {
        assert_eq!(parent.read(child), None);
    }
}
