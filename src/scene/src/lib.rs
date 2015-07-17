extern crate entity;
extern crate fibe;
extern crate parent;
extern crate shared_future;
extern crate lease;
extern crate system;

use std::collections::{HashSet, HashMap};
use entity::{Entity, DeleteEntity};
use fibe::{task, Schedule};
use parent::ParentSystem;

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

#[derive(Clone, Debug)]
pub struct SceneData {
    parent_mesages: Vec<parent::Message>,

    // entity is a member of x scenes
    belongs_to: HashMap<Entity, HashSet<Entity>>,

    // entity has x in its scene
    contains: HashMap<Entity, HashSet<Entity>>,
}

// Reads from the parent channel
fn sync_ingest(ingest: &mut system::channel::Receiver<Message>) -> Vec<Message> {
    let mut msgs: Vec<Message> = Vec::new();
    while let Ok(op) = ingest.recv() {
        msgs.push(*op);
    }
    msgs
}

impl SceneData {
    /// Get the entitires for a supplied scene
    pub fn scene_entities(&self, scene: Scene) -> Option<&HashSet<Entity>> {
        self.contains.get(&scene.0)
    }

    fn new() -> SceneData {
        SceneData {
            parent_mesages: Vec::new(),
            belongs_to: HashMap::new(),
            contains: HashMap::new(),
        }
    }

    fn delete(&mut self, msgs: &HashSet<Entity>) {
        for eid in msgs.iter() {
            // A scene is deleted, we need to tell the downstream
            // as a series of unbinds
            if let Some(children) = self.contains.remove(&eid) {
                for cid in children.into_iter() {
                    if let Some(belongs) = self.belongs_to.get_mut(&cid) {
                        belongs.remove(&eid);
                    }
                }                        
            }

            // remove all the bindings that the child may have been in
            if let Some(parents) = self.belongs_to.remove(&eid) {
                for pid in parents.into_iter() {
                    if let Some(contains) = self.contains.get_mut(&pid) {
                        contains.remove(&eid);
                    }
                }     
            }
        }
    }

    /// Read from the ingest channel
    fn apply_ingest(&mut self, msgs: &[Message]) {
        for op in msgs.iter() {
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
    }
}

/// A `Scene` is an entity that is used to manage
impl Scene {
    /// Create a new Scene
    pub fn new() -> Scene { Scene(Entity::new()) }

    /// Read the internal entity
    pub fn as_entity(&self) -> Entity { self.0 }

    /// Bind a entity to the scene, write this operation to SceneSystem
    pub fn bind(&self, child: Entity, src: &mut SceneSystem) {
        src.send(Message::Bind(*self, child))
    }

    /// Unbind a entity to the scene, write this operation to SceneSystem
    pub fn unbind(&self, child: Entity, src: &mut SceneSystem) {
        src.send(Message::Unbind(*self, child))
    }

    /// Delete this entity from a device
    pub fn delete<D>(&self, delete: &mut D) where D: DeleteEntity<Entity> {
        delete.delete(self.0);
    }
}

impl entity::WriteEntity<Entity, Scene> for SceneSystem {
    fn write(&mut self, eid: Entity, scene: Scene) {
        scene.bind(eid, self);
    }
}

impl entity::WriteEntity<Scene, Entity> for SceneSystem {
    fn write(&mut self, scene: Scene, eid: Entity) {
        scene.bind(eid, self);
    }
}

/// Creates a new scene system. The scene system manages a relationship
/// between Scene objects and entities. A Scene may contain 1 or more
/// objects. An object may exist in more then one Scene.
///
/// The Scene system will run in the supplied scheduler until the
/// input channels are closed.
///
/// This will supply a SceneSystem for communication
/// into and out of the system.
pub fn scene(sched: &mut Schedule, mut parents: ParentSystem) -> SceneSystem {
    let sd = SceneData::new();
    let (mut system, handle) = system::System::new(sd.clone(), sd);

    let mut limsgs = Vec::new();

    task(move |_| {
        loop {
            let p = &mut parents;
            system = system.update(|mut scene, _, mut msgs| {
                let mut deleted = p.deleted.clone();
                p.next_frame();
                for &d in p.deleted.iter() { deleted.insert(d); }
                let imsgs = sync_ingest(&mut msgs);

                scene.apply_ingest(&limsgs[..]);
                scene.apply_ingest(&imsgs[..]);
                scene.delete(&deleted);

                limsgs = imsgs;
                scene
            });
        }
    }).start(sched);

    handle
}

pub type SceneSystem = system::SystemHandle<Message, SceneData>;