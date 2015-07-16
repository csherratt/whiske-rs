
use std::collections::HashMap;
use cgmath;
use graphics::{Geometry, Material};
use scene::Scene;
use entity::{self, Entity, Operation};
use snowstorm::mpsc::*;
use system;
use fibe::{self, task};


/// This holds the binding between a geometry and the material
/// for a drawable entity
#[derive(Copy, Clone, Debug)]
pub struct DrawBinding(pub Geometry, pub Material);

///
#[derive(Copy, Clone)]
pub struct Camera(pub cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>, pub Scene);

/// Marker for which camera is the pimary
#[derive(Copy, Clone, Debug)]
pub struct Primary;

#[derive(Clone, Debug)]
pub struct DebugText{
    pub text: String,
    pub start: [i32; 2],
    pub color: [f32; 4]
}

#[derive(Clone)]
pub enum Message {
    Binding(Operation<Entity, DrawBinding>),
    Camera(Operation<Entity, Camera>),
    Slot(Operation<Entity, Primary>),
    DebugText(Operation<Entity, DebugText>)
}

pub type Renderer = system::SystemHandle<Message, RenderData>;

impl entity::WriteEntity<Entity, DrawBinding> for Renderer {
    fn write(&mut self, eid: Entity, value: DrawBinding) {
        self.send(Message::Binding(Operation::Upsert(eid, value)))
    }
}

impl entity::WriteEntity<Entity, Primary> for Renderer {
    fn write(&mut self, eid: Entity, value: Primary) {
        self.send(Message::Slot(Operation::Upsert(eid, value)))
    }
}

impl entity::WriteEntity<Entity, Camera> for Renderer {
    fn write(&mut self, eid: Entity, value: Camera) {
        self.send(Message::Camera(Operation::Upsert(eid, value)))
    }
}

impl entity::WriteEntity<Entity, DebugText> for Renderer {
    fn write(&mut self, eid: Entity, value: DebugText) {
        self.send(Message::DebugText(Operation::Upsert(eid, value)))
    }
}

#[derive(Clone)]
pub struct RenderData {
    pub cameras: HashMap<Entity, Camera>,
    pub debug_text: HashMap<Entity, DebugText>,
    pub binding: HashMap<Entity, DrawBinding>,
    pub primary: Option<Entity>,
}

impl RenderData {
    fn new() -> RenderData {
        RenderData {
            cameras: HashMap::new(),
            binding: HashMap::new(),
            debug_text: HashMap::new(),
            primary: None
        }
    }

    fn apply_ingest(&mut self, msgs: &[Message]) {
        for m in msgs.iter() {
            match m {
                &Message::Binding(Operation::Upsert(eid, binding)) => {
                    self.binding.insert(eid, binding);                  
                }
                &Message::Binding(Operation::Delete(eid)) => {
                    self.binding.remove(&eid);
                }
                &Message::Camera(Operation::Upsert(eid, camera)) => {
                    self.cameras.insert(eid, camera);
                }
                &Message::Camera(Operation::Delete(eid)) => {
                    self.cameras.remove(&eid);
                }
                &Message::Slot(Operation::Upsert(eid, _)) => {
                    self.primary = Some(eid);
                }
                &Message::Slot(Operation::Delete(eid)) => {
                    if self.primary == Some(eid) {
                        self.primary = None;
                    }
                }
                &Message::DebugText(Operation::Upsert(eid, ref text)) => {
                    self.debug_text.insert(eid, text.clone());
                }
                &Message::DebugText(Operation::Delete(eid)) => {
                    self.debug_text.remove(&eid);
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

pub fn renderer(sched: &mut fibe::Schedule) -> Renderer {
    let rd = RenderData::new();
    let (mut system, handle) = system::System::new(rd.clone(), rd);

    let mut limsgs = Vec::new();

    task(move |_| {
        loop {
            system = system.update(|mut scene, _, mut msgs| {
                let imsgs = sync_ingest(&mut msgs);

                scene.apply_ingest(&limsgs[..]);
                scene.apply_ingest(&imsgs[..]);

                limsgs = imsgs;
                scene
            });
        }
    }).start(sched);

    handle
}
