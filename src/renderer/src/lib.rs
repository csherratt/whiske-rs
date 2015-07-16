extern crate entity;
extern crate transform;
extern crate graphics;
extern crate scene;
extern crate snowstorm;
extern crate fibe;
extern crate bounding;
extern crate hprof;
extern crate genmesh;
extern crate lease;
extern crate shared_future;
extern crate system;

#[macro_use]
extern crate gfx;
extern crate gfx_phase;
extern crate gfx_scene;
extern crate gfx_device_gl;
extern crate gfx_pipeline;
extern crate gfx_text;
#[cfg(feature="virtual_reality")]
extern crate gfx_vr;
extern crate gfx_scene_aabb_debug;

extern crate engine;
extern crate draw_queue;
extern crate pulse;
extern crate cgmath;
extern crate image;

#[cfg(feature="virtual_reality")]
extern crate vr;

mod render_data;

use std::collections::{HashMap, HashSet};
use transform::TransformSystem;
use graphics::{
    Graphics, Texture, Geometry,
    Pos, PosTex, PosNorm, PosTexNorm,
};
use scene::{Scene, SceneSystem};
use engine::Window;
use entity::Entity;

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
use cgmath::{Transform, AffineMatrix3, Matrix4, Aabb3};

#[cfg(feature="virtual_reality")]
use cgmath::Matrix;

pub use render_data::{DrawBinding, Camera, Primary, DebugText, Renderer};

struct GeometrySlice<R: Resources> {
    mesh: Mesh<R>,
    slice: Slice<R>
}

struct Globals {
    graphics: graphics::Graphics,
    transform: TransformSystem,
    scenes: SceneSystem,    
    bounding: bounding::Bounding,
    render: Renderer,
}

struct GfxData<R: Resources> {
    vertex: HashMap<Entity, (Mesh<R>, Option<handle::Buffer<R, u32>>)>,
    materials: HashMap<graphics::Material, Material<R>>,
    geometry_slice: HashMap<Geometry, GeometrySlice<R>>,
    textures: HashMap<Texture, handle::Texture<R>>,    
    sampler: gfx::handle::Sampler<R>,
}

pub struct RendererSystem<R: Resources, C: gfx::CommandBuffer<R>, D: gfx::Device, F: Factory<R>> {
    device: D,
    factory: F,

    // Tempoary holding place for the input data
    globals: Option<Globals>,
    gfx_data: Option<GfxData<R>>,

    pipeline: Option<forward::Pipeline<R>>,

    // debug
    text: gfx_text::Renderer<R, F>,
    aabb_debug: gfx_scene_aabb_debug::AabbRender<R>,

    #[cfg(feature="virtual_reality")]
    ivr: Option<vr::IVRSystem>,
    #[cfg(feature="virtual_reality")]
    gvr: Option<gfx_vr::Render<R, C>>,

    #[cfg(not(feature="virtual_reality"))]
    phantom: std::marker::PhantomData<C>
}

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
    aabb: Aabb3<f32>,
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
    type Bound = Aabb3<f32>;

    fn get_bound(&self) -> Aabb3<f32> { self.aabb }
    fn get_mesh(&self) -> &gfx::Mesh<R> { &self.mesh }
    fn get_fragments(&self) -> &[gfx_scene::Fragment<R, M>] { &self.fragments[..] }
}

struct RenderContext<R: Resources>{
    scene: Scene,
    local: GfxData<R>,
    globals: Globals
}


impl<R> AbstractScene<R> for RenderContext<R>
    where R: Resources
{
    type ViewInfo = gfx_pipeline::ViewInfo<f32>;
    type Material = Material<R>;
    type Camera = MaterializedCamera;
    type Status = Report;


    fn draw<H, S>(&self,
                  phase: &mut H,
                  camera: &MaterializedCamera,
                  stream: &mut S) -> Result<Self::Status, Error> where
        H: gfx_phase::AbstractPhase<R, Material<R>, gfx_pipeline::ViewInfo<f32>>,
        S: gfx::Stream<R>,
    
    {   
        let mut culler = Frustum::new();
        let empty = HashSet::new();
        let drawlist = self.globals.scenes.scene_entities(self.scene)
                                          .unwrap_or_else(|| &empty);
        let items: Vec<MaterializedEntity<R, Material<R>>> =
            drawlist.iter()
                    .filter_map(|eid| self.globals.render.binding.get(eid).map(|x| (eid, x)))
                    .filter_map(|(eid, draw)| {

            match (self.local.geometry_slice.get(&draw.0),
                   self.local.materials.get(&(draw.1)),
                   self.globals.transform.world(*eid)) {
                (Some(a), Some(b), Some(c)) => {
                    Some(MaterializedEntity{
                        aabb: self.globals.bounding.aabb[&draw.0],
                        transform: AffineMatrix3{mat: (*c).into()},
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

        let res = Context::new(&mut culler, camera)
            .draw(items.iter(), phase, stream);


        //self.aabb_debug.render(items.iter(), camera, stream);
        res
    }
}

impl<F> RendererSystem<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, Device, F>
    where F: gfx::Factory<gfx_device_gl::Resources>+Clone

{
    #[cfg(feature="virtual_reality")]
    pub fn new(sched: &mut fibe::Schedule,
               graphics: Graphics,
               position: TransformOutput,
               scene: SceneOutput,
               bounding: bounding::Bounding,
               ra: engine::RenderArgs<Device, F>) -> (Renderer, RendererSystem<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, Device, F>) {

        use gfx::tex::WrapMode::Tile;

        let (device, mut factory, vr) = (ra.device, ra.factory, ra.vr);

        let mut pipeline = forward::Pipeline::new(&mut factory).unwrap();
        pipeline.background = Some([0., 0., 0., 1.]);
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

        let aabb_debug = gfx_scene_aabb_debug::AabbRender::new(&mut factory).unwrap();
        let gfx_vr = vr.as_ref().map(|vr| gfx_vr::Render::new(&mut factory, vr));

        let render = Renderer::new(sched);

        (render.clone(),
         RendererSystem {
            device: device,
            factory: factory,
            position: Position(HashMap::new()),
            vertex: HashMap::new(),
            materials: HashMap::new(),
            geometry_slice: HashMap::new(),
            pipeline: Some(pipeline),
            scenes: HashMap::new(),
            scene: Scene::new(),
            textures: HashMap::new(),
            sampler: sampler,
            text: text,
            ivr: vr,
            gvr: gfx_vr,
            aabb_debug: aabb_debug
        })
    }

    #[cfg(not(feature="virtual_reality"))]
    pub fn new(sched: &mut fibe::Schedule,
               graphics: Graphics,
               transform: TransformSystem,
               scenes: SceneSystem,
               bounding: bounding::Bounding,
               ra: engine::RenderArgs<Device, F>) -> (Renderer, RendererSystem<gfx_device_gl::Resources, gfx_device_gl::CommandBuffer, Device, F>) {

        use gfx::tex::WrapMode::Tile;
        let (device, mut factory) = (ra.device, ra.factory);

        let mut pipeline = forward::Pipeline::new(&mut factory).unwrap();
        pipeline.background = Some([0., 0., 0., 1.]);
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

        let aabb_debug = gfx_scene_aabb_debug::AabbRender::new(&mut factory).unwrap();
        let render = render_data::renderer(sched);

        let globals = Globals{
            transform: transform,
            graphics: graphics,
            scenes: scenes,
            bounding: bounding,
            render: render.clone()
        };

        let gfx_data = GfxData{
            vertex: HashMap::new(),
            materials: HashMap::new(),
            geometry_slice: HashMap::new(),
            textures: HashMap::new(),
            sampler: sampler,
        };

        (render,
         RendererSystem {
            globals: Some(globals),
            gfx_data: Some(gfx_data),
            device: device,
            factory: factory,
            pipeline: Some(pipeline),
            text: text,
            aabb_debug: aabb_debug,
            phantom: std::marker::PhantomData
        })
    }

}

fn update_vertex_buffer<R, F>(factory: &mut F,
                              graphics: &Graphics,
                              table: &mut HashMap<Entity, (Mesh<R>, Option<handle::Buffer<R, u32>>)>,
                              id: Entity)
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
    table.insert(id, (vertex, index));
}


impl<R: Resources> GfxData<R> {
    fn add_material_texture(&mut self, graphics: &Graphics, entity: graphics::Material) {
        let dst = self.materials.entry(entity)
                      .or_insert_with(|| {
                       Material {
                            color: [1., 1., 1., 1.],
                            texture: None,
                            transparency: Transparency::Opaque
                       }});

        for (&id, &mat) in graphics.material.get(&entity).unwrap().iter() {
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
    fn add_texture<F>(&mut self,
                   id: Texture,
                   texture: &image::DynamicImage,
                   factory: &mut F)
        where F: Factory<R>
    {

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

        let text = factory.create_texture(tinfo)
                          .ok().expect("Failed to create texture");
        let img_info = (*text.get_info()).into();
        factory.update_texture(
            &text,
            &img_info,
            &texture.raw_pixels()[..],
            Some(gfx::tex::Kind::D2)
        ).unwrap();

        self.textures.insert(id, text);
    }

    fn update_geometry(&mut self, graphics: &Graphics, id: Geometry) {
        let geometry = graphics.geometry.get(&id).unwrap().clone();

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

    fn update_with_graphics<F>(&mut self, graphics: &Graphics, factory: &mut F)
        where F: Factory<R>
    {
        let _g = hprof::enter("vertex_buffer");
        for (&id, &msg) in graphics.vertex_buffer_updated.iter() {
            println!("Updating {:?}", id);
            match msg {
                graphics::Flag::Updated => {
                    update_vertex_buffer(factory, &graphics, &mut self.vertex, id);
                },
                graphics::Flag::Deleted => {}
            }
        }
        drop(_g);

        let _g = hprof::enter("texture_updated");
        for (&id, &msg) in graphics.texture_updated.iter() {
            println!("Updating {:?}", id);
            match msg {
                graphics::Flag::Updated => {
                    self.add_texture(id, graphics.texture.get(&id).unwrap(), factory);
                },
                graphics::Flag::Deleted => {}
            }            
        }
        drop(_g);

        let _g = hprof::enter("material_updated");
        for (&id, &msg) in graphics.material_updated.iter() {
            println!("Updating {:?}", id);
            match msg {
                graphics::Flag::Updated => {
                    self.add_material_texture(graphics, id);
                },
                graphics::Flag::Deleted => {}
            }
        }
        drop(_g);

        let _g = hprof::enter("geometry_updated");
        for (&id, &msg) in graphics.geometry_updated.iter() {
            println!("Updating {:?}", id);
            match msg {
                graphics::Flag::Updated => {
                    self.update_geometry(graphics, id);
                },
                graphics::Flag::Deleted => {}
            }
        }
        drop(_g);        
    }

    fn sync<F>(&mut self, globals: Globals, factory: &mut F) -> Globals
        where F: Factory<R>
    {
        let Globals{
            mut graphics,
            scenes,
            transform,
            mut bounding,
            render
        } = globals;

        let scenes = scenes.next_frame_async();
        let transform = transform.next_frame_async();
        let render = render.next_frame_async();

        let _g = hprof::enter("graphics-fetch");
        graphics.next_frame();
        drop(_g);

        self.update_with_graphics(&graphics, factory);

        let _g = hprof::enter("bounding-fetch");
        bounding.next_frame();
        drop(_g);

        let _g = hprof::enter("scenes-fetch");
        let scenes = scenes.get().unwrap();
        drop(_g);

        let _g = hprof::enter("transform-fetch");
        let transform = transform.get().unwrap();
        drop(_g);

        let _g = hprof::enter("render-fetch");
        let render = render.get().unwrap();
        drop(_g);

        Globals {
            graphics: graphics,
            scenes: scenes,
            transform: transform,
            bounding: bounding,
            render: render
        }
    }

}


impl<R, C, D, F> RendererSystem<R, C, D, F>
    where R: Resources,
          C: gfx::CommandBuffer<R>,
          D: gfx::Device<Resources=R, CommandBuffer=C>,
          F: gfx::Factory<R>+Clone

{
    #[cfg(feature="virtual_reality")]
    pub fn draw(&mut self, _: &mut fibe::Schedule, window: &mut Window<D, R>) {
        hprof::start_frame();
        let _g = hprof::enter("sync system");
        self.sync();
        drop(_g);

        let camera = if let Some(cid) = self.render.primary {
            if let Some(c) = self.render.cameras.get(&cid) {
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

            for (_, text) in self.render.debug_text.iter() {
                self.text.add(
                    &text.text, text.start, text.color
                );
            }
            self.text.draw(window).unwrap();
            let _g = hprof::enter("present");
            window.present(&mut self.device);
            drop(_g);
            hprof::end_frame();
            hprof::profiler().print_timing();
        }
    }

    #[cfg(not(feature="virtual_reality"))]
    pub fn draw(&mut self, _: &mut fibe::Schedule, window: &mut Window<D, R>) {
        let mut globals = self.globals.take().unwrap();
        let mut gfx_data = self.gfx_data.take().unwrap();

        hprof::start_frame();
        let _g = hprof::enter("sync");
        globals = gfx_data.sync(globals, &mut self.factory);
        drop(_g);

        let camera = if let Some(cid) = globals.render.primary {
            if let Some(c) = globals.render.cameras.get(&cid) {
                Some((MaterializedCamera {
                    projection: c.0.clone().into(),
                    transform: globals.transform
                                      .world(cid)
                                      .map(|&x| AffineMatrix3{mat: x.into()})
                                      .unwrap_or_else(|| AffineMatrix3::identity())
                }, c.1))
            } else {
                None
            }
        } else {
            None
        };

        if let Some((camera, scene)) = camera {
            let rc = RenderContext{
                scene: scene,
                local: gfx_data,
                globals: globals
            };

            let mut pipeline = self.pipeline.take().unwrap();
            pipeline.render(&rc, &camera, window).unwrap();
            self.pipeline = Some(pipeline);

            for (_, text) in rc.globals.render.debug_text.iter() {
                self.text.add(
                    &text.text, text.start, text.color
                );
            }
            self.text.draw(window).unwrap();
            let _g = hprof::enter("present");
            window.present(&mut self.device);
            drop(_g);
            hprof::end_frame();
            hprof::profiler().print_timing();

            self.globals = Some(rc.globals);
            self.gfx_data = Some(rc.local);
        } else {
            self.globals = Some(globals);
            self.gfx_data = Some(gfx_data);   
        }

    }
}


