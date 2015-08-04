extern crate engine;
extern crate entity;
extern crate system;

use std::collections::{HashMap, HashSet};
use engine::fibe::*;
use entity::{Entity, WriteEntity, Operation};

pub type Message = Operation<Entity, Parent>;

#[derive(Clone)]
pub struct ParentData {
    /// Lookup table to find the parent from the child eid
    pub child_to_parent: HashMap<Entity, Parent>,

    /// lookup table to find the children from the parent's eid
    pub parent_to_children: HashMap<Entity, HashSet<Entity>>,

    // Set of deleted entities from the last updated
    pub deleted: HashMap<Entity, Option<Parent>>,

    // Entities that's parent was changed during the last update
    pub modified: HashMap<Entity, Option<Parent>>
}

impl ParentData {
    fn new() -> ParentData {
        ParentData {
            child_to_parent: HashMap::new(),
            parent_to_children: HashMap::new(),
            deleted: HashMap::new(),
            modified: HashMap::new()
        }
    }

    /// This creates a binding between the parent and the child
    fn bind(&mut self, parent: Entity, child: Entity) {
        let old = self.child_to_parent.insert(child, Parent::Child(parent));
        self.parent_to_children
            .entry(parent)
            .or_insert_with(HashSet::new)
            .insert(child);
        self.modified.insert(child, old);
    }

    /// Recessively delete the children of a parent
    fn delete(&mut self, eid: Entity) {
        if let Some(children) = self.parent_to_children.remove(&eid) {
            for child in children {
                self.delete(child);
            }
        }
        let old = self.child_to_parent.remove(&eid);
        if let Some(_) = old {
            if let Some(p2c) = self.parent_to_children.get_mut(&eid) {
                p2c.remove(&eid);
            }
        }
        self.deleted.insert(eid, old);
    }

    fn write(&mut self, op: Operation<Entity, Parent>) {
        match op {
            Operation::Delete(eid) => self.delete(eid),
            Operation::Upsert(eid, Parent::Root) => {
                self.parent_to_children.insert(eid, HashSet::new());
                let old = self.child_to_parent.insert(eid, Parent::Root);
                self.modified.insert(eid, old);
            }
            Operation::Upsert(eid, Parent::Child(parent)) => {
                self.bind(parent, eid);
            }
        }
    }

    fn apply_parent(&mut self, msgs: &[Message]) {
        self.deleted.clear();
        self.modified.clear();

        for &m in msgs {
            self.write(m);
        }
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

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum Parent {
    /// Insert this node without a parent
    Root,
    /// Insert this node as a child of a supplied parent
    Child(Entity)
}

/// The `parent` system takes and input of parent child bindings
pub fn parent(sched: &mut Schedule) -> ParentSystem {
    let pd = ParentData::new();
    let (mut system, handle) = system::System::new(pd.clone(), pd);

    let mut lpmsgs = Vec::new();

    task(move |_| {
        loop {
            let s = system.update(|mut parent, _, mut msgs| {
                let pmsgs = sync_ingest(&mut msgs);

                parent.apply_parent(&lpmsgs[..]);
                parent.apply_parent(&pmsgs[..]);

                lpmsgs = pmsgs;
                parent
            });
            system = if let Some(s) = s { s } else { return; };
        }
    }).start(sched);

    handle
}

impl entity::WriteEntity<Entity, Parent> for ParentSystem {
    fn write(&mut self, eid: Entity, value: Parent) {
        self.send(Operation::Upsert(eid, value));
    }
}

impl entity::ReadEntity<Entity, Parent> for ParentSystem {
    fn read(&self, eid: &Entity) -> Option<&Parent> {
        self.child_to_parent.get(eid)
    }
}

pub type ParentSystem = system::SystemHandle<Message, ParentData>;