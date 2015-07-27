extern crate snowstorm;
extern crate transform;
extern crate entity;
extern crate fibe;
extern crate cgmath;
extern crate pulse;
extern crate camera;
extern crate engine;

use std::f32;
use entity::Entity;
use engine::event::{WindowEvent, Key, Action};
use transform::{TransformSystem, Local};
use snowstorm::channel::Receiver;
use fibe::{Schedule, task};
use cgmath::{Decomposed, Quaternion, Vector3, rad, Rotation3, Angle};

pub fn no_clip(sched: &mut Schedule,
               entity: Entity,
               mut last: Decomposed<f32, Vector3<f32>, Quaternion<f32>>,
               mut input: Receiver<WindowEvent>,
               mut output: TransformSystem) {
    
    let mut speed_foward = 0.;
    let mut speed_right = 0.;
    let speed_up = 0.;
    let mut last_mouse = None;
    let rate = 2. / 60.;

    task(move |_| {
        loop {
            for msg in input.copy_iter(true) {
                match msg {
                    WindowEvent::CursorPos(x, y) => {
                        let (dx, dy) = match last_mouse {
                            Some((ox, oy)) => ((x - ox) as f32, (y - oy) as f32),
                            None => (0., 0.)
                        };
                        let (mut rx, ry, mut rz) = last.rot.to_euler();

                        rx = rx.add_a(rad((-dx / 120.) as f32));
                        rz = rz.add_a(rad((-dy / 120.) as f32));

                        let max_rot: f32 = f32::consts::FRAC_PI_2;
                        if rz.s > max_rot {
                            rz.s = max_rot;
                        } else if rz.s < -max_rot {
                            rz.s = -max_rot;
                        }

                        last.rot = Rotation3::from_euler(rx, ry, rz);
                        last_mouse = Some((x, y));
                    }
                    WindowEvent::Key(Key::W, _, Action::Press, _) => {
                        speed_foward = rate;
                    }
                    WindowEvent::Key(Key::S, _, Action::Press, _) => {
                        speed_foward = -rate;
                    }
                    WindowEvent::Key(Key::W, _, Action::Release, _) => {
                        speed_foward = 0.;
                    }
                    WindowEvent::Key(Key::S, _, Action::Release, _) => {
                        speed_foward = 0.;
                    }
                    WindowEvent::Key(Key::A, _, Action::Press, _) => {
                        speed_right = -rate;
                    }
                    WindowEvent::Key(Key::D, _, Action::Press, _) => {
                        speed_right = rate;
                    }
                    WindowEvent::Key(Key::A, _, Action::Release, _) => {
                        speed_right = 0.;
                    }
                    WindowEvent::Key(Key::D, _, Action::Release, _) => {
                        speed_right = 0.;
                    }
                    _ => ()
                }
            }

            let camera = camera::Camera::new(last);
            let pos = camera.move_with_vector(
                &Vector3::new(speed_right,
                              speed_up,
                              -speed_foward)
            );

            last.disp.x = pos.x;
            last.disp.y = pos.y;
            last.disp.z = pos.z;

            entity.bind(Local(last)).write(&mut output);

            if !input.next_frame() {
                return;
            } else {
                output = output.next_frame().get().unwrap();
            }
        }
    }).start(sched);
}
