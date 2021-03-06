extern crate cgmath;
extern crate graphics;
extern crate entity;
extern crate lease;
extern crate engine;
extern crate shared_future;

use std::collections::{HashMap, HashSet};
use cgmath::{Aabb, Aabb3, Point3, Vector4, Matrix, Matrix4};
use entity::Entity;
use graphics::{Graphics, Geometry, GeometryData, VertexBufferData};
use engine::fibe::*;

#[derive(Clone)]
pub struct Bounding {
    inner: Option<lease::Lease<BoundingStore>>,
    next: shared_future::Future<Bounding>
}

#[derive(Clone)]
pub struct BoundingStore {
    vb_to_geo: HashMap<Entity, HashSet<Geometry>>,
    pub aabb: HashMap<graphics::Geometry, Aabb3<f32>>,
    pub aabb_updated: HashSet<graphics::Geometry>
}

fn to_point3(p: [f32; 3]) -> Point3<f32> {
    Point3::new(p[0], p[1], p[2])
}

fn create_aabb(geo: &GeometryData, vb: &VertexBufferData) -> Option<Aabb3<f32>> {
    if vb.vertex.len() == 0 {
        return None;
    }
    let position = vb.vertex[0].attribute_reader(0).unwrap();


    match vb.index {
        Some(ref index) => {
            let first = position[index[geo.buffer.start as usize] as usize];
            let mut aabb = Aabb3::new(to_point3(first), to_point3(first));

            for i in (geo.buffer.start+1)..(geo.buffer.start + geo.buffer.length) {
                let pos = position[index[i as usize] as usize];
                aabb = aabb.grow(&to_point3(pos));
            }

            Some(aabb)
        }
        None => {
            let first = position[geo.buffer.start as usize];
            let mut aabb = Aabb3::new(to_point3(first), to_point3(first));

            for i in (geo.buffer.start+1)..(geo.buffer.start + geo.buffer.length) {
                let pos = position[i as usize];
                aabb = aabb.grow(&to_point3(pos));
            }

            Some(aabb)
        }
    }
}


impl BoundingStore {
    /// Search for any geometry that has been modified if it has
    /// add it to the list of geometries to be updated. If the VB
    /// a geometry is owned by is modified invalidate all geometries
    /// that are invalidated and added to the list to be updated
    fn create_update_list(&self, g: &Graphics) -> HashSet<Geometry> {
        let mut update = HashSet::new();

        for (k, _) in g.geometry_updated.iter() {
            update.insert(*k);
        }

        for (v, _) in g.vertex_buffer_updated.iter() {
            if let Some(vb_to_geo) = self.vb_to_geo.get(v) {
                for k in vb_to_geo.iter() {
                    update.insert(*k);
                }
            }
        }

        update
    }

    fn update(&mut self, g: &graphics::Graphics) {
        let updated = self.create_update_list(g);
        for geo in updated.iter() {
            let aabb = if let Some(gdat) = g.geometry.get(&geo) {
                if let Some(vb) = g.vertex_buffer.get(&gdat.buffer.parent) {
                    self.vb_to_geo
                        .entry(gdat.buffer.parent)
                        .or_insert_with(|| HashSet::new())
                        .insert(*geo);

                    // ok now we have the VB we an created the geometry
                    create_aabb(gdat, vb)
                } else {
                    None
                }
            } else {
                None
            };

            if let Some(aabb) = aabb {
                self.aabb.insert(*geo, aabb);
            } else {
                self.aabb.remove(geo);
            }
        }
        self.aabb_updated = updated;
    }
}

impl Bounding {
    /// Create a new bounding system
    pub fn new(sched: &mut Schedule, graphics: graphics::Graphics) -> Bounding {
        let mut inner = BoundingStore {
            vb_to_geo: HashMap::new(),
            aabb: HashMap::new(),
            aabb_updated: HashSet::new()
        };

        inner.update(&graphics);

        let (mut front, l) = lease::lease(inner.clone());
        let (mut back, _) = lease::lease(inner);
        let (future, mut set) = shared_future::Future::new();

        let mut graphics = Some(graphics);
        task(move |_| {
            loop {
                let g = graphics.take().unwrap().next_frame().get().unwrap();
                let mut inner = back.get();
                inner.clone_from(&*front);

                inner.update(&g);
                let (nown, nlease) = lease::lease(inner);
                back = front;
                front = nown;
                let (next, nset) = shared_future::Future::new();
                set.set(Bounding{
                    inner: Some(nlease),
                    next: next
                });
                set = nset;
                graphics = Some(g);

            }
        }).start(sched);

        Bounding {
            inner: Some(l),
            next: future
        }
    }

    pub fn next_frame(&mut self) {
        drop(self.inner.take());
        let Bounding{inner, next} = self.next.clone().get().unwrap();
        self.inner = inner;
        self.next = next;
    }

    /// calculate a scaled aabb
    pub fn scaled_aabb(&self, geo: &Geometry, mat: Matrix4<f32>) -> Option<Aabb3<f32>> {
        fn to_point3(v: Vector4<f32>) -> Point3<f32> {
            Point3::new(v.x / v.w, v.y / v.w, v.z / v.w)
        }

        self.aabb.get(geo)
            .map(|aabb| {
                let points = [
                    to_point3(mat.mul_v(&Vector4::new(aabb.min.x, aabb.min.y, aabb.min.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.min.x, aabb.min.y, aabb.max.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.min.x, aabb.max.y, aabb.min.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.min.x, aabb.max.y, aabb.max.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.max.x, aabb.min.y, aabb.min.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.max.x, aabb.min.y, aabb.max.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.max.x, aabb.max.y, aabb.min.z, 1.))),
                    to_point3(mat.mul_v(&Vector4::new(aabb.max.x, aabb.max.y, aabb.max.z, 1.))),
                ];

                let mut aabb = Aabb3::new(points[0], points[1]);
                for p in points[2..].iter() {
                    aabb = aabb.grow(p);
                }
                aabb
            })
    }
}

impl std::ops::Deref for Bounding {
    type Target = BoundingStore;

    fn deref(&self) -> &BoundingStore {
        self.inner.as_ref().unwrap()
    }
}