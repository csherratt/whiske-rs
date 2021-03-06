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

use std::path::PathBuf;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture, Kd,
    Material, MaterialComponent, GeometryData
};
use parent::{Parent, ParentSystem};
use renderer::{DrawBinding, Camera, Primary, Renderer};
use scene::Scene;
use cgmath::{Decomposed, Transform, PerspectiveFov, Quaternion, Vector3};
use future_pulse::Future;
use transform::{TransformSystem, Local, World};
use entity::Entity;

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
    config_menu::config_menu(
        engine.sched(),
        input,
        name,
        parent.clone(),
        config,
        renderer.clone()
    );

    let mut sink = Router {
        graphics: graphics,
        renderer: renderer,
        transform: transform,
        scene: sscene,
        parent: parent
    };

    let scene = Scene::new();
    let shapes = std_graphics::StdGeometry::load(engine.sched(), sink.graphics.clone());
    let texture = Texture::load(engine.sched(), PathBuf::from("assets/cat.png"), sink.graphics.clone());

    let shapes = shapes.get();
    let texture = texture.get().unwrap();

    let camera = Entity::new();
    let logo_material = Material::new()
                                 .bind(Kd(texture))
                                 .write(&mut sink);
    
    
    // This creates a giant skybox
    let mut transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>> = Decomposed::identity();
    transform.scale = 1.;
    transform.disp.z = -1f32;
    Entity::new()
           .bind(DrawBinding(shapes.plane, logo_material))
           .bind(Local(transform))
           .bind(scene)
           .write(&mut sink);

    engine.start_input_processor(move |_, mut msg| {
        let mut sink = sink;
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
                  )).write(&mut sink);
            sink = sink.next_frame();
        }
    });
    engine.run();
}
