extern crate engine;
extern crate fibe;
extern crate renderer;
extern crate transform;
extern crate scene;
extern crate graphics;
extern crate parent;
extern crate genmesh;
extern crate cgmath;
#[macro_use(router)]
extern crate entity;
extern crate future_pulse;
extern crate no_clip;

use graphics::{Vertex, VertexPosTexNorm, PosTexNorm, VertexBuffer,
    Geometry, Material, Primative, KdFlat, MaterialComponent,
    GeometryData
};
use parent::{Parent, ParentInput};
use renderer::{DrawBinding, Camera, Primary, RendererInput};
use scene::Scene;
use genmesh::generators::Cube;
use genmesh::{MapToVertices, Indexer, LruIndexer};
use genmesh::{Vertices, Triangulate, Quad};
use cgmath::{Vector3, EuclideanVector, Decomposed, Transform, PerspectiveFov};
use future_pulse::Future;
use transform::TransformInput;

use entity::Entity;
use transform::Delta;

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
        [Entity, Delta] => transform: TransformInput,
        [Entity, Scene] |
        [Scene, Entity] => scene: scene::SceneInput,
        [Entity, Parent] => parent: ParentInput
    }
}

impl Router {
    fn next_frame(&mut self) {
        self.parent.next_frame();
        self.gsrc.next_frame();
        self.scene.next_frame();
        self.transform.next_frame();
        self.renderer.next_frame();
    }
}

fn main() {
    let mut engine = engine::Engine::new();
    let (pinput, poutput) = parent::parent(engine.sched());
    let (sinput, soutput) = scene::scene(engine.sched(), poutput.clone());
    let (tinput, toutput) = transform::transform(engine.sched(), poutput.clone());
    let (gsink, gsrc) = graphics::GraphicsSource::new();

    let (read, set) = Future::new();
    engine.start_render(|_, render, device|{
        let (input, mut renderer) = renderer::Renderer::new(gsink, toutput, soutput, render, device);
        set.set(input);
        Box::new(move |sched, stream| {
            renderer.draw(sched, stream);
        })
    });

    let mut sink = Router {
        gsrc: gsrc,
        renderer: read.get(),
        transform: tinput,
        scene: sinput,
        parent: pinput
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
    let shell: Vec<Scene> = (0..(count*2)).map(|_| Scene::new()).collect();

    for (xi, x) in (-count..count).enumerate() {
        for (yi, y) in (-count..count).enumerate() {
            for (zi, z) in (-count..count).enumerate() {

                let mut pos = Delta(Decomposed::identity());
                pos.0.disp.x = x as f32 * 5.;
                pos.0.disp.y = y as f32 * 5.;
                pos.0.disp.z = z as f32 * 5.;

                let layer = ((x*x+y*y+z*z) as f32).sqrt() as usize;

                Entity::new()
                       .bind(DrawBinding(cube, mat))
                       .bind(pos)
                       .bind(all)
                       .bind(xs[xi])
                       .bind(ys[yi])
                       .bind(zs[zi])
                       .bind(shell[layer])
                       .write(&mut sink);
            }
        }
    }

    let mut i = 0;
    let scenes: Vec<Scene> = xs.into_iter()
                               .chain(ys.into_iter())
                               .chain(zs.into_iter())
                               .chain(shell.into_iter()).collect();

    let camera = Entity::new();

    let trans = sink.transform.clone();
    engine.start_input_processor(move |sched, msg| {
        no_clip::no_clip(sched, camera, Decomposed::identity(), msg, trans);
    });

    engine.start_input_processor(move |_, mut msg| {
        loop {
            for _ in msg.copy_iter(true) {}
            msg.next_frame();

            i += 1;
            camera.bind(Primary)
                  .bind(Camera(
                    PerspectiveFov {
                        fovy: cgmath::deg(90.),
                        aspect: 4./3.,
                        near: 0.1,
                        far: 1000.
                    },
                    scenes[(i / 4) % scenes.len()]))
                  .write(&mut sink);
            sink.next_frame();
        }
    });
    engine.run();
}
