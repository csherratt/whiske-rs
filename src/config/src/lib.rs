extern crate system;
extern crate engine;
extern crate entity;
extern crate ordered_vec;
extern crate parent;
extern crate name;

use engine::fibe::*;
use entity::*;
use ordered_vec::OrderedVec;
use parent::ParentSystem;
use name::PathLookup;

#[derive(Clone, Debug)]
pub enum Config {
    Bool(bool),
    Float(f64),
    String(String)
}

#[derive(Clone)]
pub struct ConfigData {
    pub current: OrderedVec<Entity, Config>,
    pub changed: OrderedVec<Entity, Config>
}

impl ConfigData {
    fn new() -> ConfigData {
        ConfigData {
            current: OrderedVec::new(),
            changed: OrderedVec::new()
        }
    }

    fn apply_ingest(&mut self, data: &[Message]) {
        self.changed.clear();
        self.current.apply_updates(data.iter().map(|x| x.clone()));
        self.changed.apply_updates(data.iter().map(|x| x.clone()));
    }
}

type Message = Operation<Entity, Config>;

// Reads from the parent channel
fn sync_ingest(ingest: &mut system::channel::Receiver<Message>) -> Vec<Operation<Entity, Config>> {
    ingest.iter().map(|x| x.clone()).collect()
}

pub fn config(sched: &mut Schedule, parents: ParentSystem) -> ConfigSystem {
    let td = ConfigData::new();
    let (mut system, handle) = system::System::new(td.clone(), td);

    task(move |_| {
        let mut parents = Some(parents);
        loop {
            let s = system.update(|mut config, old, mut msgs| {
                let p = parents.take().unwrap().next_frame().get().unwrap();

                config.clone_from(old);

                let mut imsgs = sync_ingest(&mut msgs);
                for &d in p.deleted.keys() {
                    imsgs.push(Operation::Delete(d));
                }
                imsgs.sort_by(|a, b| a.key().cmp(b.key()));
                config.apply_ingest(&imsgs[..]);

                parents = Some(p);
                config
            });
            system = if let Some(s) = s { s } else { return; };
        }
    }).start(sched);

    handle
}

impl entity::WriteEntity<Entity, Config> for ConfigSystem {
    fn write(&mut self, eid: Entity, delta: Config) {
        self.send(Operation::Upsert(eid, delta));
    }
}

impl entity::ReadEntity<Entity, Config> for ConfigSystem {
    fn read(&self, eid: &Entity) -> Option<&Config> {
        self.current.get(eid)
    }
}

pub type ConfigSystem = system::SystemHandle<Message, ConfigData>;

pub trait GetConfig<'a> {
    fn config_bool(&self, item: &'a str) -> Option<bool>;
    fn config_f64(&self, item: &'a str) -> Option<f64>;
    fn config_string(&self, item: &'a str) -> Option<&str>;
}

impl<'a, T> GetConfig<'a> for T
    where T: PathLookup<'a> +
             ReadEntity<Entity, Config>
{
    fn config_bool(&self, item: &'a str) -> Option<bool> {
        self.lookup(item)
            .and_then(|eid| self.read(&eid))
            .and_then(|config| {
                if let &Config::Bool(b) = config {
                    Some(b)
                } else {
                    None
                }
            })
    }

    fn config_f64(&self, item: &'a str) -> Option<f64> {
        self.lookup(item)
            .and_then(|eid| self.read(&eid))
            .and_then(|config| {
                if let &Config::Float(f) = config {
                    Some(f)
                } else {
                    None
                }
            })
    }

    fn config_string(&self, item: &'a str) -> Option<&str> {
        self.lookup(item)
            .and_then(|eid| self.read(&eid))
            .and_then(|config| {
                if let &Config::String(ref s) = config {
                    Some(&s[..])
                } else {
                    None
                }
            })
    }
}
