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
#[macro_use(router)]
extern crate entity;
extern crate future_pulse;

use snowstorm::channel::*;
use graphics::{Vertex, VertexPosTexNorm, PosTexNorm, VertexBuffer,
    Geometry, Material, Primative, KdFlat, MaterialComponent,
    GeometryData
};
use parent::Parent;
use renderer::{DrawBinding, Camera, Primary, RendererInput};
use scene::Scene;
use genmesh::generators::Cube;
use genmesh::{MapToVertices, Indexer, LruIndexer};
use genmesh::{Vertices, Triangulate, Quad};
use cgmath::{Vector3, EuclideanVector, Decomposed, Transform, PerspectiveFov};
use future_pulse::Future;

use entity::{Entity, Operation};
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

router!{
    struct Router {
        [VertexBuffer, Vertex] |
        [VertexBuffer, Vec<u32>] |
        [Material, MaterialComponent] |
        [Geometry, GeometryData] => gsrc: graphics::GraphicsSource,
        [Entity, DrawBinding] |
        [Entity, Camera] |
        [Entity, Primary] => renderer: RendererInput,
        [Entity, Delta] => transform: Sender<Operation<Entity, Delta>>,
        [Entity, Scene] |
        [Scene, Entity] => scene: scene::SceneInput,
        [Entity, Parent] => parent: Sender<Operation<Entity, Parent>>
    }
}

fn main() {
    let mut engine = engine::Engine::new();
    let (tx_parent, rx) = channel();
    let rx_parent = parent::parent(engine.sched(), rx);
    let (scene_input, scene_output) = scene::scene(engine.sched(), rx_parent.clone());

    let (tx_position, rx) = channel();
    let rx_position = position::position(engine.sched(), rx, rx_parent.clone());

    let (gsink, gsrc) = graphics::GraphicsSource::new();

    let (read, set) = Future::new();
    engine.start_render(|_, render, device|{
        let (input, mut renderer) = renderer::Renderer::new(gsink, rx_position, scene_output, render, device);
        set.set(input);
        Box::new(move |sched, stream| {
            renderer.draw(sched, stream);
        })
    });

    let mut sink = Router {
        gsrc: gsrc,
        renderer: read.get(),
        transform: tx_position,
        scene: scene_input,
        parent: tx_parent
    };

    let (cube, mat) = {
        let (cube_v, cube_i) = build_vectors(
            Cube::new().vertex(|(x, y, z)| {
                VertexPosTexNorm { position: [x, y, z], normal: [0., 0., 0.], texture: [0., 0.] }
            })
        );

        let vb = VertexBuffer::new().bind(cube_v).bind_index(cube_i).write(&mut sink);
        let geo = Geometry::new().bind(vb.geometry(Primative::Triangle)).write(&mut sink);
        let mat = Material::new().bind(KdFlat([1., 0., 0.])).write(&mut sink);
        (geo, mat)
    };

    let count = 10;

    let all = Scene::new();
    let xs: Vec<Scene> = (-count..count).map(|_| Scene::new()).collect();
    let ys: Vec<Scene> = (-count..count).map(|_| Scene::new()).collect();
    let zs: Vec<Scene> = (-count..count).map(|_| Scene::new()).collect();

    for (xi, x) in (-count..count).enumerate() {
        for (yi, y) in (-count..count).enumerate() {
            for (zi, z) in (-count..count).enumerate() {

                let mut pos = Delta(Decomposed::identity());
                pos.0.disp.x = x as f32 * 5.;
                pos.0.disp.y = y as f32 * 5.;
                pos.0.disp.z = z as f32 * 5.;

                Entity::new()
                       .bind(DrawBinding(cube, mat))
                       .bind(pos)
                       .bind(all)
                       .bind(xs[xi])
                       .bind(ys[yi])
                       .bind(zs[zi])
                       .write(&mut sink);
            }
        }
    }

    let mut i = 0;
    let mut scenes: Vec<Scene> = xs.into_iter().chain(ys.into_iter()).chain(zs.into_iter()).collect();
    scenes.push(all);

    let camera = Entity::new();
    engine.start_input_processor(move |_, mut msg| {
        loop {
            // TODO, fibers~
            while let Ok(_) = msg.recv() {}
            msg.next_frame();

            i += 1;
            let len = scenes.len();
            camera.bind(Delta(Decomposed::identity()))
                  .bind(Primary)
                  .bind(Camera(
                    PerspectiveFov {
                        fovy: cgmath::deg(90.),
                        aspect: 4./3.,
                        near: 0.1,
                        far: 1000.
                    },
                    scenes[(i) % len]))
                  .write(&mut sink);

            sink.parent.next_frame();
            sink.gsrc.next_frame();
            sink.scene.next_frame();
            sink.transform.next_frame();
            sink.renderer.next_frame();
        }
    });
    engine.run();
}
