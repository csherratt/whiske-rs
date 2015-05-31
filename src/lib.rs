extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate pulse;

use std::collections::{HashMap, HashSet};
use fibe::*;
use snowstorm::channel::*;
use entity::{Entity, WriteEntity, Operation};
use pulse::{Signals, Signal};

pub type Message = Operation<Entity, Parent>;

struct ParentSystem {
    input: Receiver<Message>,
    output: Sender<Message>,

    /// Lookup table to find the parent from the child eid
    child_to_parent: HashMap<Entity, Entity>,

    /// lookup table to find the children from the parent's eid
    parent_to_children: HashMap<Entity, HashSet<Entity>>
}

impl ParentSystem {
    /// This creates a binding between the parent and the child
    fn bind(&mut self, parent: Entity, child: Entity) {
        self.child_to_parent.insert(child, parent);

        let mut inserted = false;
        self.parent_to_children
            .entry(parent)
            .or_insert_with(|| {
                inserted = true;
                HashSet::new()
             })
            .insert(child);

        if inserted {
            if self.child_to_parent.get(&parent).is_none() {
                self.output.send(Operation::Upsert(parent, Parent::Root));
            }
        }
        self.output.send(Operation::Upsert(child, Parent::Child(parent)));
    }

    /// Recessively delete the children of a parent
    fn delete(&mut self, eid: Entity) {
        if let Some(children) = self.parent_to_children.remove(&eid) {
            for child in children {
                self.delete(child);
            }
        }
        if let Some(_) = self.child_to_parent.remove(&eid) {
            if let Some(p2c) = self.parent_to_children.get_mut(&eid) {
                p2c.remove(&eid);
            }
        }
        self.output.send(Operation::Delete(eid));
    }

    fn write(&mut self, op: Operation<Entity, Parent>) {
        match op {
            Operation::Delete(eid) => self.delete(eid),
            Operation::Upsert(eid, Parent::Root) => {
                self.parent_to_children.insert(eid, HashSet::new());
                self.output.send(Operation::Upsert(eid, Parent::Root));
            }
            Operation::Upsert(eid, Parent::Child(parent)) => {
                self.bind(parent, eid);
            }
        }
    }
}

impl ResumableTask for ParentSystem {
    fn resume(&mut self, _: &mut Schedule) -> WaitState {
        while let Some(&msg) = self.input.try_recv() {
            self.write(msg.clone());
        }
        if self.input.closed() {
            // The channel is closed and there is no next frame
            // which means there are no more Senders, and we should
            // exit
            if !self.input.next_frame() {
                return WaitState::Completed;
            } else {
                // signal the next stage that we are done
                self.output.next_frame();
            }
        }
        
        // there is still more data to process
        WaitState::Pending(self.input.signal())
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Parent {
    /// Insert this node without a parent
    Root,
    /// Insert this node as a child of a supplied parent
    Child(Entity)
}

/// The `parent` system takes and input of parent child bindings
pub fn parent(sched: &mut Schedule) -> (ParentInput, ParentOutput) {
    let (tx_ingest, rx_ingest) = channel();
    let (tx_engest, rx_engest) = channel();

    let signal = rx_ingest.signal();

    ParentSystem {
        input: rx_ingest,
        output: tx_engest,
        child_to_parent: HashMap::new(),
        parent_to_children: HashMap::new()
    }.after(signal).start(sched);
    
    (ParentInput(tx_ingest), ParentOutput(rx_engest))
}

#[derive(Clone)]
pub struct ParentInput(Sender<Message>);

impl ParentInput {
    /// Migrate to the next frame
    pub fn next_frame(&mut self) {
        self.0.next_frame()
    }
}

impl entity::WriteEntity<Entity, Parent> for ParentInput {
    fn write(&mut self, eid: Entity, value: Parent) {
        self.0.write(eid, value);
    }
}

impl entity::DeleteEntity<Entity> for ParentInput {
    fn delete(&mut self, eid: Entity) {
        self.0.delete(eid);
    }
}

#[derive(Clone)]
pub struct ParentOutput(Receiver<Message>);

impl ParentOutput {
    /// Migrate to the next frame
    pub fn next_frame(&mut self) -> bool {
        self.0.next_frame()
    }

    /// Check if there is data from the future
    pub fn closed(&mut self) -> bool {
        self.0.closed()
    }

    ///
    pub fn recv(&mut self) -> Result<&Message, snowstorm::channel::ReceiverError> {
        self.0.recv()
    }

    ///
    pub fn try_recv(&mut self) -> Option<&Message> {
        self.0.try_recv()
    }
}

impl pulse::Signals for ParentOutput {
    fn signal(&self) -> Signal {
        self.0.signal()
    }
}