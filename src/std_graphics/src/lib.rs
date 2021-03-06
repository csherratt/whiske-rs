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
extern crate engine;
extern crate future_pulse;
extern crate genmesh;
extern crate pulse;
extern crate image;
extern crate gfx_mesh;

use image::{DynamicImage, Rgba, GenericImage};

use graphics::{Vertex, VertexBuffer,
    Geometry, Material, Primative, Kd, Graphics, Texture
};

use genmesh::generators::{Plane, Cube, SphereUV};
use genmesh::{MapToVertices, Indexer, LruIndexer};
use genmesh::{Vertices, Triangulate, Quad, Polygon};
use cgmath::{Vector3, EuclideanVector};

use engine::fibe::{Schedule, task};
use future_pulse::Future;
use gfx_mesh::Attribute;


type Out = ([f32; 3], [f32; 3], [f32; 2]);

fn build_vectors<T>(input: T) -> (Vec<Out>, Vec<u32>)
    where T: Iterator<Item=Quad<Out>>
{
    let mut mesh_data: Vec<Out> = Vec::new();
    let index: Vec<u32> = {
        let mut indexer = LruIndexer::new(16, |_, v| mesh_data.push(v));
        input.map(|mut p| {
            let a = Vector3::new(p.x.0[0],
                                 p.x.0[1],
                                 p.x.0[2]);
            let b = Vector3::new(p.y.0[0],
                                 p.y.0[1],
                                 p.y.0[2]);
            let c = Vector3::new(p.z.0[0],
                                 p.z.0[1],
                                 p.z.0[2]);

            let normal = (a - b).cross(&(b - c)).normalize();

            p.x.1 = [normal.x, normal.y, normal.z];
            p.y.1 = [normal.x, normal.y, normal.z];
            p.z.1 = [normal.x, normal.y, normal.z];
            p.w.1 = [normal.x, normal.y, normal.z];

            p.x.2 = [0., 1.];
            p.y.2 = [1., 1.];
            p.z.2 = [1., 0.];
            p.w.2 = [0., 0.];

            p
        })
        .vertex(|v| indexer.index(v) as u32)
        .triangulate()
        .vertices()
        .collect()
    };

    (mesh_data, index)
}

fn build_vectors_poly<T>(input: T) -> (Vec<Out>, Vec<u32>) 
    where T: Iterator<Item=Polygon<(f32, f32, f32)>>
{
    let mut mesh_data: Vec<Out> = Vec::new();
    let index: Vec<u32> = {
        let mut indexer = LruIndexer::new(16, |_, v| mesh_data.push(v));
        input
        .vertex(|(x, y, z)| {
            let n = Vector3::new(x, y, z).normalize();
            ([x, y, z], [n.x, n.y, n.z], [0., 0.])
        })
        .vertex(|v| indexer.index(v) as u32)
        .triangulate()
        .vertices()
        .collect()
    };

    (mesh_data, index)
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
    pub lime: Material,
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
    pub flat: StandardColors,
    pub checkerboard: Material
}

impl StdMaterials {
    /// Load the Materials library
    pub fn load(sink: &mut Graphics) -> StdMaterials {
        let mut checkerboard = DynamicImage::new_rgba8(512, 512);
        for x in 0..512 {
            for y in 0..512 {
                checkerboard.put_pixel(x, y,
                    if (x ^ y) & 0x1 == 0 {
                        Rgba([255, 255, 255, 255])
                    } else {
                        Rgba([  0,   0,   0, 255])
                    }
                );
            }
        }
        let checkerboard = Texture::new().bind(checkerboard).write(sink);

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
                lime:    Material::new().bind(Kd([0.00, 1.00, 0.00, 1.])).write(sink),
                green:   Material::new().bind(Kd([0.00, 0.50, 0.00, 1.])).write(sink),
                aqua:    Material::new().bind(Kd([0.00, 1.00, 1.00, 1.])).write(sink),
                teal:    Material::new().bind(Kd([0.00, 0.50, 0.50, 1.])).write(sink),
                blue:    Material::new().bind(Kd([0.00, 0.00, 1.00, 1.])).write(sink),
                navy:    Material::new().bind(Kd([0.00, 0.00, 0.50, 1.])).write(sink),
                fuchsia: Material::new().bind(Kd([1.00, 0.00, 1.00, 1.])).write(sink),
                purple:  Material::new().bind(Kd([0.50, 0.00, 0.50, 1.])).write(sink)
            },
            checkerboard: Material::new().bind(Kd(checkerboard)).write(sink)
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

fn to_vertex(v: Vec<Out>) -> graphics::Vertex {
    use graphics::{POSITION, NORMAL, TEX0};
    use gfx_mesh::BuildInterlaced;
    [Attribute::f32(POSITION, 3), Attribute::f32(NORMAL, 3), Attribute::f32(TEX0, 2)]
        .build(v.into_iter())
        .unwrap()
        .owned_attributes()
}

fn build_sphere(mut sink: Graphics, size: usize) -> Geometry {
    let (sphere_v, sphere_i) = build_vectors_poly(SphereUV::new(size, size));
    let vb = VertexBuffer::new()
                          .bind(to_vertex(sphere_v))
                          .bind_index(sphere_i)
                          .write(&mut sink);
    Geometry::new()
             .bind(vb.geometry(Primative::Triangle))
             .write(&mut sink)
}

impl StdGeometry {
    pub fn load(sched: &mut Schedule, mut sink: Graphics) -> Future<StdGeometry> {
        let g = sink.clone();
        let uv_2 = task(move |_| build_sphere(g, 2)).start(sched);
        let g = sink.clone();
        let uv_4 = task(move |_| build_sphere(g, 4)).start(sched);
        let g = sink.clone();
        let uv_8 = task(move |_| build_sphere(g, 8)).start(sched);
        let g = sink.clone();
        let uv_16 = task(move |_| build_sphere(g, 16)).start(sched);
        let g = sink.clone();
        let uv_32 = task(move |_| build_sphere(g, 32)).start(sched);
        let g = sink.clone();
        let uv_64 = task(move |_| build_sphere(g, 64)).start(sched);
        let g = sink.clone();
        let uv_128 = task(move |_| build_sphere(g, 128)).start(sched);
        let g = sink.clone();
        let uv_256 = task(move |_| build_sphere(g, 256)).start(sched);

        let mut g = sink.clone();
        let cube = task(move |_| {
            let (cube_v, cube_i) = build_vectors(
                Cube::new().vertex(|(x, y, z)|
                    ([x, y, z], [0., 0., 0.], [0., 0.])
                )
            );
            let vb = VertexBuffer::new()
                                  .bind(to_vertex(cube_v))
                                  .bind_index(cube_i)
                                  .write(&mut g);
            Geometry::new().bind(vb.geometry(Primative::Triangle)).write(&mut g)
        }).start(sched);

        let plane = task(move |_| {
            let (plane_v, plane_i) = build_vectors(
                Plane::new().vertex(|(x, y)|
                    ([x, y, 0.], [0., 0., 0.], [0., 0.])
                )
            );
            let vb = VertexBuffer::new()
                                  .bind(to_vertex(plane_v))
                                  .bind_index(plane_i)
                                  .write(&mut sink);
            Geometry::new().bind(vb.geometry(Primative::Triangle)).write(&mut sink)
        }).start(sched);

        task(move |_| {
            StdGeometry{
                sphere: Spheres {
                    uv_2: uv_2.get(),
                    uv_4: uv_4.get(),
                    uv_8: uv_8.get(),
                    uv_16: uv_16.get(),
                    uv_32: uv_32.get(),
                    uv_64: uv_64.get(),
                    uv_128: uv_128.get(),
                    uv_256: uv_256.get(),
                },
                cube: cube.get(),
                plane: plane.get()
            }
        }).start(sched)
    }
}
