extern crate parent;
extern crate scene;
extern crate entity;
extern crate snowstorm;
extern crate fibe;

use std::collections::HashMap;
use parent::parent;
use scene::{scene, Scene};
use entity::Entity;
use snowstorm::channel::channel;

#[test]
fn pass_through() {
    let (mut parent_tx, parent_rx) = channel();
    let mut front = fibe::Frontend::new();
    let parent_rx = parent(&mut front, parent_rx);
    let (mut src, mut sink) = scene(&mut front, parent_rx);

    let scene = Scene::new();
    let entities: Vec<Entity> = (0..10).map(|_| Entity::new()).collect();
    for &e in &entities {
        scene.bind(e, &mut src);
    }

    src.next_frame();
    parent_tx.next_frame();

    let mut map = HashMap::new();
    while let Some(x) = sink.write_into(&mut map) {
        x.wait().unwrap();
    }

    for e in &entities {
        assert!(map.get(&scene).unwrap().contains(e));
        scene.unbind(*e, &mut src);
    }

    src.next_frame();
    parent_tx.next_frame();

    let mut map = HashMap::new();
    while let Some(x) = sink.write_into(&mut map) {
        x.wait().unwrap();
    }

    assert!(map.get(&scene).is_none());
}
