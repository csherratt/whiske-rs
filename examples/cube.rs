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

use graphics::{Vertex, VertexBuffer, Geometry,
    Material, MaterialComponent,GeometryData
};
use parent::{Parent, ParentInput};
use renderer::{DrawBinding, Camera, Primary, RendererInput};
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

    let materials = std_graphics::StdMaterials::load(&mut sink.gsrc);
    let shapes = std_graphics::StdGeometry::load(engine.sched(), sink.gsrc.clone());
    let shapes = shapes.get();

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
                       .bind(DrawBinding(shapes.sphere.uv_32, materials.flat.red))
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
