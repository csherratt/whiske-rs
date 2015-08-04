extern crate entity;
extern crate system;
extern crate parent;
extern crate engine;

use std::collections::HashMap;
use std::sync::Arc;
use engine::fibe::*;
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
        if s.len() == 0 {
            return None;
        }

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

    /// The `root` is used for objects with no parents
    root: HashMap<Name, Entity>,

    /// path is used for children of the root
    path: HashMap<Entity, HashMap<Name, Entity>>
}

impl NameData {
    fn new() -> NameData {
        NameData{
            name: HashMap::new(),
            path: HashMap::new(),
            root: HashMap::new()
        }
    }

    /// Ingest messages
    fn update(&mut self, data: Vec<Message>, parent: &ParentSystem) {
        for x in data {
            match x {
                Operation::Upsert(eid, name) => {
                    self.name.insert(eid, name.clone());

                    match parent.read(&eid) {
                        Some(&Parent::Child(p)) => {
                            self.path
                                .entry(p)
                                .or_insert_with(|| HashMap::new())
                                .insert(name, eid);
                        }
                        Some(&Parent::Root) | None => {
                            self.root.insert(name, eid);
                        }
                    };
                }
                Operation::Delete(ref eid) => {
                    self.name.remove(eid);
                    self.path.remove(eid);
                }
            }
        }

        for (c, v) in parent.deleted.iter() {
            let name = if let Some(name) = self.name.remove(c) {
                name
            } else {
                continue;
            };
            self.path.remove(c);

            match v {
                &Some(Parent::Root) | &None => {
                    self.root
                        .remove(&name);
                }
                &Some(Parent::Child(ref p)) => {
                    self.path
                        .get_mut(p)
                        .map(|x| x.remove(&name));
                }
            }
        }

        for (c, old) in parent.modified.iter() {
            let name = if let Some(name) = self.name.get(c) {
                name
            } else {
                continue;
            };
            let new: &Parent = parent.read(c).unwrap();

            match old {
                &Some(Parent::Root) | &None => {
                    self.root
                        .remove(name);
                }
                &Some(Parent::Child(ref p)) => {
                    self.path
                        .get_mut(p)
                        .map(|x| x.remove(name));
                }
            }

            match new {
                &Parent::Root => {
                    self.root
                        .insert(name.clone(), *c);
                }
                &Parent::Child(p) => {
                    self.path
                        .entry(p)
                        .or_insert_with(|| HashMap::new())
                        .insert(name.clone(), *c);
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

impl entity::ReadEntity<Entity, Name> for NameData {
    fn read(&self, eid: &Entity) -> Option<&Name> {
        self.name.get(eid)
    }
}

pub struct ChildByName<'a>(pub Entity, pub &'a str);

impl<'a> entity::ReadEntity<ChildByName<'a>, Entity> for NameData {
    fn read(&self, eid: &ChildByName<'a>) -> Option<&Entity> {
        if let Some(path) = self.path.get(&eid.0) {
            path.get(eid.1)
        } else {
            None
        }
    }
}

impl<'a> entity::ReadEntity<ChildByName<'a>, Entity> for NameSystem {
    fn read(&self, eid: &ChildByName<'a>) -> Option<&Entity> {
        if let Some(path) = self.path.get(&eid.0) {
            path.get(eid.1)
        } else {
            None
        }
    }
}

pub struct RootName<'a>(pub &'a str);

impl<'a> entity::ReadEntity<RootName<'a>, Entity> for NameData {
    fn read(&self, eid: &RootName<'a>) -> Option<&Entity> {
        self.root.get(eid.0)
    }
}

impl<'a> entity::ReadEntity<RootName<'a>, Entity> for NameSystem {
    fn read(&self, eid: &RootName<'a>) -> Option<&Entity> {
        self.root.get(eid.0)
    }
}

pub type NameSystem = system::SystemHandle<Message, NameData>;

pub trait PathLookup<'a> {
    fn lookup(&self, path: &'a str) -> Option<&Entity>;
}

impl<'a, T> PathLookup<'a> for T
    where T: ReadEntity<RootName<'a>, Entity> +
             ReadEntity<ChildByName<'a>, Entity> 
{
    fn lookup(&self, path: &'a str) -> Option<&Entity> {
        let mut path: std::str::Split<'a, char> = path.split('.');

        let root_path = if let Some(path) = path.next() {
            path
        } else {
            return None;
        };

        let mut node = if let Some(eid) = self.read(&RootName(root_path)) {
            eid
        } else {
            return None;
        };

        loop {
            let p = if let Some(p) = path.next() {
                p
            } else {
                return Some(node);
            };

            node = if let Some(eid) = self.read(&ChildByName(*node, p)) {
                eid
            } else {
                return None
            };
        }
    } 
}

pub trait FullPath {
    /// create a printable path from an eid
    fn full_path(&self, eid: &Entity) -> Option<String>;
}

impl<T> FullPath for T
    where T: ReadEntity<Entity, Parent> +
             ReadEntity<Entity, Name>
{
    /// create a printable path from an eid
    fn full_path(&self, eid: &Entity) -> Option<String> {
        let mut base = match self.read(eid) {
            None | Some(&Parent::Root) => String::new(),
            Some(&Parent::Child(ref p)) => {
                let mut p = if let Some(p) = self.full_path(p) {
                    p
                } else {
                    return None;
                };
                p.push_str(".");
                p
            }
        };

        let name: Option<&Name> = self.read(eid);
        if let Some(name) = name {
            base.push_str(&**name);
            Some(base)
        } else {
            None
        }
    }
}
