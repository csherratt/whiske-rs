extern crate parent;
extern crate scene;
extern crate entity;
extern crate fibe;

use std::collections::HashMap;
use parent::parent;
use scene::{scene, Scene};
use entity::Entity;

#[test]
fn pass_through() {
    let mut front = fibe::Frontend::new();
    let (mut pinput, poutput) = parent(&mut front);
    let (mut src, mut sink) = scene(&mut front, poutput);

    let scene = Scene::new();
    let entities: Vec<Entity> = (0..10).map(|_| Entity::new()).collect();
    for &e in &entities {
        scene.bind(e, &mut src);
    }

    src.next_frame();
    pinput.next_frame();

    let mut map = HashMap::new();
    while let Some(x) = sink.write_into(&mut map) {
        x.wait().unwrap();
    }
    sink.next_frame();

    for e in &entities {
        assert!(map.get(&scene).unwrap().contains(e));
        scene.unbind(*e, &mut src);
    }

    src.next_frame();
    pinput.next_frame();

    while let Some(x) = sink.write_into(&mut map) {
            x.wait().unwrap();
    }

    assert!(map.get(&scene).is_none());
}

#[test]
fn delete_scenes() {
    let mut front = fibe::Frontend::new();
    let (mut pinput, poutput) = parent(&mut front);
    let (mut src, mut sink) = scene(&mut front, poutput);

    let scenes: Vec<Scene> = (0..10).map(|_| Scene::new()).collect();
    let entity = Entity::new();
        for &s in &scenes {
        s.bind(entity, &mut src);
    }

    src.next_frame();
    pinput.next_frame();

    let mut map = HashMap::new();
    while let Some(x) = sink.write_into(&mut map) {
        x.wait().unwrap();
    }
    sink.next_frame();

    for s in &scenes {
        assert!(map.get(s).unwrap().contains(&entity));
    }

    entity.delete(&mut pinput);
    src.next_frame();
    pinput.next_frame();

    while let Some(x) = sink.write_into(&mut map) {
        x.wait().unwrap();
    }

    for s in &scenes {
        assert!(map.get(s).is_none());
    }
}

#[test]
fn delete_parent() {
    let mut front = fibe::Frontend::new();
    let (mut pinput, poutput) = parent(&mut front);
    let (mut src, mut sink) = scene(&mut front, poutput);

    let scene = Scene::new();
    let entities: Vec<Entity> = (0..10).map(|_| Entity::new()).collect();
    for &e in &entities {
        scene.bind(e, &mut src);
    }

    src.next_frame();
    pinput.next_frame();

    let mut map = HashMap::new();
    while let Some(x) = sink.write_into(&mut map) {
        x.wait().unwrap();
    }
    sink.next_frame();

    for e in &entities {
        assert!(map.get(&scene).unwrap().contains(e));
    }
    scene.delete(&mut pinput);

    src.next_frame();
    pinput.next_frame();

    while let Some(x) = sink.write_into(&mut map) {
            x.wait().unwrap();
    }

    assert!(map.get(&scene).is_none());
}

