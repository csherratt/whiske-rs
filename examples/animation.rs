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
extern crate animation;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture,
    Material, MaterialComponent, GeometryData
};
use parent::{Parent, ParentSystem};
use renderer::{DrawBinding, Camera, Primary, Renderer};
use scene::Scene;
use cgmath::{Decomposed, Transform, PerspectiveFov, Quaternion, Vector3, rad, Rotation3};
use future_pulse::Future;
use transform::{TransformSystem, Local};
use entity::Entity;
use animation::{animation, Lerp, Animation, AnimationSystem};

router!{
    struct Router {
        [rw: VertexBuffer, Vertex] |
        [rw: VertexBuffer, Vec<u32>] |
        [w: Material, MaterialComponent<[f32; 4]>] |
        [w: Material, MaterialComponent<Texture>] |
        [rw: Texture, image::DynamicImage] |
        [rw: Geometry, GeometryData] => graphics: graphics::Graphics,
        [w: Entity, DrawBinding] |
        [w: Entity, Camera] |
        [w: Entity, Primary] => renderer: Renderer,
        [w: Entity, Local] => transform: TransformSystem,
        [w: Entity, Scene] |
        [w: Scene, Entity] => scene: scene::SceneSystem,
        [w: Entity, Parent] => parent: ParentSystem,
        [w: Entity, Animation] |
        [w: Entity, Lerp] => animation: AnimationSystem
    }
}

impl Router {
    fn next_frame(&mut self) {
        self.parent.next_frame();
        self.graphics.next_frame();
        self.scene.next_frame();
        self.animation.next_frame();
        self.renderer.next_frame();
        self.transform.next_frame();
    }
}

fn main() {
    let mut engine = engine::Engine::new();
    let parent = parent::parent(engine.sched());
    let sscene = scene::scene(engine.sched(), parent.clone());
    let transform = transform::transform(engine.sched(), parent.clone());
    let graphics = graphics::Graphics::new(engine.sched());

    let bound = bounding::Bounding::new(engine.sched(), graphics.clone());

    let t = transform.clone();
    let s = sscene.clone();
    let (read, set) = Future::new();
    engine.start_render(|sched, ra|{
        let (input, mut renderer) = renderer::RendererSystem::new(sched, graphics.clone(), t, s, bound, ra);
        set.set(input);
        Box::new(move |sched, stream| {
            renderer.draw(sched, stream);
        })
    });

    let ch = engine.input_channel();
    let animation = animation(engine.sched(), ch, parent.clone(), transform.clone());

    let mut sink = Router {
        graphics: graphics,
        renderer: read.get(),
        transform: transform,
        scene: sscene,
        parent: parent,
        animation: animation
    };

    let scene = Scene::new();
    let shapes = std_graphics::StdGeometry::load(engine.sched(), sink.graphics.clone()).get();
    let materials = std_graphics::StdMaterials::load(&mut sink.graphics);

    let mut transform: Decomposed<f32, Vector3<f32>, Quaternion<f32>> = Decomposed::identity();
    transform.disp.z = -10f32;

    let mut lerp = Animation::new(1., Decomposed{scale: 1., rot: Rotation3::from_angle_z(rad(0.)), disp: Vector3::new(0., 0., -10.)});

    for i in 1..1000 {
        let idx = i as f32 * -3.1415926535;
        lerp = lerp.then(1., Decomposed{scale: 1., rot: Rotation3::from_angle_z(rad(idx)), disp: Vector3::new(0., 0., -10.)});
    }

    Entity::new()
        .bind(Local(transform))
        .bind(DrawBinding(shapes.cube, materials.flat.red))
        .bind(scene)
        .bind(lerp)
        .write(&mut sink);

    let camera = Entity::new();

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
                  )).write(&mut sink);
            sink.next_frame();
        }
    });
    engine.run();
}
