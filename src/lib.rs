extern crate entity;
extern crate snowstorm;
extern crate fibe;
extern crate parent;
extern crate pulse;

use std::collections::{HashSet, HashMap};
use snowstorm::channel::{Sender, Receiver};
use entity::{Entity, Operation};
use fibe::{Schedule, ResumableTask, WaitState, IntoTask};
use pulse::{Signal, Signals, SelectMap};
use parent::Parent;

/// This holds an abstract of a scene
///     A scene may have 0-N children. The children are `bound` to it.
///     An entity may live in more then one scene.
///
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq)]
pub struct Scene(pub Entity);

#[derive(Copy, Clone, Debug)]
pub enum Message {
    /// Child added to the parents scene
    Bind(Scene, Entity),
    /// Child was removed form a scene
    Unbind(Scene, Entity),
}

#[derive(Clone)]
pub struct SceneSource(pub Sender<Message>);

impl SceneSource {
    pub fn next_frame(&mut self) {
        self.0.next_frame();
    }     
} 

#[derive(Clone)]
pub struct SceneSink(pub Receiver<Message>);

impl SceneSink {
    pub fn next_frame(&mut self) -> bool {
        self.0.next_frame()
    }

    pub fn try_recv(&mut self) -> Option<Message> {
        self.0.try_recv().map(|x| *x)
    }

    pub fn write_into(&mut self, scenes: &mut HashMap<Scene, HashSet<Entity>>) -> Option<Signal> {
        while let Some(op) = self.try_recv() {
            match op {
                Message::Bind(scene, eid) => {
                    scenes.entry(scene)
                          .or_insert_with(HashSet::new)
                          .insert(eid);
                }
                Message::Unbind(scene, eid) => {
                    let len = scenes.get_mut(&scene)
                          .map(|h| {
                            h.remove(&eid);
                            h.len()
                           });

                    if len == Some(0) {
                        scenes.remove(&scene);
                    }
                }
            }
        }

        if self.0.closed() {
            None
        } else {
            Some(self.0.signal())
        }
    }
} 


struct SceneSystem {
    // input
    parents: Receiver<parent::Message>,
    ingest: Receiver<Message>,
    select: SelectMap<fn(&mut SceneSystem) -> Option<Signal>>,

    // output
    output: Sender<Message>,

    // entity is a member of x scenes
    belongs_to: HashMap<Entity, HashSet<Entity>>,

    // entity has x in its scene
    contains: HashMap<Entity, HashSet<Entity>>,

    // lookup table to find the children from the parent's eid
    parent_to_children: HashMap<Entity, HashSet<Entity>>
}

impl SceneSystem {
    // Reads from the parent channel
    fn sync_parent(&mut self) -> Option<Signal> {
        while let Some(op) = self.parents.try_recv() {
            match op {
                &Operation::Upsert(ref parent, Parent::Child(child)) => {
                    self.parent_to_children
                        .get_mut(parent)
                        .unwrap()
                        .insert(child);
                }
                &Operation::Upsert(parent, Parent::Root) => {
                    self.parent_to_children
                        .insert(parent, HashSet::new());
                }
                &Operation::Delete(eid) => {
                    // A scene is deleted, we need to tell the downstream
                    // as a series of unbinds
                    if let Some(children) = self.contains.remove(&eid) {
                        for cid in children.into_iter() {
                            self.output.send(Message::Unbind(Scene(eid), cid))
                        }                        
                    }

                    // remove all the bindings that the child may have been in
                    if let Some(parents) = self.belongs_to.remove(&eid) {
                        for pid in parents.into_iter() {
                            self.output.send(Message::Unbind(Scene(pid), eid))
                        }     
                    }

                    self.parent_to_children.remove(&eid);
                }
            }
        }

        if self.parents.closed() {
            None
        } else {
            Some(self.parents.signal())
        }
    }

    /// Read from the ingest channel
    fn sync_ingest(&mut self) -> Option<Signal> {
        while let Some(op) = self.ingest.try_recv() {
            self.output.send(*op);
            match op {
                &Message::Bind(Scene(scene), eid) => {
                    self.contains
                        .entry(scene)
                        .or_insert_with(HashSet::new)
                        .insert(eid);
                    self.belongs_to
                        .entry(eid)
                        .or_insert_with(HashSet::new)
                        .insert(scene);
                }
                &Message::Unbind(Scene(scene), eid) => {
                    let len = self.contains
                        .get_mut(&scene)
                        .map(|c| {
                            c.remove(&eid);
                            c.len()
                        });
                    if let Some(len) = len {
                        if len == 0 {
                            self.contains.remove(&scene);
                        }                        
                    }
                    let len = self.belongs_to
                        .get_mut(&eid)
                        .map(|c| {
                            c.remove(&scene);
                            c.len()
                        });
                    if let Some(len) = len {
                        if len == 0 {
                            self.belongs_to.remove(&eid);
                        }                        
                    }
                }
            }
        }

        if self.ingest.closed() {
            None
        } else {
            Some(self.ingest.signal())
        }
    }
}

impl ResumableTask for SceneSystem {
    fn resume(&mut self, _: &mut Schedule) -> WaitState {
        while let Some((_, cb)) = self.select.try_next() {
            if let Some(sig) = cb(self) {
                self.select.add(sig, cb);
            }
        }

        if self.select.len() == 0 {
            if !self.parents.next_frame() || !self.ingest.next_frame() {
                return WaitState::Completed;
            } else {
                self.output.next_frame();
            }
        }
        
        WaitState::Pending(self.select.signal())
    }
}

pub fn scene(sched: &mut Schedule, parents: Receiver<parent::Message>) -> (SceneSource, SceneSink) {
    let (src_tx, src_rx) = snowstorm::channel::channel();
    let mut select: SelectMap<fn(&mut SceneSystem) -> Option<Signal>> = SelectMap::new();
    select.add(parents.signal(), SceneSystem::sync_parent);
    select.add(src_rx.signal(), SceneSystem::sync_ingest);
    let signal = select.signal();

    let (sink_tx, sink_rx) = snowstorm::channel::channel();


    Box::new(SceneSystem {
        parents: parents,
        ingest: src_rx,
        select: select,
        output: sink_tx,
        belongs_to: HashMap::new(),
        contains: HashMap::new(),
        parent_to_children: HashMap::new(),
    }).after(signal).start(sched);

    (SceneSource(src_tx), SceneSink(sink_rx))
}

impl Scene {
    pub fn new() -> Scene { Scene(Entity::new()) }

    pub fn bind(&self, child: Entity, src: &mut SceneSource) {
        src.0.send(Message::Bind(*self, child))
    }

    pub fn unbind(&self, child: Entity, src: &mut SceneSource) {
        src.0.send(Message::Bind(*self, child))
    }
}

