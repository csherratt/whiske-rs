extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate cgmath;
extern crate pulse;
extern crate system;

use std::collections::{HashMap, HashSet};
use entity::*;
use parent::{parent, ParentSystem};
use fibe::*;
use snowstorm::channel::*;
use cgmath::*;

type Message = Operation<Entity, Delta>;

#[derive(Clone)]
pub struct TransformData {
    // All entities that need to be updated 
    deltas: HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
    solved: HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>
}

// Recursively adds all eid + all children of eid to the dirty set
fn mark_dirty(solved: &mut HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
              parent: &ParentSystem,
              eid: Entity) {

    solved.remove(&eid);
    if let Some(children) = parent.parent_to_children.get(&eid).map(|x| x.clone()) {
        for &child in children.iter() {
            mark_dirty(solved, parent, child);
        }
    }   
}

impl TransformData {
    /// Get the world Trasform from the entity
    pub fn local(&self, eid: Entity) -> Option<&Decomposed<f32, Vector3<f32>, Quaternion<f32>>> {
        self.deltas.get(&eid)
    }

    /// Get the world Trasform from the entity
    pub fn world(&self, eid: Entity) -> Option<&Decomposed<f32, Vector3<f32>, Quaternion<f32>>> {
        self.solved.get(&eid)
    }

    fn new() -> TransformData {
        TransformData {
            deltas: HashMap::new(),
            solved: HashMap::new()
        }
    }

    fn apply_ingest(&mut self, parent: &ParentSystem, msg: &[Message]) {
        for op in msg.iter() {
            match op {
                &Operation::Delete(ref eid) => {
                    self.deltas.remove(eid);
                }
                &Operation::Upsert(eid, Delta(pos)) => {
                    self.deltas.insert(eid, pos);
                    mark_dirty(&mut self.solved, parent, eid);
                }
            }
        }
    }

    fn delete(&mut self, msg: &HashSet<Entity>) {
        for eid in msg.iter() {
            self.deltas.remove(eid);
            self.solved.remove(eid);
        }
    }

    fn invalidate(&mut self, parent: &ParentSystem, msg: &HashSet<Entity>) {
        for eid in msg.iter() {
            self.solved.remove(eid);
            if let Some(children) = parent.parent_to_children.get(eid) {
                self.invalidate(parent, children);
            }
        }
    }

    fn update(&mut self, parent: &ParentSystem) {
        fn solve (parent: &ParentSystem,
                  deltas: &HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
                  solved: &mut HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
                  eid: Entity) ->  Decomposed<f32, Vector3<f32>, Quaternion<f32>> {

            if let Some(v) = solved.get(&eid) {
                return *v;
            }

            // check to see if I have a parent
            let parent = if let Some(p) = parent.child_to_parent.get(&eid).map(|x| *x) {
                solve(parent, deltas, solved, p)
            } else {
                Decomposed::identity()
            };

            let mat: Decomposed<f32, Vector3<f32>, Quaternion<f32>> =
                deltas.get(&eid)
                      .map(|x| *x)
                      .expect("Expected delta, but none found")
                      .into();

            let v = parent.concat(&mat);
            solved.insert(eid, v);
            v            
        };

        for (eid, _) in self.deltas.iter() {
            solve(parent, &self.deltas, &mut self.solved, *eid);
        }
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Delta(pub Decomposed<f32, Vector3<f32>, Quaternion<f32>>);
#[derive(Debug, Clone, Copy)]
pub struct Solved(pub Decomposed<f32, Vector3<f32>, Quaternion<f32>>);

impl Solved {
    pub fn to_mat(&self) -> Matrix4<f32> {
        From::from(self.0)
    }
}

// Reads from the parent channel
fn sync_ingest(ingest: &mut system::channel::Receiver<Message>) -> Vec<Message> {
    let mut msgs: Vec<Message> = Vec::new();
    while let Ok(op) = ingest.recv() {
        msgs.push(*op);
    }
    msgs
}

pub fn transform(sched: &mut Schedule, mut parents: ParentSystem) -> TransformSystem {
    let td = TransformData::new();
    let (mut system, handle) = system::System::new(td.clone(), td);

    let mut limsgs = Vec::new();

    task(move |_| {
        loop {
            let p = &mut parents;
            system = system.update(|mut transform, _, mut msgs| {
                let mut deleted = p.deleted.clone();
                let mut invalidate = p.modified.clone();
                p.next_frame();
                for p in p.deleted.iter() { deleted.insert(*p); }
                for p in p.modified.iter() { invalidate.insert(*p); }

                let imsgs = sync_ingest(&mut msgs);

                transform.apply_ingest(p, &limsgs[..]);
                transform.apply_ingest(p, &imsgs[..]);
                transform.delete(&deleted);
                transform.invalidate(p, &invalidate);

                transform.update(p);

                limsgs = imsgs;
                transform
            });
        }
    }).start(sched);

    handle
}

impl entity::WriteEntity<Entity, Delta> for TransformSystem {
    fn write(&mut self, eid: Entity, delta: Delta) {
        self.send(Operation::Upsert(eid, delta));
    }
}

pub type TransformSystem = system::SystemHandle<Message, TransformData>;
