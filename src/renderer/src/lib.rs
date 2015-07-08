extern crate entity;
extern crate transform;
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
extern crate gfx_text;
extern crate gfx_vr;

extern crate engine;
extern crate draw_queue;
extern crate pulse;
extern crate cgmath;
extern crate image;
extern crate vr;

use std::collections::{HashMap, HashSet};
use snowstorm::channel;
use transform::{Solved, TransformOutput};
use graphics::{
    Graphics, VertexComponent, Texture,
    Pos, PosTex, PosNorm, PosTexNorm, Vertex,
    MaterialComponent, GeometryData, Geometry,
    VertexBuffer
};
use scene::{Scene, SceneOutput};
use engine::Window;
use pulse::{Signal, SelectMap, Signals};
use entity::{Entity, Operation};

use gfx::{
    Mesh, handle, BufferRole, Factory,
    Slice, PrimitiveType, SliceKind,
};
use gfx::traits::{FactoryExt, Stream};
use gfx_device_gl::{Device};
use gfx_scene::{AbstractScene, Report, Error, Context, Frustum};
use gfx_pipeline::{Material, Transparency, forward, Pipeline};
use gfx::device::Resources;
use image::GenericImage;

use cgmath::{
    Bound, Relation, Transform, BaseFloat, AffineMatrix3,
    Decomposed, Vector3, Quaternion, Matrix4, Matrix
};

struct GeometrySlice<R: Resources> {
    mesh: Mesh<R>,
    slice: Slice<R>
}

#[derive(Copy, Clone, Debug)]
pub struct NullBound;

impl<S: BaseFloat> Bound<S> for NullBound {
    fn relate_plane(&self, _p: &cgmath::Plane<S>) -> Relation {
        Relation::In
    }
}

pub struct Renderer<R: Resources, C: gfx::CommandBuffer<R>, D: gfx::Device, F: Factory<R>> {
    device: D,
    factory: F,

    graphics: graphics::Graphics,
    transform_input: TransformOutput,
    render_input: channel::Receiver<Message>,
    scene_output: SceneOutput,

    position: Position,

    vertex: HashMap<Entity, (Mesh<R>, Option<handle::Buffer<R, u32>>)>,
    materials: HashMap<graphics::Material, Material<R>>,
    geometry_slice: HashMap<Geometry, GeometrySlice<R>>,
    textures: HashMap<Texture, handle::Texture<R>>,
    cameras: HashMap<Entity, Camera>,
    debug_text: HashMap<Entity, DebugText>,

    primary: Option<Entity>,

    binding: HashMap<Entity, DrawBinding>,

    scene: Scene,
    scenes: HashMap<Scene, HashSet<Entity>>,

    pipeline: Option<forward::Pipeline<R>>,

    // debug
    sampler: gfx::handle::Sampler<R>,
    text: gfx_text::Renderer<R, F>,
    ivr: Option<vr::IVRSystem>,
    gvr: Option<gfx_vr::Render<R, C>>

}

pub struct Position(pub HashMap<Entity, Solved>);

pub struct MaterializedCamera {
    transform: AffineMatrix3<f32>,
    projection: Matrix4<f32>
}

impl gfx_scene::Node for MaterializedCamera {
    type Transform = AffineMatrix3<f32>;

    fn get_transform(&self) -> Self::Transform {
        self.transform
    }
}

impl gfx_scene::Camera<f32> for MaterializedCamera {
    type Projection = AffineMatrix3<f32>;

    fn get_projection(&self) -> Self::Projection {
        AffineMatrix3{mat: self.projection}
    }
}

pub struct MaterializedEntity<R: gfx::Resources, M> {
    transform: AffineMatrix3<f32>,
    mesh: gfx::Mesh<R>,
    fragments: [gfx_scene::Fragment<R, M>; 1]

}

impl<R: gfx::Resources, M> gfx_scene::Node for MaterializedEntity<R, M> {
    type Transform = AffineMatrix3<f32>;

    fn get_transform(&self) -> AffineMatrix3<f32> {
        self.transform
    }
}

impl<R: gfx::Resources, M> gfx_scene::Entity<R, M> for MaterializedEntity<R, M> {
    type Bound = NullBound;

    fn get_bound(&self) -> NullBound { NullBound }
    fn get_mesh(&self) -> &gfx::Mesh<R> { &self.mesh }
    fn get_fragments(&self) -> &[gfx_scene::Fragment<R, M>] { &self.fragments[..] }
}

impl<R, C, D, F> AbstractScene<R> for Renderer<R, C, D, F>
    where R: Resources,
          C: gfx::CommandBuffer<R>,
          D: gfx::Device,
          F: Factory<R>
{
    type ViewInfo = gfx_pipeline::ViewInfo<f32>;
    type Material = Material<R>;
    type Camera = MaterializedCamera;
    type Status = Report;


    #[inline(never)]
    fn draw<H, S>(&self,
                  phase: &mut H,
                  camera: &MaterializedCamera,
                  stream: &mut S) -> Result<Self::Status, Error> where
        H: gfx_phase::AbstractPhase<R, Material<R>, gfx_pipeline::ViewInfo<f32>>,
        S: gfx::Stream<R>,
    
    {   
        let mut culler = Frustum::new();
        let empty = HashSet::new();
        let drawlist = self.scenes.get(&self.scene)
                                  .unwrap_or_else(|| &empty);
        let items: Vec<MaterializedEntity<R, Material<R>>> =
            drawlist.iter()
                    .filter_map(|eid| self.binding.get(eid).map(|x| (eid, x)))
                    .filter_map(|(eid, draw)| {

            match (self.geometry_slice.get(&draw.0),
                   self.materials.get(&(draw.1)),
                   self.position.0.get(&eid)) {
                (Some(a), Some(b), Some(c)) => {
                    Some(MaterializedEntity{
                        transform: AffineMatrix3{mat: c.0.into()},
                        mesh: a.mesh.clone(),
                        fragments: [gfx_scene::Fragment{
                            material: b.clone(),
                            slice: a.slice.clone()
                        }]
                    })
                }
                _ => None

            }
        }).collect();

        Context::new(&mut culler, camera)
                .draw(items.iter(), phase, stream)
    }
}

#[derive(Clone)]
pub enum Message {
    Binding(Operation<Entity, DrawBinding>),
    Camera(Operation<Entity, Camera>),
    Slot(Operation<Entity, Primary>),
    DebugText(Operation<Entity, DebugText>)
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

impl entity::WriteEntity<Entity, DebugText> for RendererInput {
    fn write(&mut self, eid: Entity, value: DebugText) {
        self.0.send(Message::DebugText(Operation::Upsert(eid, value)))
    }
}

impl<F> Renderer<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, Device, F>
    where F: gfx::Factory<gfx_device_gl::Resources>+Clone

{
    pub fn new(graphics: Graphics,
               position: TransformOutput,
               scene: SceneOutput,
               ra: engine::RenderArgs<Device, F>) -> (RendererInput, Renderer<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, Device, F>) {

        use gfx::tex::WrapMode::Tile;

        let (device, mut factory, vr) = (ra.device, ra.factory, ra.vr);

        let mut pipeline = forward::Pipeline::new(&mut factory).unwrap();
        pipeline.background = Some([1.0; 4]);
        pipeline.phase.technique.lights = vec![
            gfx_pipeline::Light{
                active: true,
                kind: gfx_pipeline::light::Kind::Omni,
                color: [1., 1., 1., 1.],
                attenuation: gfx_pipeline::light::Attenuation::Spherical{
                    intensity: 1.,
                    distance: 1000., 
                },
                position: cgmath::Vector4::new(1., 1., 1., 1.,)

            }
        ];
        let (tx, rx) = channel::channel();

        let text = gfx_text::new(factory.clone()).unwrap();
        let sampler = factory.create_sampler(
            gfx::tex::SamplerInfo{
                filtering: gfx::tex::FilterMethod::Mipmap,
                wrap_mode: (Tile, Tile, Tile),
                lod_bias: 0.,
                lod_range: (0., 10.),
                comparison: None
            }
        );


        let gfx_vr = vr.as_ref().map(|vr| gfx_vr::Render::new(&mut factory, vr));

        (RendererInput(tx),
         Renderer {
            device: device,
            factory: factory,
            graphics: graphics,
            transform_input: position,
            render_input: rx,
            position: Position(HashMap::new()),
            vertex: HashMap::new(),
            materials: HashMap::new(),
            geometry_slice: HashMap::new(),
            binding: HashMap::new(),
            debug_text: HashMap::new(),
            pipeline: Some(pipeline),
            scenes: HashMap::new(),
            scene_output: scene,
            scene: Scene::new(),
            primary: None,
            cameras: HashMap::new(),
            textures: HashMap::new(),
            sampler: sampler,
            text: text,
            ivr: vr,
            gvr: gfx_vr
        })
    }
}

fn update_vertex_buffer<R, F>(factory: &mut F,
                              graphics: &Graphics,
                              table: &mut HashMap<Entity, (Mesh<R>, Option<handle::Buffer<R, u32>>)>,
                              id: VertexBuffer)
    where R: gfx::Resources,
          F: gfx::Factory<R>
{
    let (vertex, index) = {
        let v = graphics.vertex_buffer.get(&id).unwrap();
        let vertex = match v.vertex {
            Pos(ref data) => factory.create_mesh(&data[..]),
            PosTex(ref data) => factory.create_mesh(&data[..]),
            PosNorm(ref data) => factory.create_mesh(&data[..]),
            PosTexNorm(ref data) => factory.create_mesh(&data[..])
        };
        let index = v.index.as_ref().map(|data|
            factory.create_buffer_static(&data, BufferRole::Index)
        );
        (vertex, index)
    };
    table.insert(id.0, (vertex, index));
}

impl<R, C, D, F> Renderer<R, C, D, F>
    where R: Resources,
          C: gfx::CommandBuffer<R>,
          D: gfx::Device<Resources=R, CommandBuffer=C>,
          F: gfx::Factory<R>+Clone

{

    fn add_material_texture(&mut self, entity: graphics::Material) {
        let dst = self.materials.entry(entity)
                      .or_insert_with(|| {
                       Material {
                            color: [1., 1., 1., 1.],
                            texture: None,
                            transparency: Transparency::Opaque
                       }});

        for (&id, &mat) in self.graphics.material.get(&entity).unwrap().iter() {
            match id {
                graphics::Kd(_) => {
                    let text = if let Some(text) = self.textures.get(&mat) {
                        text.clone()
                    } else {
                        println!("Texture not found");
                        return;
                    };

                    dst.texture = Some((text, Some(self.sampler.clone())));
                }
                _ => ()
            }
        }
    }

    /// load target texture into graphics memory, and refer to it by the supplied
    /// entity id
    fn add_texture(&mut self,
                   id: Texture,
                   texture: &image::DynamicImage) {

        // Flip the image
        //let texture = texture.flipv();

        let format = match texture.color() {
            image::RGB(8) => {
                gfx::tex::Format::Unsigned(
                    gfx::tex::Components::RGB,
                    8,
                    gfx::attrib::IntSubType::Normalized
                )
            }
            image::RGBA(8) => {
                gfx::tex::Format::Unsigned(
                    gfx::tex::Components::RGBA,
                    8,
                    gfx::attrib::IntSubType::Normalized
                )
            }
            _ => {
                println!("Unsupported format {:?}", texture.color());
                return;
            }
        };

        let (width, height) = texture.dimensions();
        let tinfo = gfx::tex::TextureInfo {
            width: width as u16,
            height: height as u16,
            depth: 1,
            levels: 1,
            kind: gfx::tex::Kind::D2,
            format: format,
        };

        let text = self.factory
                       .create_texture(tinfo)
                       .ok().expect("Failed to create texture");
        let img_info = (*text.get_info()).into();
        self.factory.update_texture(
            &text,
            &img_info,
            &texture.raw_pixels()[..],
            Some(gfx::tex::Kind::D2)
        ).unwrap();

        self.textures.insert(id, text);
    }

    fn update_geometry(&mut self, id: Geometry) {
        let geometry = self.graphics.geometry.get(&id).unwrap().clone();

        match self.vertex.get(&geometry.buffer.parent) {
            Some(&(ref v, None)) => {
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
            Some(&(ref v, Some(ref i))) => {
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
            self.geometry_slice.insert(id, slice);
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
                Message::DebugText(Operation::Upsert(eid, text)) => {
                    self.debug_text.insert(eid, text);
                }
                Message::DebugText(Operation::Delete(eid)) => {
                    self.debug_text.remove(&eid);
                }
            }
        }

        if self.render_input.closed() {
            None
        } else {
            Some(self.render_input.signal())
        }
    }

    fn sync_position(&mut self) -> Option<Signal> {
        for msg in self.transform_input.copy_iter(false) {
            msg.write(&mut self.position.0);
        }

        if self.transform_input.closed() {
            None
        } else {
            Some(self.transform_input.signal())
        }
    }

    fn sync_scene(&mut self) -> Option<Signal> {
        self.scene_output.write_into(&mut self.scenes)
    }

    fn sync(&mut self) {
        self.graphics.next_frame();

        let graphics = self.graphics.clone();
        for (&id, &msg) in graphics.vertex_buffer_updated.iter() {
            match msg {
                graphics::Flag::Updated => {
                    update_vertex_buffer(&mut self.factory, &graphics, &mut self.vertex, id);
                },
                graphics::Flag::Deleted => {}
            }
        }
        for (&id, &msg) in graphics.texture_updated.iter() {
            match msg {
                graphics::Flag::Updated => {
                    self.add_texture(id, graphics.texture.get(&id).unwrap());
                },
                graphics::Flag::Deleted => {}
            }            
        }
        for (&id, &msg) in graphics.material_updated.iter() {
            match msg {
                graphics::Flag::Updated => {
                    self.add_material_texture(id);
                },
                graphics::Flag::Deleted => {}
            }
        }
        for (&id, &msg) in graphics.geometry_updated.iter() {
            match msg {
                graphics::Flag::Updated => {
                    self.update_geometry(id);
                },
                graphics::Flag::Deleted => {}
            }
        }



        let mut select: SelectMap<fn(&mut Renderer<R, C, D, F>) -> Option<Signal>> = SelectMap::new();
        select.add(self.transform_input.signal(), Renderer::sync_position);
        select.add(self.scene_output.signal(), Renderer::sync_scene);
        select.add(self.render_input.signal(), Renderer::sync_binding);

        while let Some((_, cb)) = select.next() {
            if let Some(s) = cb(self) {
                select.add(s, cb);
            }
        }

        self.transform_input.next_frame();
        self.scene_output.next_frame();
        self.render_input.next_frame();
    }

    /// 
    pub fn draw(&mut self, _: &mut fibe::Schedule, window: &mut Window<D, R>) {
        self.sync();

        let camera = if let Some(cid) = self.primary {
            if let Some(c) = self.cameras.get(&cid) {
                Some((MaterializedCamera {
                    projection: c.0.clone().into(),
                    transform: self.position.0
                                   .get(&cid)
                                   .map(|x| AffineMatrix3{mat: x.0.into()})
                                   .unwrap_or_else(|| AffineMatrix3::identity())
                }, c.1))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((mut camera, scene)) = camera {
            self.scene = scene;

            let mut pipeline = self.pipeline.take().unwrap();
            let ivr = self.ivr.take();
            let mut gvr = self.gvr.take();

            match (&ivr, &mut gvr) {
                (&Some(ref ivr), &mut Some(ref mut gvr)) => {
                    let old = camera.transform.mat;
                    gvr.render_into(&ivr, |s, p, v| {
                        camera.projection = p;
                        camera.transform.mat = old.mul_m(&v.invert().unwrap());
                        pipeline.render(self, &camera, s).unwrap();
                    });
                    gvr.render_frame(&ivr, &mut self.device, window);
                }
                _ => {
                    pipeline.render(self, &camera, window).unwrap();
                }
            }
            self.pipeline = Some(pipeline);
            self.ivr = ivr;
            self.gvr = gvr;

            for (_, text) in self.debug_text.iter() {
                self.text.add(
                    &text.text, text.start, text.color
                );
            }
            self.text.draw(window).unwrap();
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

#[derive(Clone, Debug)]
pub struct DebugText{
    pub text: String,
    pub start: [i32; 2],
    pub color: [f32; 4]
}


