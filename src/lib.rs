extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate cgmath;
extern crate pulse;

use std::collections::{HashMap, HashSet};
use pulse::{SelectMap, Signals, Signal};
use entity::*;
use parent::{parent, Parent};
use fibe::*;
use snowstorm::channel::*;
use cgmath::*;

struct PositionSystem {
    // Inputs
    delta: Receiver<Operation<Entity, Delta>>,
    parent: Receiver<parent::Message>,

    // input select
    select: SelectMap<fn (&mut PositionSystem) -> Option<Signal>>,

    // output
    output: Sender<Operation<Entity, Solved>>,

    // child mappings
    child_to_parent: HashMap<Entity, Entity>,
    parent_to_child: HashMap<Entity, HashSet<Entity>>,

    // All entities that need to be updated 
    dirty: HashSet<Entity>,

    deltas: HashMap<Entity, Decomposed<f32, Vector3<f32>, Quaternion<f32>>>,
}

// Recursively adds all eid + all children of eid to the dirty set
fn mark_dirty(dirty: &mut HashSet<Entity>,
              parent_to_child: &HashMap<Entity, HashSet<Entity>>,
              eid: Entity) {

    dirty.insert(eid);
    if let Some(children) = parent_to_child.get(&eid).map(|x| x.clone()) {
        for &child in children.iter() {
            mark_dirty(dirty, parent_to_child, child);
        }
    }   
}

impl PositionSystem {
    fn handle_delta(&mut self) -> Option<Signal> {
        while let Some(op) = self.delta.try_recv().map(|x| x.clone()) {
            match op {
                Operation::Delete(ref eid) => {
                    self.deltas.remove(eid);
                }
                Operation::Upsert(eid, Delta(pos)) => {
                    self.deltas.insert(eid, pos);
                    mark_dirty(&mut self.dirty, &self.parent_to_child, eid);
                }
            }
        }

        if self.delta.closed() {
            None
        } else {
            Some(self.delta.signal())
        }
    }

    fn handle_parent(&mut self) -> Option<Signal> {
        while let Some(op) = self.parent.try_recv() {
            match op {
                &Operation::Delete(ref eid) => {
                    let found = self.deltas.remove(eid);
                    self.dirty.remove(eid);
                    self.child_to_parent.remove(eid);
                    self.parent_to_child.remove(eid);
                    if found.is_some() {
                        eid.delete(&mut self.output);
                    }
                }
                &Operation::Upsert(eid, Parent::Child(parent)) => {
                    self.child_to_parent.insert(eid, parent);
                    self.parent_to_child
                        .entry(parent)
                        .or_insert_with(|| HashSet::new())
                        .insert(eid);
                    self.dirty.insert(eid);
                }
                &Operation::Upsert(_, Parent::Root) => ()
            }
        }

        if self.parent.closed() {
            None
        } else {
            Some(self.delta.signal())
        }
    }

    fn solve(&self, eid: Entity) -> Matrix4<f32> {
        // check to see if I have a parent
        let parent = self.child_to_parent.get(&eid)
            .map(|&parent| self.solve(parent))
            .unwrap_or_else(|| Matrix4::identity());

        let mat: Matrix4<f32> =
            self.deltas.get(&eid)
                       .map(|x| *x)
                       .expect("Expected delta, but none found")
                       .into();

        parent.mul_m(&mat)
    }


    fn update(&mut self) {
        for dirty in self.dirty.iter() {
            let solved = self.solve(*dirty);
            dirty.bind(Solved(solved)).write(&mut self.output);
        }
        self.dirty.clear();
    }
}


#[derive(Debug, Clone, Copy)]
pub struct Delta(pub Decomposed<f32, Vector3<f32>, Quaternion<f32>>);
#[derive(Debug, Clone, Copy)]
pub struct Solved(pub Matrix4<f32>);

pub fn position(sched: &mut Schedule,
                delta: Receiver<Operation<Entity, Delta>>,
                parent: Receiver<parent::Message>) -> Receiver<Operation<Entity, Solved>> {

    let (tx, output) = channel();

    let mut select: SelectMap<fn (&mut PositionSystem) -> Option<Signal>> = SelectMap::new();
    select.add(delta.signal(), PositionSystem::handle_delta);
    select.add(parent.signal(), PositionSystem::handle_parent);
    let signal = select.signal();

    PositionSystem {
        delta: delta,
        parent: parent,
        select: select,
        output: tx,
        child_to_parent: HashMap::new(),
        parent_to_child: HashMap::new(),
        dirty: HashSet::new(),
        deltas: HashMap::new()
    }.after(signal).start(sched);

    output
}

impl ResumableTask for PositionSystem {
    fn resume(&mut self, _: &mut Schedule) -> WaitState {
        if let Some((_, cb)) = self.select.try_next() {
            if let Some(sig) = cb(self) {
                self.select.add(sig, cb);
            }
        }

        if self.select.len() == 0 {
            self.update();
            if !(self.delta.next_frame() && self.parent.next_frame()) {
                return WaitState::Completed;
            }
            self.output.next_frame();

            self.select.add(self.delta.signal(), PositionSystem::handle_delta);
            self.select.add(self.parent.signal(), PositionSystem::handle_parent);
        }

        // there is still more data to process
        WaitState::Pending(self.select.signal())
    }
}