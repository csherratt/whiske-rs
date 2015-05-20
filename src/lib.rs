extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate pulse;
#[macro_use]
extern crate gfx;

use std::iter::FromIterator;
use entity::*;
use snowstorm::mpsc::*;
pub use material::*;

pub mod material;

/// This holds the binding between a geometry and the material
/// for a drawable entity
#[derive(Copy, Clone, Hash, Debug)]
pub struct DrawBinding(pub Geometry, pub Material);

/// A Geometry entity
#[derive(Copy, Clone, Hash, Debug)]
pub struct Geometry(Entity);


/// A handle for a vertex buffer
#[derive(Copy, Clone, Hash, Debug)]
pub struct VertexBuffer(pub Entity, Length);

/// A handle for a texture
//#[derive(Copy, Clone, Hash, Debug)]
//pub struct Texture(pub Entity);

impl Geometry {
    /// Creates a new entity with a new id
    pub fn new() -> Geometry {
        Geometry(Entity::new())
    }

    /// Binds an a component to the Geometry
    pub fn bind<T>(self, data: T) -> EntityBinding<Geometry, (T,)> {
        EntityBinding::new(self, data)
    }

    /// Delete this entity from a device
    pub fn delete<D>(&self, delete: &mut D) where D: DeleteEntity<Geometry> {
        delete.delete(*self);
    }
}

#[derive(Copy, Clone, Hash, Debug)]
pub enum Length {
    Unsized,
    Length(u32)
}

impl VertexBuffer {
    /// Create a vertex buffer
    pub fn new() -> VertexBuffer {
        VertexBuffer(Entity::new(), Length::Unsized)
    }

    /// Binds an a component to the Entity
    pub fn bind(mut self, data: Vertex) -> VertexBufferBinding<(Vertex,)> {
        self.1 = data.length();
        VertexBufferBinding::new(self, data)
    }

    /// Get the length of a vertex buffer, Returns
    /// None if the vertex buffer has no associated length
    pub fn length(&self) -> Option<u32> {
        match self.1 {
            Length::Unsized => None,
            Length::Length(x) => Some(x)
        }
    }
}

/// A Entity & Some data that is associated with it
#[derive(Copy, Clone)]
pub struct VertexBufferBinding<T> {
    entity: VertexBuffer,
    data: T
}

impl<T> VertexBufferBinding<(T,)> {
    pub fn new(entity: VertexBuffer, data: T) -> VertexBufferBinding<(T,)> {
        VertexBufferBinding {
            entity: entity,
            data: (data,)
        }
    }
}

impl<T> VertexBufferBinding<T> {
    /// Bind an additional component to the VertexBufferBinding
    #[inline]
    pub fn bind_index<O>(mut self, data: Vec<u32>) -> VertexBufferBinding<O>
        where T: Append<Vec<u32>, Output=O> {
        self.entity.1 = data.length();
        VertexBufferBinding {
            entity: self.entity,
            data: self.data.append(data)
        }
    }
}

impl<A> VertexBufferBinding<(A,)> {
    pub fn write<W>(self, sink: &mut W) -> VertexBuffer
        where W: WriteEntity<VertexBuffer, A>{

        sink.write(self.entity, self.data.0);
        self.entity
    }
}

impl<A, B> VertexBufferBinding<(A, B)> {
    pub fn write<W>(self, sink: &mut W) -> VertexBuffer
        where W: WriteEntity<VertexBuffer, A>+WriteEntity<VertexBuffer, B> {

        let (a, b) = self.data;
        sink.write(self.entity, a);
        sink.write(self.entity, b);
        self.entity
    }
}


#[derive(Clone, Copy, Debug, Hash)]
pub enum Primative {
    Point,
    Line,
    Triangle,
    TriangleAdjacency
}

/// describe geometry
#[derive(Copy, Clone, Hash, Debug)]
pub struct GeometryData {
    buffer: VertexSubBuffer,
    primative: Primative
}

/// describe a sub buffer of the parent buffer
/// The parent VertexBuffer will be the SubBuffer's data
#[derive(Copy, Clone, Hash, Debug)]
pub struct VertexSubBuffer {
    parent: Entity,
    start: u32,
    length: u32,
}

gfx_vertex!( VertexPos {
    a_Position@ position: [f32; 3],
});

gfx_vertex!( VertexPosNorm {
    a_Position@ position: [f32; 3],
    a_Normal@ normal: [f32; 3],
});

gfx_vertex!( VertexPosTex {
    a_Position@ position: [f32; 3],
    a_Texture@ texture: [f32; 2],
});

gfx_vertex!( VertexPosTexNorm {
    a_Position@ position: [f32; 3],
    a_Normal@ normal: [f32; 3],
    a_Texture@ texture: [f32; 2],
});

#[derive(Clone, Debug)]
pub enum Vertex {
    Pos(Vec<VertexPos>),
    PosTex(Vec<VertexPosTex>),
    PosNorm(Vec<VertexPosNorm>),
    PosTexNorm(Vec<VertexPosTexNorm>),
}
pub use Vertex::*;

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

// Wrapper to get the length of a buffer
pub trait GetLength {
    fn length(&self) -> Length;
}

impl<T> GetLength for Vec<T> {
    fn length(&self) -> Length {
        Length::Length(self.len() as u32)
    }
}

impl GetLength for Vertex {
    fn length(&self) -> Length {
        Length::Length(match self {
            &Vertex::Pos(ref x)         => x.len(),
            &Vertex::PosTex(ref x)      => x.len(),
            &Vertex::PosNorm(ref x)     => x.len(),
            &Vertex::PosTexNorm(ref x)  => x.len()
        } as u32)
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
pub enum VertexData {
    Vertex(Vertex),
    Index(Vec<u32>)
}

#[derive(Clone, Debug)]
pub enum Message {
    Vertex(Operation<Entity, VertexData>),
    Material(Operation<Entity, MaterialComponent>),
    Geometry(Operation<Entity, GeometryData>),
    DrawBinding(Operation<Entity, DrawBinding>)
}

#[derive(Clone)]
pub struct GraphicsSource(pub Sender<Message>);
pub struct GraphicsSink(pub Receiver<Message>);

impl GraphicsSource {
    pub fn new() -> (GraphicsSink, GraphicsSource) {
        let (vx_tx, vx_rx) = channel();
        (GraphicsSink(vx_rx), GraphicsSource(vx_tx))
    }

    pub fn next_frame(&mut self) {
        self.0.next_frame();
    }
}

impl WriteEntity<VertexBuffer, Vertex> for GraphicsSource {
    fn write(&mut self, entity: VertexBuffer, data: Vertex) {
        self.0.send(Message::Vertex(
            Operation::Upsert(entity.0, VertexData::Vertex(data))
        ))
    }
}

impl WriteEntity<VertexBuffer, Vec<u32>> for GraphicsSource {
    fn write(&mut self, entity: VertexBuffer, data: Vec<u32>) {
        self.0.send(Message::Vertex(
            Operation::Upsert(entity.0, VertexData::Index(data))
        ))
    }
}

impl WriteEntity<Material, MaterialComponent> for GraphicsSource {
    fn write(&mut self, entity: Material, data: MaterialComponent) {
        self.0.send(Message::Material(
            Operation::Upsert(entity.0, data)
        ))
    }
}

impl WriteEntity<Geometry, GeometryData> for GraphicsSource {
    fn write(&mut self, entity: Geometry, data: GeometryData) {
        self.0.send(Message::Geometry(
            Operation::Upsert(entity.0, data)
        ))
    }
}

impl WriteEntity<Entity, DrawBinding> for GraphicsSource {
    fn write(&mut self, entity: Entity, data: DrawBinding) {
        self.0.send(Message::DrawBinding(
            Operation::Upsert(entity, data)
        ))
    }
}
