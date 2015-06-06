extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate cgmath;
extern crate pulse;

use std::collections::{HashMap, HashSet};
use pulse::{SelectMap, Signals, Signal};
use entity::*;
use parent::{parent, Parent, ParentOutput};
use fibe::*;
use snowstorm::channel::*;
use cgmath::*;

struct TransformSystem {
    // Inputs
    delta: Receiver<Operation<Entity, Delta>>,
    parent: ParentOutput,

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

impl TransformSystem {
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

    fn solve(&self, eid: Entity) -> Decomposed<f32, Vector3<f32>, Quaternion<f32>> {
        // check to see if I have a parent
        let parent = self.child_to_parent.get(&eid)
            .map(|&parent| self.solve(parent))
            .unwrap_or_else(|| Decomposed::identity());

        let mat: Decomposed<f32, Vector3<f32>, Quaternion<f32>> =
            self.deltas.get(&eid)
                       .map(|x| *x)
                       .expect("Expected delta, but none found")
                       .into();

        parent.concat(&mat)
    }

    fn update(&mut self) {
        for dirty in self.dirty.iter() {
            let solved = self.solve(*dirty);
            dirty.bind(Solved(solved)).write(&mut self.output);
        }
        self.dirty.clear();
    }

    fn run(&mut self) {
        let mut select: SelectMap<fn(&mut TransformSystem) -> Option<Signal>> = SelectMap::new();

        loop {
            select.add(self.delta.signal(), TransformSystem::handle_delta);
            select.add(self.parent.signal(), TransformSystem::handle_parent);

            while let Some((_, cb)) = select.next() {
                if let Some(sig) = cb(self) {
                    select.add(sig, cb);
                }
            }

            self.update();

            if self.delta.next_frame() && self.parent.next_frame() {
                self.output.next_frame();
            } else {
                return;
            }
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

pub fn transform(sched: &mut Schedule,
                 parent: ParentOutput) -> (TransformInput, TransformOutput) {

    let (tx, output) = channel();
    let (delta_input, delta) = channel();

    let mut system = TransformSystem {
        delta: delta,
        parent: parent,
        output: tx,
        child_to_parent: HashMap::new(),
        parent_to_child: HashMap::new(),
        dirty: HashSet::new(),
        deltas: HashMap::new()
    };
    task(move |_| system.run()).start(sched);

    (TransformInput(delta_input), TransformOutput(output))
}

/// A channel to send infromation to the Transform System
#[derive(Clone)]
pub struct TransformInput(Sender<Operation<Entity, Delta>>);

impl TransformInput {
    pub fn next_frame(&mut self) {
        self.0.next_frame()
    }
}

impl entity::WriteEntity<Entity, Delta> for TransformInput {
    fn write(&mut self, eid: Entity, delta: Delta) {
        self.0.write(eid, delta);
    }
}

#[derive(Clone)]
pub struct TransformOutput(Receiver<Operation<Entity, Solved>>);

impl TransformOutput {
    pub fn recv(&mut self) -> Result<&entity::Operation<entity::Entity, Solved>, snowstorm::channel::ReceiverError> {
        self.0.recv()
    }

    pub fn try_recv(&mut self) -> Option<&entity::Operation<entity::Entity, Solved>> {
        self.0.try_recv()
    }

    pub fn closed(&mut self) -> bool {
        self.0.closed()
    }

    pub fn next_frame(&mut self) -> bool {
        self.0.next_frame()
    }

    pub fn copy_iter<'a>(&'a mut self, block: bool) -> CopyIter<'a, entity::Operation<entity::Entity, Solved>> {
        self.0.copy_iter(block)
    }
}

impl Signals for TransformOutput {
    fn signal(&self) -> Signal {
        self.0.signal()
    }
}