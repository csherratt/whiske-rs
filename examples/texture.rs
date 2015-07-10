extern crate engine;
extern crate renderer;
extern crate transform;
extern crate scene;
extern crate graphics;
extern crate parent;
#[macro_use(router)]
extern crate entity;
extern crate future_pulse;
extern crate std_graphics;
extern crate cgmath;
extern crate image;
extern crate bounding;

use std::path::PathBuf;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture, Kd,
    Material, MaterialComponent, GeometryData
};
use parent::{Parent, ParentInput};
use renderer::{DrawBinding, Camera, Primary, RendererInput};
use scene::Scene;
use cgmath::{Decomposed, Transform, PerspectiveFov, Quaternion, Vector3};
use future_pulse::Future;
use transform::{TransformInput, Delta};
use entity::Entity;

router!{
    struct Router {
        [VertexBuffer, Vertex] |
        [VertexBuffer, Vec<u32>] |
        [Material, MaterialComponent<[f32; 4]>] |
        [Material, MaterialComponent<Texture>] |
        [Texture, image::DynamicImage] |
        [Geometry, GeometryData] => graphics: graphics::Graphics,
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
        self.graphics.next_frame();
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
    let graphics = graphics::Graphics::new(engine.sched());

    let bound = bounding::Bounding::new(engine.sched(), graphics.clone());

    let (read, set) = Future::new();
    engine.start_render(|_, ra|{
        let (input, mut renderer) = renderer::Renderer::new(graphics.clone(), toutput, soutput, bound, ra);
        set.set(input);
        Box::new(move |sched, stream| {
            renderer.draw(sched, stream);
        })
    });

    let mut sink = Router {
        graphics: graphics,
        renderer: read.get(),
        transform: tinput,
        scene: sinput,
        parent: pinput
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
           .bind(Delta(transform))
           .bind(scene)
           .write(&mut sink);

    engine.start_input_processor(move |_, mut msg| {
        loop {
            for _ in msg.copy_iter(true) {}
            msg.next_frame();

            camera.bind(Primary)
                  .bind(Delta(Decomposed::identity()))
                  .bind(Camera(
                    PerspectiveFov {
                        fovy: cgmath::deg(90.),
                        aspect: 4./3.,
                        near: 0.1,
                        far: 1000.
                    },
                    scene
                  )).write(&mut sink);
            sink.next_frame();
        }
    });
    engine.run();
}
