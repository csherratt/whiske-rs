extern crate engine;
extern crate renderer;
extern crate transform;
extern crate scene;
extern crate graphics;
extern crate parent;
#[macro_use(route, router)]
extern crate entity;
extern crate future_pulse;
extern crate std_graphics;
extern crate cgmath;
extern crate image;
extern crate bounding;
extern crate config;
extern crate name;
extern crate config_menu;
extern crate genmesh;
extern crate noise;
extern crate time;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture,
    Material, MaterialComponent, GeometryData,
    VertexPosTexNorm, PosTexNorm, Primative
};
use parent::{Parent, ParentSystem};
use renderer::{DrawBinding, Camera, Primary, Renderer};
use scene::Scene;
use cgmath::{Vector, Decomposed, Transform, PerspectiveFov, Quaternion, Vector3, rad, Rotation3, EuclideanVector};
use future_pulse::Future;
use transform::{TransformSystem, Local, World};
use entity::Entity;
use config_menu::config_menu;

use genmesh::generators::{Plane, Cube, SphereUV};
use genmesh::{MapToVertices, Indexer, LruIndexer, EmitTriangles};
use genmesh::{Vertices, Triangulate, Quad, Polygon, MapVertex};

use noise::{perlin3, Seed, Point2};

router!{
    struct Router {
        [rw: VertexBuffer, Vertex] |
        [rw: VertexBuffer, Vec<u32>] |
        [w: Material, MaterialComponent<[f32; 4]>] |
        [w: Material, MaterialComponent<Texture>] |
        [rw: Texture, image::DynamicImage] |
        [rw: Geometry, GeometryData] => graphics: graphics::Graphics,
        [rw: Entity, DrawBinding] |
        [rw: Entity, Camera] |
        [w: Entity, Primary] => renderer: Renderer,
        [w: Entity, Local] |
        [r: Entity, World] => transform: TransformSystem,
        [w: Entity, Scene] |
        [w: Scene, Entity] => scene: scene::SceneSystem,
        [w: Entity, Parent] => parent: ParentSystem
    }
}

impl Router {
    fn next_frame(self) -> Router {
        let Router {
            parent, scene, renderer, transform, graphics
        } = self;

        let parent = parent.next_frame();
        let scene = scene.next_frame();
        let renderer = renderer.next_frame();
        let transform = transform.next_frame();
        let graphics = graphics.next_frame();

        Router{
            parent: parent.get().unwrap(),
            scene: scene.get().unwrap(),
            renderer: renderer.get().unwrap(),
            transform: transform.get().unwrap(),
            graphics: graphics.get().unwrap()
        }

    }
}

impl Clone for Router {
    fn clone(&self) -> Router {
        Router {
            parent: self.parent.clone(),
            scene: self.scene.clone(),
            renderer: self.renderer.clone(),
            transform: self.transform.clone(),
            graphics: self.graphics.clone(),
        }
    }
}

fn main() {
    let mut engine = engine::Engine::new();
    let parent = parent::parent(engine.sched());
    let sscene = scene::scene(engine.sched(), parent.clone());
    let transform = transform::transform(engine.sched(), parent.clone());
    let graphics = graphics::Graphics::new(engine.sched());
    let bound = bounding::Bounding::new(engine.sched(), graphics.clone());
    let name = name::name(engine.sched(), parent.clone());
    let config = config::config(engine.sched(), parent.clone());

    let t = transform.clone();
    let s = sscene.clone();
    let n = name.clone();
    let c = config.clone();
    let (read, set) = Future::new();
    engine.start_render(|sched, ra|{
        let (input, mut renderer) = renderer::RendererSystem::new(sched, graphics.clone(), t, s, bound, n, c, ra);
        set.set(input);
        Box::new(move |sched, stream| {
            renderer.draw(sched, stream);
        })
    });

    let renderer = read.get();

    let input = engine.input_channel();
    config_menu(
        engine.sched(),
        input,
        name,
        parent.clone(),
        config,
        renderer.clone()
    );

    let ch = engine.input_channel();

    let mut sink = Router {
        graphics: graphics,
        renderer: renderer,
        transform: transform,
        scene: sscene,
        parent: parent,
    };

    let scene = Scene::new();
    let materials = std_graphics::StdMaterials::load(&mut sink.graphics);
    let camera = Entity::new();

    let mut transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>> = Decomposed::identity();
    transform.disp.z = -10f32;
    transform.scale = 4.;


    let mut router = sink.clone();
    let start = time::precise_time_s();
    engine.start_input_processor(move |_, mut msg| {

        let vb = VertexBuffer::new();
        let geo = Geometry::new();
        
        let seed = Seed::new(0);

        Entity::new()
            .bind(Local(transform))
            .bind(DrawBinding(geo, materials.flat.red))
            .bind(scene)
            .write(&mut router);

        let mut x = 0.;

        loop {
            for _ in msg.copy_iter(true) {}
            msg.next_frame();

            let now = time::precise_time_s();
            build_sphere(&seed, (now - start) as f32, vb, geo, &mut router);

            router = router.next_frame();
        }
    });

    let mut router = sink;
    engine.start_input_processor(move |_, mut msg| {
        loop {
            for _ in msg.copy_iter(true) {}
            msg.next_frame();

            camera.bind(Primary)
                  .bind(Local(Decomposed::identity()))
                  .bind(Camera(
                    PerspectiveFov {
                        fovy: cgmath::deg(90.),
                        aspect: 4./3.,
                        near: 0.1,
                        far: 1000.
                    },
                    scene
                  )).write(&mut router);

            router = router.next_frame();
        }
    });
    engine.run();
}

fn build_vectors<U, P, T: Iterator<Item=P>>(input: T) -> (graphics::Vertex, Vec<u32>)
    where P: MapVertex<(f32, f32, f32), u32, Output=U>,
          U: EmitTriangles<Vertex=u32>
{

    let mut mesh_data: Vec<VertexPosTexNorm> = Vec::new();
    let index: Vec<u32> = {
        let mut indexer = LruIndexer::new(16, |_, v| mesh_data.push(v));
        input
        .vertex(|(x, y, z)| {
            let n = Vector3::new(x, y, z).normalize();
            let v = VertexPosTexNorm {
                position: [x, y, z],
                texture: [0., 0.],
                normal: [n.x, n.y, n.z]
            };
            indexer.index(v) as u32
        })
        .triangulate()
        .vertices()
        .collect()
    };

    (PosTexNorm(mesh_data), index)
}

fn build_sphere(seed: &Seed, xx: f32, vb: VertexBuffer, geo: Geometry, sink: &mut Router) {
    let (plane_v, plane_i) = build_vectors(
        SphereUV::new(32, 32).vertex(
            |(x, y, z)| {
                let v = Vector3::new(x, y, z).mul_s(
                    (perlin3(seed, &[x + xx, y, z]) + 3.) / 4.
                );
                (v.x, v.y, v.z)
            }
        )
    );
    let vb = vb.bind(plane_v).bind_index(plane_i).write(sink);
    geo.bind(vb.geometry(Primative::Triangle)).write(sink);
}


