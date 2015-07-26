extern crate fibe;
extern crate entity;
extern crate system;
extern crate parent;

use std::collections::HashMap;
use std::sync::Arc;
use fibe::*;
use entity::{Entity, ReadEntity, WriteEntity, Operation};
use parent::{Parent, ParentSystem};

pub type Message = Operation<Entity, Name>;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Name(Arc<String>);

impl std::borrow::Borrow<str> for Name {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl std::ops::Deref for Name {
    type Target = str;

    fn deref(&self) -> &str {
        &self.0
    }
}

impl Name {
    /// Creates a new Name, if the Name is made up valid characters
    ///
    /// '.' and '/' are both reserved
    pub fn new(s: String) -> Option<Name> {
        for c in s.chars() {
            match c {
                '.' | '/' => return None,
                _ => ()
            }
        }

        Some(Name(Arc::new(s)))
    }
}

#[derive(Clone, Debug)]
pub struct NameData {
    /// Lookup table to find the parent from the child eid
    name: HashMap<Entity, Name>,

    path: HashMap<Entity, HashMap<Name, Entity>>
}

impl NameData {
    fn new() -> NameData {
        NameData{
            name: HashMap::new(),
            path: HashMap::new()
        }
    }

    /// Ingest messages
    fn update(&mut self, data: Vec<Message>, parent: &ParentSystem) {
        for x in data {
            match x {
                Operation::Upsert(eid, name) => {
                    self.name.insert(eid, name.clone());

                    let r: Option<&Parent> = parent.read(&eid);
                    if let Some(&Parent::Child(p)) = r {
                        self.path
                            .entry(p)
                            .or_insert_with(|| HashMap::new())
                            .insert(name, eid);
                    }
                }
                Operation::Delete(ref eid) => {
                    self.name.remove(eid);
                }
            }
        }
    }
}

// Reads from the parent channel
fn sync_ingest(ingest: &mut system::channel::Receiver<Message>) -> Vec<Message> {
    let mut msgs: Vec<Message> = Vec::new();
    while let Ok(op) = ingest.recv() {
        msgs.push(op.clone());
    }
    msgs
}

/// The `parent` system takes and input of parent child bindings
pub fn name(sched: &mut Schedule, parent: ParentSystem) -> NameSystem {
    let pd = NameData::new();
    let (mut system, handle) = system::System::new(pd.clone(), pd);

    task(move |_| {
        let mut parent = Some(parent);
        loop {
            let s = system.update(|mut name, src, mut msgs| {
                let p = parent.take().unwrap().next_frame().get().unwrap();
                name.clone_from(src);
                name.update(sync_ingest(&mut msgs), &p);
                parent = Some(p);
                name
            });
            system = if let Some(s) = s { s } else { return; };
        }
    }).start(sched);

    handle
}

impl entity::WriteEntity<Entity, Name> for NameSystem {
    fn write(&mut self, eid: Entity, value: Name) {
        self.send(Operation::Upsert(eid, value));
    }
}

impl entity::ReadEntity<Entity, Name> for NameSystem {
    fn read(&self, eid: &Entity) -> Option<&Name> {
        self.name.get(eid)
    }
}

pub struct ChildByName<'a>(pub Entity, pub &'a str);

impl<'a> entity::ReadEntity<ChildByName<'a>, Entity> for NameSystem {
    fn read(&self, eid: &ChildByName<'a>) -> Option<&Entity> {
        if let Some(path) = self.path.get(&eid.0) {
            path.get(eid.1)
        } else {
            None
        }
    }
}

pub type NameSystem = system::SystemHandle<Message, NameData>;
