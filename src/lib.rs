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
    GraphicsSink, Message, VertexData,
    Pos, PosTex, PosNorm, PosTexNorm,
    MaterialComponent, GeometryData, DrawBinding
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
use gfx_device_gl::{Device, Resources};
use gfx_scene::{AbstractScene, Camera, Report, Error, Context, Frustum};
use gfx_pipeline::{Material, Transparency, forward, Pipeline};

use cgmath::{Bound, Relation, BaseFloat, Decomposed, Vector3, Quaternion, Transform};

struct GeometrySlice {
    mesh: Mesh<Resources>,
    slice: Slice<Resources>
}

#[derive(Copy, Clone, Debug)]
struct NullBound;

impl<S: BaseFloat> Bound<S> for NullBound {
    fn relate_plane(&self, _p: &cgmath::Plane<S>) -> Relation {
        Relation::In
    }
}

pub struct Renderer {
    device: Device,
    factory: gfx_device_gl::Factory,

    graphics: graphics::GraphicsSink,
    pos_input: channel::Receiver<Operation<Entity, Solved>>,
    scene_output: SceneOutput,

    position: Position,

    vertex: HashMap<Entity, (Option<Mesh<Resources>>, Option<handle::Buffer<Resources, u32>>)>,
    materials: HashMap<Entity, Material<Resources>>,
    geometry_data: HashMap<Entity, GeometryData>,
    geometry_slice: HashMap<Entity, GeometrySlice>,

    binding: HashMap<Entity, DrawBinding>,
    to_draw: HashMap<Entity, gfx_scene::Entity<Resources, Material<Resources>, Position, NullBound>>,

    scene: Scene,
    scenes: HashMap<Scene, HashSet<Entity>>,

    pipeline: Option<forward::Pipeline<Resources>>,

}

pub struct Position(pub HashMap<Entity, Solved>);

impl gfx_scene::World for Position {
    type Scalar = f32;
    type Transform = Decomposed<f32, Vector3<f32>, Quaternion<f32>>;
    type NodePtr = Entity;
    type SkeletonPtr = ();

    fn get_transform(&self, node: &Entity) -> Decomposed<f32, Vector3<f32>, Quaternion<f32>> {
        self.0.get(node).unwrap().0
    }
}

impl AbstractScene<Resources> for Renderer {
    type ViewInfo = gfx_pipeline::ViewInfo<f32>;
    type Material = Material<Resources>;
    type Camera = Camera<cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>, Entity>;
    type Status = Report;

    fn draw<H, S>(&self,
                  phase: &mut H,
                  camera: &Camera<cgmath::PerspectiveFov<f32, cgmath::Deg<f32>>, Entity>,
                  stream: &mut S) -> Result<Report, Error> where
        H: gfx_phase::AbstractPhase<Resources, Material<Resources>, gfx_pipeline::ViewInfo<f32>>,
        S: gfx::Stream<Resources>,
    
    {   
        let mut culler = Frustum::new();
        let drawlist = self.scenes.get(&self.scene).unwrap();
        let iter = drawlist.iter().map(|x| self.to_draw.get(x).unwrap());
        Context::new(&self.position, &mut culler, camera)
                .draw(iter, phase, stream)
    }
}

impl Renderer {
    pub fn new(graphics: GraphicsSink,
               position: channel::Receiver<Operation<Entity, Solved>>,
               scene: SceneOutput,
               device: Device,
               mut factory: gfx_device_gl::Factory) -> Renderer {

        let pipeline = forward::Pipeline::new(&mut factory);

        Renderer {
            device: device,
            factory: factory,
            graphics: graphics,
            pos_input: position,
            position: Position(HashMap::new()),
            vertex: HashMap::new(),
            materials: HashMap::new(),
            geometry_data: HashMap::new(),
            geometry_slice: HashMap::new(),
            to_draw: HashMap::new(),
            binding: HashMap::new(),
            pipeline: Some(pipeline.unwrap()),
            scenes: HashMap::new(),
            scene_output: scene,
            scene: Scene::new()
        }
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

    fn add_binding(&mut self, entity: Entity, draw: DrawBinding) {
        self.binding.insert(entity, draw);

        match (self.geometry_slice.get(&(draw.0).0), self.materials.get(&(draw.1).0)) {
            (Some(a), Some(b)) => {
                Some(gfx_scene::Entity{
                    name: "".to_string(),
                    visible: true,
                    mesh: a.mesh.clone(),
                    node: entity,
                    skeleton: None,
                    bound: NullBound,
                    fragments: vec![gfx_scene::Fragment{
                        material: b.clone(),
                        slice: a.slice.clone()
                    }]
                })
            }
            _ => None
        }.map(|e| self.to_draw.insert(entity, e));
    }

    fn sync_graphics(&mut self) -> Option<Signal> {
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
                Message::DrawBinding(Operation::Upsert(eid, geo)) => {
                    self.add_binding(eid, geo);
                }
                Message::DrawBinding(Operation::Delete(eid)) => {
                    self.binding.remove(&eid);
                    self.to_draw.remove(&eid);
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
        let mut select: SelectMap<fn(&mut Renderer) -> Option<Signal>> = SelectMap::new();
        select.add(self.graphics.0.signal(), Renderer::sync_graphics);
        select.add(self.pos_input.signal(), Renderer::sync_position);
        select.add(self.scene_output.signal(), Renderer::sync_scene);

        while let Some((_, cb)) = select.next() {
            if let Some(s) = cb(self) {
                select.add(s, cb);
            }
        }

        self.graphics.0.next_frame();
        self.pos_input.next_frame();
        self.scene_output.next_frame();
    }

    /// 
    pub fn draw(&mut self, _: &mut fibe::Schedule, window: &mut Window, scene: Scene) {
        self.scene = scene;
        self.sync();
        let eid = Entity::new();
        let camera = gfx_scene::Camera {
            name: "Cam".to_string(),
            projection: cgmath::PerspectiveFov{
                fovy: cgmath::deg(90.),
                aspect: 4./3.,
                near: 0.1,
                far: 1000.
            },
            node: eid
        };
        self.position.0.insert(eid, Solved(Decomposed::identity()));
        let mut pipeline = self.pipeline.take().unwrap();
        pipeline.render(self, &camera, window).unwrap();
        self.pipeline = Some(pipeline);
        window.present(&mut self.device);
    }
}

