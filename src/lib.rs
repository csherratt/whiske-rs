//   Copyright 2014-2015 Colin Sherratt
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

extern crate cgmath;
extern crate entity;
extern crate graphics;
extern crate fibe;
extern crate future_pulse;
extern crate genmesh;
extern crate pulse;

use graphics::{Vertex, VertexPosTexNorm, PosTexNorm, VertexBuffer,
    Geometry, Material, Primative, Kd, GraphicsSource
};

use genmesh::generators::{Plane, Cube, SphereUV};
use genmesh::{MapToVertices, Indexer, LruIndexer};
use genmesh::{Vertices, Triangulate, Quad, Polygon};
use cgmath::{Vector3, EuclideanVector};

use fibe::{Schedule, task};
use future_pulse::Future;
use pulse::{Barrier, Signals};

fn build_vectors<T: Iterator<Item=Quad<VertexPosTexNorm>>>(input: T)
    -> (graphics::Vertex, Vec<u32>) {

    let mut mesh_data: Vec<VertexPosTexNorm> = Vec::new();
    let index: Vec<u32> = {
        let mut indexer = LruIndexer::new(16, |_, v| mesh_data.push(v));
        input.map(|mut p| {
            let a = Vector3::new(p.x.position[0],
                                 p.x.position[1],
                                 p.x.position[2]);
            let b = Vector3::new(p.y.position[0],
                                 p.y.position[1],
                                 p.y.position[2]);
            let c = Vector3::new(p.z.position[0],
                                 p.z.position[1],
                                 p.z.position[2]);

            let normal = (a - b).cross(&(b - c)).normalize();

            p.x.normal = [normal.x, normal.y, normal.z];
            p.y.normal = [normal.x, normal.y, normal.z];
            p.z.normal = [normal.x, normal.y, normal.z];
            p.w.normal = [normal.x, normal.y, normal.z];

            p.x.texture = [-1., -1.];
            p.y.texture = [-1.,  1.];
            p.z.texture = [ 1.,  1.];
            p.w.texture = [ 1., -1.];

            p
        })
        .vertex(|v| indexer.index(v) as u32)
        .triangulate()
        .vertices()
        .collect()
    };

    (PosTexNorm(mesh_data), index)
}

fn build_vectors_poly<T: Iterator<Item=Polygon<(f32, f32, f32)>>>(input: T)
    -> (graphics::Vertex, Vec<u32>) {

    let mut mesh_data: Vec<VertexPosTexNorm> = Vec::new();
    let index: Vec<u32> = {
        let mut indexer = LruIndexer::new(16, |_, v| mesh_data.push(v));
        input
        .vertex(|(x, y, z)| {
            let n = Vector3::new(x, y, z).normalize();
            VertexPosTexNorm {
                position: [x, y, z],
                texture: [0., 0.],
                normal: [n.x, n.y, n.z]
            }
        })
        .vertex(|v| indexer.index(v) as u32)
        .triangulate()
        .vertices()
        .collect()
    };

    (PosTexNorm(mesh_data), index)
}

#[derive(Clone, Copy, Debug)]
pub struct StandardColors {
    pub white: Material,
    pub silver: Material,
    pub gray: Material,
    pub black: Material,
    pub red: Material,
    pub maroon: Material,
    pub yellow: Material,
    pub olive: Material,
    pub line: Material,
    pub green: Material,
    pub aqua: Material,
    pub teal: Material,
    pub blue: Material,
    pub navy: Material,
    pub fuchsia: Material,
    pub purple: Material,
}

#[derive(Copy, Clone, Debug)]
pub struct StdMaterials {
    pub flat: StandardColors
}

impl StdMaterials {
    /// Load the Materials library
    pub fn load(sink: &mut GraphicsSource) -> StdMaterials {
        StdMaterials {
            flat: StandardColors {
                white:   Material::new().bind(Kd([1.00, 1.00, 1.00, 1.])).write(sink),
                silver:  Material::new().bind(Kd([0.75, 0.75, 0.75, 1.])).write(sink),
                gray:    Material::new().bind(Kd([0.50, 0.50, 0.50, 1.])).write(sink),
                black:   Material::new().bind(Kd([0.00, 0.00, 0.00, 1.])).write(sink),
                red:     Material::new().bind(Kd([1.00, 0.00, 0.00, 1.])).write(sink),
                maroon:  Material::new().bind(Kd([0.50, 0.00, 0.00, 1.])).write(sink),
                yellow:  Material::new().bind(Kd([1.00, 1.00, 0.00, 1.])).write(sink),
                olive:   Material::new().bind(Kd([0.50, 0.50, 0.00, 1.])).write(sink),
                line:    Material::new().bind(Kd([0.00, 1.00, 0.00, 1.])).write(sink),
                green:   Material::new().bind(Kd([0.00, 0.50, 0.00, 1.])).write(sink),
                aqua:    Material::new().bind(Kd([0.00, 1.00, 1.00, 1.])).write(sink),
                teal:    Material::new().bind(Kd([0.00, 0.50, 0.50, 1.])).write(sink),
                blue:    Material::new().bind(Kd([0.00, 0.00, 1.00, 1.])).write(sink),
                navy:    Material::new().bind(Kd([0.00, 0.00, 0.50, 1.])).write(sink),
                fuchsia: Material::new().bind(Kd([1.00, 0.00, 1.00, 1.])).write(sink),
                purple:  Material::new().bind(Kd([0.50, 0.00, 0.50, 1.])).write(sink)
            }
        }

    }
}

#[derive(Copy, Clone, Debug)]
pub struct Spheres {
    pub uv_2: Geometry,
    pub uv_4: Geometry,
    pub uv_8: Geometry,
    pub uv_16: Geometry,
    pub uv_32: Geometry,
    pub uv_64: Geometry,
    pub uv_128: Geometry,
    pub uv_256: Geometry,
}

#[derive(Copy, Clone, Debug)]
pub struct StdGeometry {
    pub cube: Geometry,
    pub plane: Geometry,
    pub sphere: Spheres,
}

fn build_sphere(mut sink: GraphicsSource, size: usize, geo: Geometry) {
    let (sphere_v, sphere_i) = build_vectors_poly(SphereUV::new(size, size));
    let vb = VertexBuffer::new()
                          .bind(sphere_v)
                          .bind_index(sphere_i)
                          .write(&mut sink);
    geo.bind(vb.geometry(Primative::Triangle)).write(&mut sink);
}

impl StdGeometry {
    pub fn load(sched: &mut Schedule, mut sink: GraphicsSource) -> Future<StdGeometry> {
        let s = Spheres {
            uv_2: Geometry::new(),
            uv_4: Geometry::new(),
            uv_8: Geometry::new(),
            uv_16: Geometry::new(),
            uv_32: Geometry::new(),
            uv_64: Geometry::new(),
            uv_128: Geometry::new(),
            uv_256: Geometry::new()
        };

        let mut tasks = Vec::new();

        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 2, s.uv_2)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 4, s.uv_4)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 8, s.uv_8)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 16, s.uv_16)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 32, s.uv_32)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 64, s.uv_64)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 128, s.uv_128)).start(sched));
        let g = sink.clone();
        tasks.push(task(move |_| build_sphere(g, 256, s.uv_256)).start(sched));

        let cube = Geometry::new();
        let mut g = sink.clone();
        tasks.push(task(move |_| {
            let (cube_v, cube_i) = build_vectors(
                Cube::new().vertex(|(x, y, z)| {
                    VertexPosTexNorm {
                        position: [x, y, z],
                        texture: [0., 0.],
                        normal: [0., 0., 0.]
                    }
                }
            ));
            let vb = VertexBuffer::new()
                                  .bind(cube_v)
                                  .bind_index(cube_i)
                                  .write(&mut g);
            cube.bind(vb.geometry(Primative::Triangle)).write(&mut g);
        }).start(sched));

        let plane = Geometry::new();
        tasks.push(task(move |_| {
            let (plane_v, plane_i) = build_vectors(
                Plane::new().vertex(|(x, y)| {
                    VertexPosTexNorm {
                        position: [x, y, 0.],
                        texture: [0., 0.],
                        normal: [0., 0., 0.]
                    }
                }
            ));
            let vb = VertexBuffer::new()
                                  .bind(plane_v)
                                  .bind_index(plane_i)
                                  .write(&mut sink);
            cube.bind(vb.geometry(Primative::Triangle)).write(&mut sink);
        }).start(sched));

        let (future, set) = Future::new();
        task(move |_| {
            set.set(StdGeometry{
                sphere: s,
                cube: cube,
                plane: plane
            });
        }).after(Barrier::new(&tasks).signal()).start(sched);
        future
    }
}
