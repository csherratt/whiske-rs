extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate pulse;
extern crate future_pulse;
extern crate image;
#[macro_use]
extern crate gfx;
extern crate shared_future;
extern crate lease;

use std::iter::FromIterator;
use std::collections::HashMap;

use fibe::*;

use entity::*;
use snowstorm::mpsc::*;

pub use material::*;
pub use texture::Texture;
pub use vertex::*;

pub mod material;
pub mod texture;
pub mod vertex;

impl FromIterator<VertexPos> for Vertex {
    fn from_iter<T>(iter: T) -> Vertex where T: IntoIterator<Item=VertexPos> {
        Pos(iter.into_iter().collect())
    }
}

impl FromIterator<VertexPosTex> for Vertex {
    fn from_iter<T>(iter: T) -> Vertex where T: IntoIterator<Item=VertexPosTex> {
        PosTex(iter.into_iter().collect())
    }
}

impl FromIterator<VertexPosNorm> for Vertex {
    fn from_iter<T>(iter: T) -> Vertex where T: IntoIterator<Item=VertexPosNorm> {
        PosNorm(iter.into_iter().collect())
    }
}

impl FromIterator<VertexPosTexNorm> for Vertex {
    fn from_iter<T>(iter: T) -> Vertex where T: IntoIterator<Item=VertexPosTexNorm> {
        PosTexNorm(iter.into_iter().collect())
    }
}

impl VertexBuffer {
    /// Use the entire vertex buffer with the primative as a geometry
    pub fn geometry(&self, primative: Primative) -> GeometryData {
        self.entire().geometry(primative)
    }

    /// Convert the VertexBuffer into a subbuffer the includes the entire
    /// range of the VertexBuffer
    pub fn entire(&self) -> VertexSubBuffer {
        let max = self.length().expect("VertexBuffer was not bound to any buffer. Cannot use as subbuffer.");
        self.subbuffer(0, max)
    }

    /// Convert the VertexBuffer into a subbuffer the includes the entire
    /// Buffers starting at start
    pub fn from(&self, start: u32) -> VertexSubBuffer {
        let max = self.length().expect("VertexBuffer was not bound to any buffer. Cannot use as subbuffer.");
        self.subbuffer(start, max-start)
    }

    /// Convert the VertexBuffer into a subbuffer the includes the entire
    /// Buffers starting at start
    pub fn up_to(&self, end: u32) -> VertexSubBuffer {
        self.subbuffer(0, end)
    }

    /// Use a section of the buffer as a subbuffer
    pub fn subbuffer(&self, start: u32, length: u32) -> VertexSubBuffer {
        let max = self.length().expect("VertexBuffer was not bound to any buffer. Cannot use as subbuffer.");
        assert!(start < max);
        assert!(max >= start + length);

        VertexSubBuffer {
            parent: self.0,
            start: start,
            length: length
        }
    }
}

impl VertexSubBuffer {
    /// Use the entire vertex subbuffer with the primative as a geometry
    pub fn geometry(&self, primative: Primative) -> GeometryData {
        GeometryData {
            buffer: *self,
            primative: primative
        }
    }

    /// Convert the VertexBuffer into a subbuffer the includes the entire
    /// range of the VertexBuffer
    pub fn entire(&self) -> VertexSubBuffer {
        self.subbuffer(0, self.length)
    }

    /// Convert the VertexBuffer into a subbuffer the includes the entire
    /// Buffers starting at start
    pub fn from(&self, start: u32) -> VertexSubBuffer {
        let length = self.length - start;
        self.subbuffer(start, length)
    }

    /// Convert the VertexBuffer into a subbuffer the includes the entire
    /// Buffers starting at start
    pub fn up_to(&self, end: u32) -> VertexSubBuffer {
        self.subbuffer(0, end)
    }

    /// Use a section of the buffer as a subbuffer
    pub fn subbuffer(&self, start: u32, length: u32) -> VertexSubBuffer {
        assert!(start < self.length);
        assert!(start + length < self.length);

        VertexSubBuffer {
            parent: self.parent,
            start: start,
            length: length
        }
    }
}

#[derive(Clone, Debug)]
pub enum VertexComponent {
    Vertex(Vertex),
    Index(Vec<u32>)
}

#[derive(Clone)]
pub enum Message {
    Vertex(Operation<VertexBuffer, VertexComponent>),
    MaterialFlat(Operation<Material, MaterialComponent<[f32; 4]>>),
    MaterialTexture(Operation<Material, MaterialComponent<Texture>>),
    Geometry(Operation<Geometry, GeometryData>),
    Texture(Operation<Texture, image::DynamicImage>)
}


impl WriteEntity<VertexBuffer, Vertex> for Graphics {
    fn write(&mut self, entity: VertexBuffer, data: Vertex) {
        self.send(Message::Vertex(
            Operation::Upsert(entity, VertexComponent::Vertex(data))
        ))
    }
}

impl WriteEntity<VertexBuffer, Vec<u32>> for Graphics {
    fn write(&mut self, entity: VertexBuffer, data: Vec<u32>) {
        self.send(Message::Vertex(
            Operation::Upsert(entity, VertexComponent::Index(data))
        ))
    }
}

impl WriteEntity<VertexBuffer, VertexBufferData> for Graphics {
    fn write(&mut self, entity: VertexBuffer, data: VertexBufferData) {
        let VertexBufferData{vertex, index} = data;
        if index.is_none() {
            self.send(Message::Vertex(Operation::Delete(entity)));
        }
        self.send(Message::Vertex(
            Operation::Upsert(entity, VertexComponent::Vertex(vertex))
        ));
        if let Some(index) = index {
            self.send(Message::Vertex(
                Operation::Upsert(entity, VertexComponent::Index(index))
            ));
        }
    }
}

impl WriteEntity<Material, MaterialComponent<[f32; 4]>> for Graphics {
    fn write(&mut self, entity: Material, data: MaterialComponent<[f32; 4]>) {
        self.send(Message::MaterialFlat(
            Operation::Upsert(entity, data)
        ))
    }
}

impl WriteEntity<Material, MaterialComponent<Texture>> for Graphics {
    fn write(&mut self, entity: Material, data: MaterialComponent<Texture>) {
        self.send(Message::MaterialTexture(
            Operation::Upsert(entity, data)
        ))
    }
}

impl WriteEntity<Geometry, GeometryData> for Graphics {
    fn write(&mut self, entity: Geometry, data: GeometryData) {
        self.send(Message::Geometry(
            Operation::Upsert(entity, data)
        ))
    }
}

impl WriteEntity<Texture, image::DynamicImage> for Graphics {
    fn write(&mut self, entity: Texture, data: image::DynamicImage) {
        self.send(Message::Texture(
            Operation::Upsert(entity, data)
        ))
    }
}

impl ReadEntity<VertexBuffer, Vertex> for GraphicsStore {
    fn read(&self, eid: &VertexBuffer) -> Option<&Vertex> {
        self.vertex_buffer.get(&eid.0).map(|v| &v.vertex)
    }
}

impl ReadEntity<VertexBuffer, Vec<u32>> for GraphicsStore {
    fn read(&self, eid: &VertexBuffer) -> Option<&Vec<u32>> {
        self.vertex_buffer.get(&eid.0).and_then(|v| v.index.as_ref())
    }
}

impl ReadEntity<VertexBuffer, VertexBufferData> for GraphicsStore {
    fn read(&self, eid: &VertexBuffer) -> Option<&VertexBufferData> {
        self.vertex_buffer.get(&eid.0)
    }
}

impl ReadEntity<Geometry, GeometryData> for GraphicsStore {
    fn read(&self, eid: &Geometry) -> Option<&GeometryData> {
        self.geometry.get(eid)
    }
}

impl ReadEntity<Texture, image::DynamicImage> for GraphicsStore {
    fn read(&self, eid: &Texture) -> Option<&image::DynamicImage> {
        self.texture.get(eid)
    }
}


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Flag {
    Updated,
    Deleted
}

#[derive(Clone)]
pub struct GraphicsStore {
    pub vertex_buffer: HashMap<Entity, VertexBufferData>,
    pub vertex_buffer_updated: HashMap<Entity, Flag>,

    pub material: HashMap<Material, HashMap<MaterialKey, Texture>>,
    pub material_updated: HashMap<Material, Flag>,

    pub texture: HashMap<Texture, image::DynamicImage>,
    pub texture_updated: HashMap<Texture, Flag>,

    pub geometry: HashMap<Geometry, GeometryData>,
    pub geometry_updated: HashMap<Geometry, Flag>,

    /// This is used the emulate `flat` colors the color is written
    /// into a texture for eas of use by the backend
    pub colors: HashMap<[u8; 4], Texture>
}

#[derive(Clone)]
pub enum Graphics {
    Valid {
        /// a channel to send graphics data with
        channel: Sender<Message>,

        /// Link the the future of this store
        next: shared_future::Future<Graphics>,

        /// Link to the data associated with this frame
        data: lease::Lease<GraphicsStore>,
    },
    UpdatePending
}

impl Graphics {
    fn send(&mut self, msg: Message) {
        match self {
            &mut Graphics::Valid{ref mut channel, ref next, ref data} => {
                channel.send(msg)
            }
            _ => ()
        }
    }
}

impl std::ops::Deref for Graphics {
    type Target = GraphicsStore;

    fn deref(&self) -> &GraphicsStore {
        match self {
            &Graphics::Valid{ref channel, ref next, ref data} => data,
            _ => panic!("Graphics is being Updated!")
        }
    }
}

impl GraphicsStore {
    fn clear_frame(&mut self) {
        self.vertex_buffer_updated.clear();
        self.material_updated.clear();
        self.texture_updated.clear();
        self.geometry_updated.clear();
    }

    fn upsert_vertex(&mut self, id: VertexBuffer, dat: VertexComponent) {
        self.vertex_buffer_updated.insert(id.0, Flag::Updated);
        let dst = self.vertex_buffer
            .entry(id.0)
            .or_insert_with(|| VertexBufferData{
                vertex: Vertex::Pos(vec![]),
                index: None
            });

        match dat {
            VertexComponent::Vertex(data) => dst.vertex = data,
            VertexComponent::Index(data) => dst.index = Some(data),
        }
    }

    fn delete_vertex(&mut self, v: VertexBuffer) {
        self.vertex_buffer_updated.insert(v.0, Flag::Deleted);
        self.vertex_buffer.delete(v.0);
    }

    fn material_flat(&mut self, id: Material, mat: MaterialComponent<[f32; 4]>) {
        fn v_to_u8(v: f32) -> u8 {
            if v > 1. {
                255
            } else if v < 0. {
                0
            } else {
                (v * 255.) as u8
            }
        }
        let (key, value) = mat.split();
        let value_u8 = [v_to_u8(value[0]),
                        v_to_u8(value[1]),
                        v_to_u8(value[2]),
                        v_to_u8(value[3])];
        let rgba = image::Rgba(value_u8);
        let mut insert = false;
        let texture = *self.colors
            .entry(value_u8)
            .or_insert_with(|| {
                insert = true;
                Texture::new()
            });

        if insert {
            self.texture_updated.insert(texture, Flag::Updated);
            self.texture.insert(texture,
                image::DynamicImage::ImageRgba8(image::ImageBuffer::from_pixel(1, 1, rgba))
            );
        }


        self.material_updated.insert(id, Flag::Updated);
        self.material
            .entry(id)
            .or_insert_with(|| HashMap::new())
            .insert(key, texture);
    }

    fn material_texture(&mut self, id: Material, mat: MaterialComponent<Texture>) {
        let (key, value) = mat.split();
        self.material_updated.insert(id, Flag::Updated);
        self.material
            .entry(id)
            .or_insert_with(|| HashMap::new())
            .insert(key, value);
    }

    fn material_delete(&mut self, id: Material) {
        self.material_updated.insert(id, Flag::Deleted);
        self.material.delete(id);
    }

    fn geometry(&mut self, id: Geometry, dat: GeometryData) {
        self.geometry_updated.insert(id, Flag::Updated);
        self.geometry.insert(id, dat);
    }

    fn geometry_delete(&mut self, id: Geometry) {
        self.geometry_updated.insert(id, Flag::Deleted);
        self.geometry.delete(id);
    }

    fn texture(&mut self, id: Texture, dat: image::DynamicImage) {
        self.texture_updated.insert(id, Flag::Updated);
        self.texture.insert(id, dat);
    }

    fn texture_delete(&mut self, id: Texture) {
        self.texture_updated.insert(id, Flag::Deleted);
        self.texture.delete(id);
    }
}

fn worker(mut owner: lease::Owner<GraphicsStore>,
          mut set: shared_future::Set<Graphics>,
          mut input: Receiver<Message>) {

    loop {
        let mut data = owner.get();
        data.clear_frame();

        for msg in input.iter() {
            match msg {
                Message::Vertex(Operation::Upsert(eid, vd)) => {
                    data.upsert_vertex(eid, vd);
                }
                Message::Vertex(Operation::Delete(eid)) => {
                    data.delete_vertex(eid);
                }
                Message::MaterialFlat(Operation::Upsert(eid, mat)) => {
                    data.material_flat(eid, mat);
                }
                Message::MaterialTexture(Operation::Upsert(eid, mat)) => {
                    data.material_texture(eid, mat);
                }
                Message::MaterialTexture(Operation::Delete(eid)) |
                Message::MaterialFlat(Operation::Delete(eid)) => {
                    data.material_delete(eid);
                }
                Message::Geometry(Operation::Upsert(eid, geo)) => {
                    data.geometry(eid, geo);
                }
                Message::Geometry(Operation::Delete(eid)) => {
                    data.geometry_delete(eid);
                }
                Message::Texture(Operation::Upsert(eid, text)) => {
                    data.texture(eid, text);
                }
                Message::Texture(Operation::Delete(eid)) => {
                    data.texture_delete(eid);
                }

            }
        }

        let (nowner, lease) = lease::lease(data);
        let (tx, ninput) = channel();
        let (next, nset) = shared_future::Future::new();
        set.set(Graphics::Valid{
            channel: tx,
            next: next,
            data: lease
        });
        owner = nowner;
        set = nset;
        input = ninput;
    }
}

impl Graphics {
    pub fn new(sched: &mut fibe::Schedule) -> Graphics {
        let (tx, rx) = channel();
        let (future, set) = shared_future::Future::new();
        let (owner, lease) = lease::lease(GraphicsStore{
            vertex_buffer: HashMap::new(),
            vertex_buffer_updated: HashMap::new(),
            material: HashMap::new(),
            material_updated: HashMap::new(),
            texture: HashMap::new(),
            texture_updated: HashMap::new(),
            geometry: HashMap::new(),
            geometry_updated: HashMap::new(),
            colors: HashMap::new()
        });

        task(|_| worker(owner, set, rx)).start(sched);

        Graphics::Valid {
            channel: tx,
            next: future,
            data: lease
        }
    }

    /// Fetch the next frame
    pub fn next_frame(&mut self) -> bool {
        use std::mem;
        let mut pending = Graphics::UpdatePending;
        mem::swap(&mut pending, self);
        let (mut channel, next, data) = match pending {
            Graphics::Valid{channel, next, data} => (channel, next, data),
            Graphics::UpdatePending => panic!("Invalid state"),
        };
        channel.flush();
        drop(data);
        drop(channel);
        match next.get().ok() {
            Some(next) => {
                *self = next;
                true
            }
            None => false
        }
    }
}
