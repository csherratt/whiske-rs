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

/// This holds an abstract of a scene
///     A scene may have 0-N children. The children are `bound` to it.
///     An entity may live in more then one scene.
///
#[derive(Copy, Clone)]
pub struct Scene(pub Entity);

pub type Message = Operation<Entity, Scene>;
pub struct SceneSource(Sender<Message>);
pub struct SceneSink(Receiver<Message>);

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
    contains: HashMap<Entity, HashSet<Entity>>
}

impl SceneSystem {
    // Reads from the parent 
    fn sync_parent(&mut self) -> Option<Signal> {
        None
    }

    fn sync_ingest(&mut self) -> Option<Signal> {
        None
    }
}

impl ResumableTask for SceneSystem {
    fn resume(&mut self, sched: &mut Schedule) -> WaitState {
        while let Some((_, cb)) = self.select.try_next() {
            if let Some(sig) = cb(self) {
                self.select.add(sig, cb);
            }
        }

        // Do update
        if self.select.len() == 0 {
            // do work

            if !self.parents.next_frame() || !self.ingest.next_frame()) {
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
        contains: HashMap::new()
    }).after(signal).start(sched);

    (SceneSource(src_tx), SceneSink(sink_rx))
}

impl Scene {
    pub fn new() -> Scene { Scene(Entity::new()) }
}

