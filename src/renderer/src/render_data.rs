
use std;
use std::ops;
use std::collections::HashMap;
use cgmath;
use graphics::{Geometry, Material};
use scene::Scene;
use entity::{self, Entity, Operation};
use snowstorm::mpsc::*;
use lease;
use shared_future;
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

#[derive(Clone)]
pub enum Renderer {
    Valid {
        /// a channel to send graphics data with
        channel: Sender<Message>,

        /// Link the the future of this store
        next: shared_future::Future<Renderer>,

        /// Link to the data associated with this frame
        data: lease::Lease<RenderData>,
    },
    UpdatePending
}

impl entity::WriteEntity<Entity, DrawBinding> for Renderer {
    fn write(&mut self, eid: Entity, value: DrawBinding) {
        match self {
            &mut Renderer::Valid{ref mut channel, next: _, data: _} => {
                channel.send(Message::Binding(Operation::Upsert(eid, value)))
            }
            _ => ()
        }
    }
}

impl entity::WriteEntity<Entity, Primary> for Renderer {
    fn write(&mut self, eid: Entity, value: Primary) {
        match self {
            &mut Renderer::Valid{ref mut channel, next: _, data: _} => {
                channel.send(Message::Slot(Operation::Upsert(eid, value)))
            }
            _ => ()
        }
    }
}

impl entity::WriteEntity<Entity, Camera> for Renderer {
    fn write(&mut self, eid: Entity, value: Camera) {
        match self {
            &mut Renderer::Valid{ref mut channel, next: _, data: _} => {
                channel.send(Message::Camera(Operation::Upsert(eid, value)))
            }
            _ => ()
        }
    }
}

impl entity::WriteEntity<Entity, DebugText> for Renderer {
    fn write(&mut self, eid: Entity, value: DebugText) {
        match self {
            &mut Renderer::Valid{ref mut channel, next: _, data: _} => {
                channel.send(Message::DebugText(Operation::Upsert(eid, value)))
            }
            _ => ()
        }
    }
}

#[derive(Clone)]
pub struct RenderData {
    pub cameras: HashMap<Entity, Camera>,
    pub debug_text: HashMap<Entity, DebugText>,
    pub binding: HashMap<Entity, DrawBinding>,
    pub primary: Option<Entity>,
}

impl Renderer {
    pub fn new(sched: &mut fibe::Schedule) -> Renderer {
        let (tx, rx) = channel();
        let (future, set) = shared_future::Future::new();

        let (fowner, lease) = lease::lease(RenderData{
            cameras: HashMap::new(),
            debug_text: HashMap::new(),
            binding: HashMap::new(),
            primary: None
        });

        let (bowner, _) = lease::lease(RenderData{
            cameras: HashMap::new(),
            debug_text: HashMap::new(),
            binding: HashMap::new(),
            primary: None
        });


        task(|_| worker(fowner, bowner, set, rx)).start(sched);

        Renderer::Valid {
            channel: tx,
            next: future,
            data: lease
        }
    }


    /// Fetch the next frame
    pub fn next_frame(&mut self) -> bool {
        use std::mem;
        let mut pending = Renderer::UpdatePending;
        mem::swap(&mut pending, self);
        let (mut channel, next, data) = match pending {
            Renderer::Valid{channel, next, data} => (channel, next, data),
            Renderer::UpdatePending => panic!("Invalid state"),
        };
        channel.flush();
        drop(data);
        drop(channel);
        match next.get().ok() {
            Some(next) => {
                *self = next;
                true
            }
            None => false
        }
    }
}

impl ops::Deref for Renderer {
    type Target = RenderData;

    fn deref(&self) -> &RenderData {
        match self {
            &Renderer::Valid{channel: _, next: _, ref data} => data,
            _ => panic!("Graphics is being Updated!")
        }
    }
}


fn worker(mut front: lease::Owner<RenderData>,
          mut back: lease::Owner<RenderData>,
          mut set: shared_future::Set<Renderer>,
          mut input: Receiver<Message>) {
    loop {
        let mut data = back.get();
        data.clone_from(&*front);

        while let Ok(msg) = input.recv().map(|x| x.clone()) {
            match msg {
                Message::Binding(Operation::Upsert(eid, binding)) => {
                    data.binding.insert(eid, binding);                  
                }
                Message::Binding(Operation::Delete(eid)) => {
                    data.binding.remove(&eid);
                }
                Message::Camera(Operation::Upsert(eid, camera)) => {
                    data.cameras.insert(eid, camera);
                }
                Message::Camera(Operation::Delete(eid)) => {
                    data.cameras.remove(&eid);
                }
                Message::Slot(Operation::Upsert(eid, _)) => {
                    data.primary = Some(eid);
                }
                Message::Slot(Operation::Delete(eid)) => {
                    if data.primary == Some(eid) {
                        data.primary = None;
                    }
                }
                Message::DebugText(Operation::Upsert(eid, text)) => {
                    data.debug_text.insert(eid, text);
                }
                Message::DebugText(Operation::Delete(eid)) => {
                    data.debug_text.remove(&eid);
                }
            }
        }

        let (nowner, lease) = lease::lease(data);
        let (tx, ninput) = channel();
        let (next, nset) = shared_future::Future::new();
        set.set(Renderer::Valid{
            channel: tx,
            next: next,
            data: lease
        });
        back = front;
        front = nowner;
        set = nset;
        input = ninput;
    }
}