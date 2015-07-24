
extern crate engine;
extern crate fibe;
extern crate entity;
extern crate cgmath;
extern crate transform;
extern crate ordered_vec;
extern crate system;
extern crate parent;
extern crate shared_future;

use cgmath::{Decomposed, Vector3, Quaternion, EuclideanVector};
use ordered_vec::OrderedVec;
use entity::{Entity, Operation};
use transform::{TransformSystem, Local};
use fibe::*;
use engine::event::WindowEvent;

#[derive(Copy, Clone, Debug)]
pub struct Lerp {
    pub time: f64,
    pub to: Decomposed<f32, Vector3<f32>, Quaternion<f32>>,
}

impl Lerp {
    pub fn new(when: f64, transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>>) -> Lerp {
        Lerp{
            time: when,
            to: transform,
        }
    }

    /// Create an empty lerp that time x seconds after the last lerp
    pub fn then(self, after: f64, transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>>) -> Animation {
        Animation{
            lerps: vec![
                self,
                Lerp{
                    time: self.time + after,
                    to: transform,
                }
            ]
        }
    }
}

#[derive(Clone, Debug)]
pub struct Animation {
    lerps: Vec<Lerp>
}

impl Animation {
    pub fn new(when: f64, transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>>) -> Animation {
        Animation{
            lerps: vec![Lerp{
                time: when,
                to: transform,
            }]
        }
    }

    /// Create an empty lerp that time x seconds after the last lerp
    pub fn then(mut self, after: f64, transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>>) -> Animation {
        let len = self.lerps.len();
        let time = self.lerps[len-1].time;
        self.lerps.push(Lerp{
            time: time + after,
            to: transform
        });
        self
    }
}

#[derive(Clone)]
pub struct AnimationData {
    pub lerps: OrderedVec<Entity, Animation>,
}

impl AnimationData {
    fn new() -> AnimationData {
        AnimationData {
            lerps: OrderedVec::new(),
        }
    }

    fn update(&mut self, t: &mut TransformSystem, last: f64, now: f64) {
        let mut delete = Vec::new();

        for (eid, anim) in self.lerps.iter_mut() {
            let mut current = if let Some(l) = t.local(*eid) {
                l.0
            } else {
                continue;
            };

            loop {
                let lerp = anim.lerps[0];

                let (x, pop) = if now > lerp.time {
                    (1.0, true)
                } else {
                    (((now - last) / (lerp.time - now)) as f32, false)
                };

                current = Decomposed {
                    scale: (lerp.to.scale - current.scale) * x + current.scale,
                    disp: current.disp.lerp(&lerp.to.disp, x as f32),
                    rot: current.rot.slerp(&lerp.to.rot, x as f32)
                };

                if pop {
                    anim.lerps.remove(0);
                    if anim.lerps.len() == 0 {
                        delete.push(Operation::Delete(*eid));
                        break;
                    }
                } else {
                    break;
                }
            }

            eid.bind(Local(current)).write(t);
        }

        if delete.len() != 0 {
            self.lerps.apply_updates(delete.into_iter());
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

pub type Message = Operation<Entity, Animation>;

pub fn animation(sched: &mut fibe::Schedule,
                 mut input: engine::InputChannel,
                 mut parent: parent::ParentSystem,
                 transform: TransformSystem) -> AnimationSystem {

    let mut transform = Some(shared_future::Future::from_value(transform));
    let ad = AnimationData::new();
    let (mut system, handle) = system::System::new(ad.clone(), ad);

    let mut time = 0.;
    let mut last_time = 0.;

    task(move |_| {
        loop {
            system = system.update(|mut anim, old, mut msgs| {
                last_time = time;
                for x in input.iter() {
                    if let &WindowEvent::TimeStamp(t) = x {
                        time = t;
                    }
                }
                input.next_frame();
                parent.next_frame();
                anim.clone_from(old);

                let mut msgs = sync_ingest(&mut msgs);
                for &p in &parent.deleted {
                    msgs.push(Operation::Delete(p));
                }
                msgs.sort_by(|a, b| a.key().cmp(b.key()));

                anim.lerps.apply_updates(msgs.into_iter());


                let mut t = transform.take().unwrap().get().unwrap();
                anim.update(&mut t, last_time, time);
                transform = Some(t.next_frame_async());

                anim
            });
        }
    }).start(sched);

    handle

}

impl entity::WriteEntity<Entity, Lerp> for AnimationSystem {
    fn write(&mut self, eid: Entity, lerp: Lerp) {
        self.send(Operation::Upsert(eid, Animation{
            lerps: vec![lerp]
        }));
    }
}


impl entity::WriteEntity<Entity, Animation> for AnimationSystem {
    fn write(&mut self, eid: Entity, a: Animation) {
        self.send(Operation::Upsert(eid, a));
    }
}


pub type AnimationSystem = system::SystemHandle<Message, AnimationData>;