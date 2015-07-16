extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate cgmath;
extern crate pulse;
extern crate system;

use std::collections::{HashMap, HashSet};
use entity::*;
use parent::{parent, Parent, ParentOutput};
use fibe::*;
use snowstorm::channel::*;
use cgmath::*;

type Message = Operation<Entity, Delta>;

#[derive(Clone)]
pub struct TransformData {
    // child mappings
    child_to_parent: HashMap<Entity, Entity>,
    parent_to_child: HashMap<Entity, HashSet<Entity>>,

    // All entities that need to be updated 
    deltas: HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
    solved: HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>
}

// Recursively adds all eid + all children of eid to the dirty set
fn mark_dirty(solved: &mut HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
              parent_to_child: &HashMap<Entity, HashSet<Entity>>,
              eid: Entity) {

    solved.remove(&eid);
    if let Some(children) = parent_to_child.get(&eid).map(|x| x.clone()) {
        for &child in children.iter() {
            mark_dirty(solved, parent_to_child, child);
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
            child_to_parent: HashMap::new(),
            parent_to_child: HashMap::new(),
            deltas: HashMap::new(),
            solved: HashMap::new()
        }
    }

    fn apply_ingest(&mut self, msg: &[Message]) {
        for op in msg.iter() {
            match op {
                &Operation::Delete(ref eid) => {
                    self.deltas.remove(eid);
                }
                &Operation::Upsert(eid, Delta(pos)) => {
                    self.deltas.insert(eid, pos);
                    mark_dirty(&mut self.solved, &self.parent_to_child, eid);
                }
            }
        }
    }

    fn apply_parent(&mut self, msg: &[parent::Message]) {
        for op in msg.iter() {
            match op {
                &Operation::Delete(ref eid) => {
                    self.deltas.remove(eid);
                    self.solved.remove(eid);
                    self.child_to_parent.remove(eid);
                    self.parent_to_child.remove(eid);
                }
                &Operation::Upsert(eid, Parent::Child(parent)) => {
                    self.child_to_parent.insert(eid, parent);
                    self.parent_to_child
                        .entry(parent)
                        .or_insert_with(|| HashSet::new())
                        .insert(eid);
                    self.solved.remove(&eid);
                }
                &Operation::Upsert(_, Parent::Root) => ()
            }
        }
    }

    fn update(&mut self) {
        fn solve (c2p: &HashMap<Entity, Entity>,
                  deltas: &HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
                  solved: &mut HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
                  eid: Entity) ->  Decomposed<f32, Vector3<f32>, Quaternion<f32>> {

            if let Some(v) = solved.get(&eid) {
                return *v;
            }

            // check to see if I have a parent
            let parent = if let Some(parent) = c2p.get(&eid).map(|x| *x) {
                solve(c2p, deltas, solved, parent)
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
            solve(&self.child_to_parent, &self.deltas, &mut self.solved, *eid);
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
fn sync_parent(parents: &mut ParentOutput) -> Vec<parent::Message> {
    let mut msgs: Vec<parent::Message> = Vec::new();
    while let Ok(op) = parents.recv() {
        msgs.push(*op);
    }
    msgs
}

// Reads from the parent channel
fn sync_ingest(ingest: &mut system::channel::Receiver<Message>) -> Vec<Message> {
    let mut msgs: Vec<Message> = Vec::new();
    while let Ok(op) = ingest.recv() {
        msgs.push(*op);
    }
    msgs
}

pub fn transform(sched: &mut Schedule, mut parents: ParentOutput) -> TransformSystem {
    let td = TransformData::new();
    let (mut system, handle) = system::System::new(td.clone(), td);

    let mut lpmsgs = Vec::new();
    let mut limsgs = Vec::new();

    task(move |_| {
        loop {
            let p = &mut parents;
            system = system.update(|mut transform, _, mut msgs| {
                let pmsgs = sync_parent(p);
                p.next_frame();
                let imsgs = sync_ingest(&mut msgs);

                transform.apply_parent(&lpmsgs[..]);
                transform.apply_ingest(&limsgs[..]);
                transform.apply_parent(&pmsgs[..]);
                transform.apply_ingest(&imsgs[..]);

                transform.update();

                lpmsgs = pmsgs;
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
