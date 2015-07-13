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
extern crate std_graphics;
extern crate cgmath;
extern crate time;
extern crate image;
extern crate bounding;

use graphics::{
    Vertex, VertexBuffer, Geometry, Texture,
    Material, MaterialComponent, GeometryData
};
use parent::{Parent, ParentInput};
use renderer::{DrawBinding, Camera, Primary, Renderer, DebugText};
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
        [Geometry, GeometryData] => graphics: graphics::Graphics,
        [Entity, DrawBinding] |
        [Entity, Camera] |
        [Entity, DebugText] |
        [Entity, Primary] => renderer: Renderer,
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
    engine.start_render(|sched, ra|{
        let (input, mut renderer) = renderer::RendererSystem::new(sched, graphics.clone(), toutput, soutput, bound, ra);
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

    let materials = std_graphics::StdMaterials::load(&mut sink.graphics);
    let shapes = std_graphics::StdGeometry::load(engine.sched(), sink.graphics.clone());
    let shapes = shapes.get();

    let count = 20;

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
                       .bind(DrawBinding(shapes.cube, materials.flat.red))
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
    /*let scenes: Vec<Scene> = xs.into_iter()
                               .chain(ys.into_iter())
                               .chain(zs.into_iter())
                               .chain(shell.into_iter()).collect();*/

    let camera = Entity::new();

    let trans = sink.transform.clone();
    engine.start_input_processor(move |sched, msg| {
        no_clip::no_clip(sched, camera, Decomposed::identity(), msg, trans);
    });

    let text = Entity::new();
    engine.start_input_processor(move |_, mut msg| {
        let mut start = time::precise_time_s();
        let mut end = time::precise_time_s();

        loop {
            let start_of_loop = time::precise_time_s();
            for _ in msg.copy_iter(true) {}
            msg.next_frame();

            i += 1;

            text.bind(DebugText{
                text: format!("Input Loop {:3.2}ms", (end - start) * 1000.),
                start: [20, 20],
                color: [1., 1., 1., 1.]
            }).write(&mut sink);

            camera.bind(Primary)
                  .bind(Camera(
                    PerspectiveFov {
                        fovy: cgmath::deg(90.),
                        aspect: 4./3.,
                        near: 0.1,
                        far: 1000.
                    },
                    shell[(i / 4) % shell.len()]))
                  .write(&mut sink);
            sink.next_frame();

            start = start_of_loop;
            end = time::precise_time_s();

        }
    });
    engine.run();
}
