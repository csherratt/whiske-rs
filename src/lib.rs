

extern crate glutin;
extern crate snowstorm;
extern crate transform;
extern crate entity;
extern crate fibe;
extern crate cgmath;
extern crate pulse;
extern crate camera;

use std::f32;
use entity::Entity;
use glutin::{Event, VirtualKeyCode};
use glutin::ElementState::{Pressed, Released};
use transform::{TransformInput, Delta};
use snowstorm::channel::Receiver;
use fibe::{Schedule, ResumableTask, WaitState, IntoTask};
use cgmath::{Decomposed, Quaternion, Vector3, rad, Rotation3, Angle};
use pulse::{Signals};

struct NoClip {
    entity: Entity,
    input: Receiver<glutin::Event>,
    output: TransformInput,
    speed_foward: f32,
    speed_right: f32,
    speed_up: f32,
    last: Decomposed<f32, Vector3<f32>, Quaternion<f32>>,
    last_mouse: Option<(i32, i32)>
}

impl ResumableTask for NoClip {
    fn resume(&mut self, _: &mut Schedule) -> WaitState {
        for msg in self.input.copy_iter(false) {
            match msg {
                Event::MouseMoved((x, y)) => {
                    let (dx, dy) = match self.last_mouse {
                        Some((ox, oy)) => ((x - ox) as f32, (y - oy) as f32),
                        None => (0., 0.)
                    };
                    let (mut rx, ry, mut rz) = self.last.rot.to_euler();

                    rx = rx.add_a(rad((-dx / 120.) as f32));
                    rz = rz.add_a(rad((-dy / 120.) as f32));

                    let max_rot: f32 = f32::consts::FRAC_PI_2;
                    if rz.s > max_rot {
                        rz.s = max_rot;
                    } else if rz.s < -max_rot {
                        rz.s = -max_rot;
                    }

                    self.last.rot = Rotation3::from_euler(rx, ry, rz);
                    self.last_mouse = Some((x, y));
                }
                Event::KeyboardInput(Pressed, _, Some(VirtualKeyCode::W)) => {
                    self.speed_foward = 1.;
                }
                Event::KeyboardInput(Pressed, _, Some(VirtualKeyCode::S)) => {
                    self.speed_foward = -1.;
                }
                Event::KeyboardInput(Released, _, Some(VirtualKeyCode::W)) => {
                    self.speed_foward = 0.;
                }
                Event::KeyboardInput(Released, _, Some(VirtualKeyCode::S)) => {
                    self.speed_foward = 0.;
                }
                Event::KeyboardInput(Pressed, _, Some(VirtualKeyCode::A)) => {
                    self.speed_right = -1.;
                }
                Event::KeyboardInput(Pressed, _, Some(VirtualKeyCode::D)) => {
                    self.speed_right = 1.;
                }
                Event::KeyboardInput(Released, _, Some(VirtualKeyCode::A)) => {
                    self.speed_right = 0.;
                }
                Event::KeyboardInput(Released, _, Some(VirtualKeyCode::D)) => {
                    self.speed_right = 0.;
                }
                _ => ()
            }
        }

        if self.input.closed() {
            let camera = camera::Camera::new(self.last);
            let pos = camera.move_with_vector(
                &Vector3::new(self.speed_right,
                              self.speed_up,
                              -self.speed_foward)
            );

            self.last.disp.x = pos.x;
            self.last.disp.y = pos.y;
            self.last.disp.z = pos.z;

            self.entity
                .bind(Delta(self.last))
                .write(&mut self.output);

            if !self.input.next_frame() {
                return WaitState::Completed;
            } else {
                self.output.next_frame();
            }
        }
        
        WaitState::Pending(self.input.signal())
    }
}

pub fn no_clip(sched: &mut Schedule,
               entity: Entity,
               start: Decomposed<f32, Vector3<f32>, Quaternion<f32>>,
               input: Receiver<Event>,
               mut output: TransformInput) {
    
    let sig = input.signal();

    entity.bind(Delta(start)).write(&mut output);

    NoClip {
        entity: entity,
        input: input,
        output: output,
        last: start,
        speed_up: 0.,
        speed_right: 0.,
        speed_foward: 0.,
        last_mouse: None,
    }.after(sig).start(sched);
}