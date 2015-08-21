extern crate parent;
extern crate engine;
extern crate snowstorm;
extern crate entity;
extern crate cgmath;
extern crate pulse;
extern crate system;
extern crate ordered_vec;

use std::collections::HashMap;
use entity::*;
use parent::{parent, ParentSystem, Parent};
use engine::fibe::*;
use snowstorm::channel::*;
use cgmath::*;
use ordered_vec::OrderedVec;

type Message = Operation<Entity, Local>;

#[derive(Clone, Debug)]
struct TransformEntry {
    dirty: bool,
    local: Local,
    world: World
}


#[derive(Clone)]
pub struct TransformData {
    // All entities that need to be updated 
    entries: OrderedVec<Entity, TransformEntry>,
}

// Recursively adds all eid + all children of eid to the dirty set
fn mark_dirty(solved: &mut OrderedVec<Entity, TransformEntry>,
              parent: &ParentSystem,
              eid: Entity) {

    solved.get_mut(&eid).map(|e| { e.dirty = true; });
    if let Some(children) = parent.parent_to_children.get(&eid).map(|x| x.clone()) {
        for &child in children.iter() {
            mark_dirty(solved, parent, child);
        }
    }   
}

impl TransformData {
    /// Get the world Trasform from the entity
    pub fn local(&self, eid: Entity) -> Option<&Local> {
        self.entries.get(&eid).map(|e| &e.local)
    }

    /// Get the world Trasform from the entity
    pub fn world(&self, eid: Entity) -> Option<&World> {
        self.entries.get(&eid).map(|e| &e.world)
    }

    fn new() -> TransformData {
        TransformData {
            entries: OrderedVec::new()
        }
    }

    fn apply_ingest(&mut self, old: &TransformData, parent: &ParentSystem, msg: &[Operation<Entity, TransformEntry>]) {
        let mut invalidate = HashMap::new();
        self.entries.clone_from_and_update(&old.entries, msg.iter().map(|x| x.clone()));
        for m in msg {
            invalidate.insert(*m.key(), None);
        }
        self.invalidate(parent, &invalidate);
    }

    fn invalidate(&mut self, parent: &ParentSystem, msg: &HashMap<Entity, Option<Parent>>) {
        for &eid in msg.keys() {
            mark_dirty(&mut self.entries, parent, eid);
        }
    }

    fn update(&mut self, parent: &ParentSystem) {
        fn solve (parent: &ParentSystem,
                  entries: &mut OrderedVec<Entity, TransformEntry>,
                  eid: Entity) ->  Decomposed<f32, Vector3<f32>, Quaternion<f32>> {

            if let Some(v) = entries.get(&eid) {
                if !v.dirty {
                    return v.world.0;
                }
            }

            // check to see if I have a parent
            let parent = if let Some(Parent::Child(p)) = parent.child_to_parent.get(&eid).map(|x| *x) {
                solve(parent, entries, p)
            } else {
                Decomposed::identity()
            };

            let v = entries.get_mut(&eid).unwrap();
            v.world.0 = parent.concat(&v.local.0);
            v.dirty = false;
            v.world.0
        };

        let keys: Vec<Entity> = self.entries.iter()
            .filter(|&(_, v)| v.dirty)
            .map(|(k, _)| *k)
            .collect();

        for eid in keys {
            solve(parent, &mut self.entries, eid);
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Local(pub Decomposed<f32, Vector3<f32>, Quaternion<f32>>);

#[derive(Debug, Clone, Copy)]
pub struct World(pub Decomposed<f32, Vector3<f32>, Quaternion<f32>>);

impl World {
    pub fn to_mat(&self) -> Matrix4<f32> {
        From::from(self.0)
    }
}

// Reads from the parent channel
fn sync_ingest(ingest: &mut system::channel::Receiver<Message>) -> Vec<Operation<Entity, TransformEntry>> {
    let mut msgs: Vec<Operation<Entity, TransformEntry>> = Vec::new();
    while let Ok(op) = ingest.recv() {
        msgs.push(match op {
            &Operation::Upsert(eid, local) => {
                Operation::Upsert(eid, TransformEntry{
                    dirty: true,
                    local: local,
                    world: World(Decomposed::identity())
                })
            }
            &Operation::Delete(eid) => Operation::Delete(eid)
        });
    }
    msgs
}

pub fn transform(sched: &mut Schedule, parents: ParentSystem) -> TransformSystem {
    let td = TransformData::new();
    let (mut system, handle) = system::System::new(td.clone(), td);

    task(move |_| {
        let mut parents = Some(parents);
        loop {
            let s = system.update(|mut transform, old, mut msgs| {
                let mut p = parents.take().unwrap().next_frame().get().unwrap();

                let mut imsgs = sync_ingest(&mut msgs);
                for &d in p.deleted.keys() {
                    imsgs.push(Operation::Delete(d));
                }
                imsgs.sort_by(|a, b| a.key().cmp(b.key()));

                transform.apply_ingest(&old, &p, &imsgs[..]);
                transform.invalidate(&p, &p.modified);
                transform.update(&mut p);

                parents = Some(p);
                transform
            });
            system = if let Some(s) = s { s } else { return; };
        }
    }).start(sched);

    handle
}

impl entity::WriteEntity<Entity, Local> for TransformSystem {
    fn write(&mut self, eid: Entity, delta: Local) {
        self.send(Operation::Upsert(eid, delta));
    }
}

impl entity::ReadEntity<Entity, Local> for TransformSystem {
    fn read(&self, eid: &Entity) -> Option<&Local> {
        self.entries.get(eid).map(|x| &x.local)
    }
}

impl entity::ReadEntity<Entity, World> for TransformSystem {
    fn read(&self, eid: &Entity) -> Option<&World> {
        self.entries.get(eid).map(|x| &x.world)
    }
}


pub type TransformSystem = system::SystemHandle<Message, TransformData>;
