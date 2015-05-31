extern crate entity;
extern crate position;
extern crate graphics;
extern crate scene;

extern crate snowstorm;
extern crate fibe;
#[macro_use]
extern crate gfx;
extern crate gfx_phase;
extern crate gfx_scene;
extern crate gfx_device_gl;
extern crate gfx_pipeline;
extern crate engine;
extern crate draw_queue;
extern crate pulse;
extern crate cgmath;

use std::collections::{HashMap, HashSet};
use snowstorm::channel;
use position::Solved;
use graphics::{
    GraphicsSink, VertexData,
    Pos, PosTex, PosNorm, PosTexNorm,
    MaterialComponent, GeometryData, Geometry
};
use scene::{Scene, SceneOutput};
use engine::Window;
use pulse::{Signal, SelectMap, Signals};
use entity::{Entity, Operation};

use gfx::{
    Mesh, handle, BufferRole, Factory,
    Slice, PrimitiveType, SliceKind
};
use gfx::traits::FactoryExt;
use gfx_device_gl::{Device};
use gfx_scene::{AbstractScene, Report, Error, Context, Frustum};
use gfx_pipeline::{Material, Transparency, forward, Pipeline};
use gfx::device::Resources;

use cgmath::{Bound, Relation, BaseFloat, Decomposed, Vector3, Quaternion};

struct GeometrySlice<R: Resources> {
    mesh: Mesh<R>,
    slice: Slice<R>
}

#[derive(Copy, Clone, Debug)]
struct NullBound;

impl<S: BaseFloat> Bound<S> for NullBound {
    fn relate_plane(&self, _p: &cgmath::Plane<S>) -> Relation {
        Relation::In
    }
}

pub struct Renderer<R: Resources, D, F> {
    device: D,
    factory: F,

    graphics: graphics::GraphicsSink,
    pos_input: channel::Receiver<Operation<Entity, Solved>>,
    render_input: channel::Receiver<Message>,
    scene_output: SceneOutput,

    position: Position,

    vertex: HashMap<Entity, (Option<Mesh<R>>, Option<handle::Buffer<R, u32>>)>,
    materials: HashMap<Entity, Material<R>>,
    geometry_data: HashMap<Entity, GeometryData>,
    geometry_slice: HashMap<Entity, GeometrySlice<R>>,
    cameras: HashMap<Entity, Camera>,
    primary: Option<Entity>,

    binding: HashMap<Entity, DrawBinding>,

    scene: Scene,
    scenes: HashMap<Scene, HashSet<Entity>>,

    pipeline: Option<forward::Pipeline<R>>,

}

pub struct Position(pub HashMap<Entity, Solved>);

impl gfx_scene::World for Position {
    type Scalar = f32;
    type Transform = Decomposed<f32, Vector3<f32>, Quaternion<f32>>;
    type NodePtr = Entity;
    type SkeletonPtr = ();

    fn get_transform(&self, node: &Entity) -> Decomposed<f32, Vector3<f32>, Quaternion<f32>> {
        self.0.get(node).expect("Transform not found").0
    }
}

impl<R: Resources, D, F> AbstractScene<R> for Renderer<R, D, F> {
    type ViewInfo = gfx_pipeline::ViewInfo<f32>;
    type Material = Material<R>;
    type Camera = gfx_scene::Camera<cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>, Entity>;
    type Status = Report;

    fn draw<H, S>(&self,
                  phase: &mut H,
                  camera: &gfx_scene::Camera<cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>, Entity>,
                  stream: &mut S) -> Result<Self::Status, Error> where
        H: gfx_phase::AbstractPhase<R, Material<R>, gfx_pipeline::ViewInfo<f32>>,
        S: gfx::Stream<R>,
    
    {   
        let mut culler = Frustum::new();
        let drawlist = self.scenes.get(&self.scene).unwrap();
        let items: Vec<gfx_scene::Entity<R, Material<R>, Position, NullBound>> =
            drawlist.iter()
                    .filter_map(|eid| self.binding.get(eid).map(|x| (eid, x)))
                    .filter_map(|(eid, draw)| {

            match (self.geometry_slice.get(&(draw.0).0),
                   self.materials.get(&(draw.1).0)) {
                (Some(a), Some(b)) => {
                    Some(gfx_scene::Entity{
                        name: "".to_string(),
                        visible: true,
                        mesh: a.mesh.clone(),
                        node: *eid,
                        skeleton: None,
                        bound: NullBound,
                        fragments: vec![gfx_scene::Fragment{
                            material: b.clone(),
                            slice: a.slice.clone()
                        }]
                    })
                }
                _ => None

            }
        }).collect();

        Context::new(&self.position, &mut culler, camera)
                .draw(items.iter(), phase, stream)
    }
}

#[derive(Copy, Clone)]
pub enum Message {
    Binding(Operation<Entity, DrawBinding>),
    Camera(Operation<Entity, Camera>),
    Slot(Operation<Entity, Primary>)
}

pub struct RendererInput(pub channel::Sender<Message>);

impl RendererInput {
    pub fn next_frame(&mut self) {
        self.0.next_frame();
    }
}

impl entity::WriteEntity<Entity, DrawBinding> for RendererInput {
    fn write(&mut self, eid: Entity, value: DrawBinding) {
        self.0.send(Message::Binding(Operation::Upsert(eid, value)))
    }
}

impl entity::WriteEntity<Entity, Primary> for RendererInput {
    fn write(&mut self, eid: Entity, value: Primary) {
        self.0.send(Message::Slot(Operation::Upsert(eid, value)))
    }
}

impl entity::WriteEntity<Entity, Camera> for RendererInput {
    fn write(&mut self, eid: Entity, value: Camera) {
        self.0.send(Message::Camera(Operation::Upsert(eid, value)))
    }
}


impl<R, D, F> Renderer<R, D, F>
    where R: Resources,
          D: gfx::Device<Resources=R>,
          F: gfx::Factory<R>

{
    pub fn new(graphics: GraphicsSink,
               position: channel::Receiver<Operation<Entity, Solved>>,
               scene: SceneOutput,
               device: D,
               mut factory: F) -> (RendererInput, Renderer<R, D, F>) {

        let pipeline = forward::Pipeline::new(&mut factory);
        let (tx, rx) = channel::channel();

        (RendererInput(tx),
         Renderer {
            device: device,
            factory: factory,
            graphics: graphics,
            pos_input: position,
            render_input: rx,
            position: Position(HashMap::new()),
            vertex: HashMap::new(),
            materials: HashMap::new(),
            geometry_data: HashMap::new(),
            geometry_slice: HashMap::new(),
            binding: HashMap::new(),
            pipeline: Some(pipeline.unwrap()),
            scenes: HashMap::new(),
            scene_output: scene,
            scene: Scene::new(),
            primary: None,
            cameras: HashMap::new()
        })
    }

    fn add_vertex(&mut self, entity: Entity, vertex: VertexData) {
        let dst = self.vertex.entry(entity).or_insert_with(|| (None, None));
        match vertex {
            VertexData::Vertex(Pos(data)) => {
                dst.0 = Some(self.factory.create_mesh(&data[..]));
            }
            VertexData::Vertex(PosTex(data)) => {
                dst.0 = Some(self.factory.create_mesh(&data[..]));
            }
            VertexData::Vertex(PosNorm(data)) => {
                dst.0 = Some(self.factory.create_mesh(&data[..]));
            }
            VertexData::Vertex(PosTexNorm(data)) => {
                dst.0 = Some(self.factory.create_mesh(&data[..]));
            }
            VertexData::Index(data) => {
                dst.1 = Some(self.factory.create_buffer_static(&data, BufferRole::Index));
            }
        }
    }

    fn add_material(&mut self, entity: Entity, material: MaterialComponent) {
        let dst = self.materials.entry(entity)
                      .or_insert_with(|| {
                       Material {
                            color: [0., 0., 0., 1.],
                            texture: None,
                            transparency: Transparency::Opaque
                       }});

        match material {
            MaterialComponent::KdFlat(x) => {
                dst.color[0] = x[0];
                dst.color[1] = x[1];
                dst.color[2] = x[2];
            }
            _ => ()
        }
    }

    fn add_geometry(&mut self, entity: Entity, geometry: GeometryData) {
        self.geometry_data.insert(entity, geometry);

        match self.vertex.get(&geometry.buffer.parent) {
            Some(&(Some(ref v), None)) => {
                Some(GeometrySlice {
                    mesh: v.clone(),
                    slice: Slice {
                        start: geometry.buffer.start,
                        end: geometry.buffer.start + geometry.buffer.length,
                        prim_type: PrimitiveType::TriangleList,
                        kind: SliceKind::Vertex
                    }
                })
            }
            Some(&(Some(ref v), Some(ref i))) => {
                Some(GeometrySlice {
                    mesh: v.clone(),
                    slice: Slice {
                        start: geometry.buffer.start,
                        end: geometry.buffer.start + geometry.buffer.length,
                        prim_type: PrimitiveType::TriangleList,
                        kind: SliceKind::Index32(i.clone(), 0)
                    }
                })
            }
            _ => None
        }.map(|slice| {
            self.geometry_slice.insert(entity, slice);
        });
    }

    fn sync_binding(&mut self) -> Option<Signal> {
        while let Some(msg) = self.render_input.try_recv().map(|x| x.clone()) {
            match msg {
                Message::Binding(Operation::Upsert(eid, binding)) => {
                    self.binding.insert(eid, binding);                  
                }
                Message::Binding(Operation::Delete(eid)) => {
                    self.binding.remove(&eid);
                }
                Message::Camera(Operation::Upsert(eid, camera)) => {
                    self.cameras.insert(eid, camera);
                }
                Message::Camera(Operation::Delete(eid)) => {
                    self.cameras.remove(&eid);
                }
                Message::Slot(Operation::Upsert(eid, _)) => {
                    self.primary = Some(eid);
                }
                Message::Slot(Operation::Delete(eid)) => {
                    if self.primary == Some(eid) {
                        self.primary = None;
                    }
                }
            }
        }

        if self.render_input.closed() {
            None
        } else {
            Some(self.render_input.signal())
        }
    }

    fn sync_graphics(&mut self) -> Option<Signal> {
        use graphics::Message;
        while let Some(msg) = self.graphics.0.try_recv() {
            match msg {
                Message::Vertex(Operation::Upsert(eid, vd)) => {
                    self.add_vertex(eid, vd);
                }
                Message::Vertex(Operation::Delete(eid)) => {
                    self.vertex.remove(&eid);
                }
                Message::Material(Operation::Upsert(eid, mat)) => {
                    self.add_material(eid, mat);
                }
                Message::Material(Operation::Delete(eid)) => {
                    self.materials.remove(&eid);
                }
                Message::Geometry(Operation::Upsert(eid, geo)) => {
                    self.add_geometry(eid, geo);
                }
                Message::Geometry(Operation::Delete(eid)) => {
                    self.geometry_data.remove(&eid);
                    self.geometry_slice.remove(&eid);
                }
            }
        }

        if self.graphics.0.closed() {
            None
        } else {
            Some(self.graphics.0.signal())
        }
    }

    fn sync_position(&mut self) -> Option<Signal> {
        for msg in self.pos_input.copy_iter(false) {
            msg.write(&mut self.position.0);
        }

        if self.pos_input.closed() {
            None
        } else {
            Some(self.pos_input.signal())
        }
    }

    fn sync_scene(&mut self) -> Option<Signal> {
        self.scene_output.write_into(&mut self.scenes)
    }

    fn sync(&mut self) {
        let mut select: SelectMap<fn(&mut Renderer<R, D, F>) -> Option<Signal>> = SelectMap::new();
        select.add(self.graphics.0.signal(), Renderer::sync_graphics);
        select.add(self.pos_input.signal(), Renderer::sync_position);
        select.add(self.scene_output.signal(), Renderer::sync_scene);
        select.add(self.render_input.signal(), Renderer::sync_binding);

        while let Some((_, cb)) = select.next() {
            if let Some(s) = cb(self) {
                select.add(s, cb);
            }
        }

        self.graphics.0.next_frame();
        self.pos_input.next_frame();
        self.scene_output.next_frame();
        self.render_input.next_frame();
    }

    /// 
    pub fn draw(&mut self, _: &mut fibe::Schedule, window: &mut Window<D, R>) {
        self.sync();

        let camera = if let Some(cid) = self.primary {
            if let Some(c) = self.cameras.get(&cid) {
                Some((gfx_scene::Camera {
                    name: "Cam".to_string(),
                    projection: c.0.clone(),
                    node: cid
                }, c.1))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((camera, scene)) = camera {
            self.scene = scene;

            let mut pipeline = self.pipeline.take().unwrap();
            pipeline.render(self, &camera, window).unwrap();
            self.pipeline = Some(pipeline);
            window.present(&mut self.device);
        }
    }
}



/// This holds the binding between a geometry and the material
/// for a drawable entity
#[derive(Copy, Clone, Debug)]
pub struct DrawBinding(pub graphics::Geometry, pub graphics::Material);

///
#[derive(Copy, Clone)]
pub struct Camera(pub cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>, pub Scene);

/// Marker for which camera is the pimary
#[derive(Copy, Clone, Debug)]
pub struct Primary;