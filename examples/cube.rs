extern crate engine;
extern crate fibe;
extern crate snowstorm;
extern crate renderer;
extern crate position;
extern crate scene;
extern crate graphics;
extern crate parent;
extern crate genmesh;
extern crate cgmath;
extern crate entity;

use snowstorm::channel::*;
use graphics::{Vertex, VertexPosTexNorm, PosTexNorm, VertexBuffer,
    Geometry, Material, Primative, KdFlat, DrawBinding
};
use scene::Scene;
use genmesh::generators::Cube;
use genmesh::{MapToVertices, Indexer, LruIndexer};
use genmesh::{Vertices, Triangulate, Quad};
use cgmath::{Vector3, EuclideanVector, Decomposed, Transform};

use entity::Entity;
use position::Delta;

fn build_vectors<T: Iterator<Item=Quad<VertexPosTexNorm>>>(input: T)
    -> (Vertex, Vec<u32>) {

    let mut mesh_data: Vec<VertexPosTexNorm> = Vec::new();
    let index: Vec<u32> = {
        let mut indexer = LruIndexer::new(8, |_, v| mesh_data.push(v));
        input.map(|mut p: Quad<VertexPosTexNorm>| {
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
            p.z.texture = [-1., -1.];
            p.w.texture = [-1.,  1.];

            p
        })
        .vertex(|v| indexer.index(v) as u32)
        .triangulate()
        .vertices()
        .collect()
    };

    (PosTexNorm(mesh_data), index)
}

fn main() {
    let mut engine = engine::Engine::new();
    let (mut tx_parent, rx) = channel();
    let rx_parent = parent::parent(engine.sched(), rx);
    let (mut scene_input, scene_output) = scene::scene(engine.sched(), rx_parent.clone());

    let (mut tx_position, rx) = channel();
    let rx_position = position::position(engine.sched(), rx, rx_parent.clone());

    let (gsink, mut gsrc) = graphics::GraphicsSource::new();

    let (cube, mat) = {
        let (cube_v, cube_i) = build_vectors(
            Cube::new().vertex(|(x, y, z)| {
                VertexPosTexNorm { position: [x, y, z], normal: [0., 0., 0.], texture: [0., 0.] }
            })
        );

        let vb = VertexBuffer::new().bind(cube_v).bind_index(cube_i).write(&mut gsrc);
        let geo = Geometry::new().bind(vb.geometry(Primative::Triangle)).write(&mut gsrc);
        let mat = Material::new().bind(KdFlat([1., 0., 0.])).write(&mut gsrc);
        (geo, mat)
    };

    let count = 5;

    let all = Scene::new();
    let xs: Vec<Scene> = (-count..count).map(|_| Scene::new()).collect();
    let ys: Vec<Scene> = (-count..count).map(|_| Scene::new()).collect();
    let zs: Vec<Scene> = (-count..count).map(|_| Scene::new()).collect();

    for (xi, x) in (-count..count).enumerate() {
        for (yi, y) in (-count..count).enumerate() {
            for (zi, z) in (-count..count).enumerate() {
                let eid = Entity::new().bind(DrawBinding(cube, mat)).write(&mut gsrc);
                let mut pos = Delta(Decomposed::identity());
                pos.0.disp.x = x as f32 * 5.;
                pos.0.disp.y = y as f32 * 5.;
                pos.0.disp.z = z as f32 * 5.;
                eid.bind(pos).write(&mut tx_position);

                all.bind(eid, &mut scene_input);
                xs[xi].bind(eid, &mut scene_input);
                ys[yi].bind(eid, &mut scene_input);
                zs[zi].bind(eid, &mut scene_input);
            }
        }
    }

    let eid = Entity::new().bind(DrawBinding(cube, mat)).write(&mut gsrc);
    eid.bind(Delta(Decomposed::identity())).write(&mut tx_position);

    let mut i = 0;
    let mut scenes: Vec<Scene> = xs.into_iter().chain(ys.into_iter()).chain(zs.into_iter()).collect();
    scenes.push(all);

    let (mut tx, mut rx) = channel();

    engine.start_input_processor(move |sched, mut msg| {
        loop {
            while let Ok(x) = msg.recv() {}
            msg.next_frame();

            tx_parent.next_frame();
            tx_position.next_frame();
            gsrc.next_frame();
            scene_input.next_frame();
            i += 1;
            let len = scenes.len();
            println!("{} {}", i, len);
            tx.send(scenes[(i / 16) % len]);
            tx.flush();
        }
    });

    engine.start_render(|_, render, device|{
        let mut renderer = renderer::Renderer::new(gsink, rx_position, scene_output, render, device);
        Box::new(move |sched, stream| {
        	println!("Render!");
            if let Ok(&scene) = rx.recv() {
        	   renderer.draw(sched, stream, scene);
            } else {
                panic!("uh, oh {:?}", rx.recv());
            }
        })
    });

    engine.run();
}
