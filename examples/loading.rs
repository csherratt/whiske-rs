extern crate engine;
extern crate fibe;
extern crate renderer;
extern crate transform;
extern crate scene;
extern crate graphics;
extern crate parent;
#[macro_use(router)]
extern crate entity;
extern crate future_pulse;
extern crate no_clip;
extern crate cgmath;
extern crate time;
extern crate image;
extern crate obj_loader;

use std::path::PathBuf;
use std::env::args;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture,
    Material, MaterialComponent, GeometryData
};
use parent::{Parent, ParentInput};
use renderer::{DrawBinding, Camera, Primary, RendererInput, DebugText};
use scene::Scene;
use cgmath::{Decomposed, Transform, PerspectiveFov};
use future_pulse::Future;
use transform::TransformInput;

use entity::Entity;
use transform::Delta;

router!{
    struct Router {
        [VertexBuffer, Vertex] |
        [VertexBuffer, Vec<u32>] |
        [Material, MaterialComponent<[f32; 4]>] |
        [Material, MaterialComponent<Texture>] |
        [Texture, image::DynamicImage] |
        [Geometry, GeometryData] => gsrc: graphics::GraphicsSource,
        [Entity, DrawBinding] |
        [Entity, Camera] |
        [Entity, DebugText] |
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

    let scene = Scene::new();

    let mut args = args(); args.next();
    let map = args.next().expect("Please supply a path");

    let obj = obj_loader::load(engine.sched(), PathBuf::from(map), sink.gsrc.clone());
    println!("Waiting for load");
    for (_, (g, m)) in obj.unwrap().get() {
        if let Some(m) = m {
            Entity::new()
                   .bind(DrawBinding(g, m))
                   .bind(scene)
                   .bind(Delta(Decomposed::identity()))
                   .write(&mut sink);
        }
    }


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
        loop {
            for _ in msg.copy_iter(true) {}
            msg.next_frame();
            sink.next_frame();
        }
    });
    engine.run();
}
