extern crate engine;
extern crate fibe;
extern crate renderer;
extern crate transform;
extern crate scene;
extern crate graphics;
extern crate parent;
#[macro_use(route, router)]
extern crate entity;
extern crate future_pulse;
extern crate no_clip;
extern crate cgmath;
extern crate time;
extern crate image;
extern crate obj_loader;
extern crate bounding;
extern crate config;
extern crate name;

use std::path::PathBuf;
use std::env::args;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture,
    Material, MaterialComponent, GeometryData
};
use parent::{Parent, ParentSystem};
use renderer::{DrawBinding, Camera, Primary, Renderer, DebugText};
use scene::Scene;
use cgmath::{Decomposed, Transform, PerspectiveFov};
use future_pulse::Future;
use transform::TransformSystem;

use entity::Entity;
use transform::{Local, World};

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
        [rw: Entity, DebugText] |
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
        let Router{
            parent, graphics, scene, transform, renderer
        } = self;

        let parent = parent.next_frame();
        let graphics = graphics.next_frame();
        let scene = scene.next_frame();
        let transform = transform.next_frame();
        let renderer = renderer.next_frame();
    
        Router {
            parent: parent.get().unwrap(),
            graphics: graphics.get().unwrap(),
            scene: scene.get().unwrap(),
            transform: transform.get().unwrap(),
            renderer: renderer.get().unwrap(),
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

    let s = sscene.clone();
    let t = transform.clone();
    let (read, set) = Future::new();
    engine.start_render(|sched, ra|{
        let (input, mut renderer) = renderer::RendererSystem::new(sched, graphics.clone(), t, s, bound, name, config, ra);
        set.set(input);
        Box::new(move |sched, stream| {
            renderer.draw(sched, stream);
        })
    });

    let mut sink = Router {
        graphics: graphics,
        renderer: read.get(),
        transform: transform,
        scene: sscene,
        parent: parent
    };

    let scene = Scene::new();

    let mut args = args(); args.next();
    let map = args.next().expect("Please supply a path");

    let obj = obj_loader::load(engine.sched(), PathBuf::from(map), sink.graphics.clone());
    println!("Waiting for load");
    for (_, (g, m)) in obj.unwrap().get() {
        let comp = Decomposed::identity();
        if let Some(m) = m {
            Entity::new()
                   .bind(DrawBinding(g, m))
                   .bind(scene)
                   .bind(Local(comp))
                   .write(&mut sink);
        }
    }
    println!("Done!");


    let camera = Entity::new();
    camera.bind(Primary)
          .bind(Camera(
            PerspectiveFov {
                fovy: cgmath::deg(90.),
                aspect: 4./3.,
                near: 0.1,
                far: 10000.
            },
            scene
          )).write(&mut sink);

    let trans = sink.transform.clone();
    engine.start_input_processor(move |sched, msg| {
        no_clip::no_clip(sched, camera, Decomposed::identity(), msg, trans);
    });

    engine.start_input_processor(move |_, mut msg| {
        let mut sink = sink;
        loop {
            for _ in msg.copy_iter(true) {}
            msg.next_frame();
            sink = sink.next_frame();
        }
    });
    engine.run();
}
