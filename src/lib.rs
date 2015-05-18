extern crate parent;
extern crate fibe;
extern crate snowstorm;
extern crate entity;
extern crate pulse;
#[macro_use]
extern crate gfx;

use entity::*;
use snowstorm::channel::*;

/// This holds the binding between a geometry and the material
/// for a drawable entity
#[derive(Copy, Clone, Hash, Debug)]
pub struct DrawBinding(Geometry, Material);

/// A Geometry entity
#[derive(Copy, Clone, Hash, Debug)]
pub struct Geometry(Entity);

/// A Material entity
#[derive(Copy, Clone, Hash, Debug)]
pub struct Material(Entity);

/// A handle for a vertex buffer
#[derive(Copy, Clone, Hash, Debug)]
pub struct VertexBuffer(Entity);

impl VertexBuffer {
    /// Create a vertex buffer
    pub fn new() -> VertexBuffer {
        VertexBuffer(Entity::new())
    }

    /// Binds an a component to the Entity
    pub fn bind<T>(self, data: T) -> EntityBinding<VertexBuffer, (T,)> {
        EntityBinding::new(self, data)
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

#[derive(Clone)]
pub enum Vertex {
    Pos(Vec<VertexPos>),
    PosTex(Vec<VertexPosTex>),
    PosNorm(Vec<VertexPosNorm>),
    PosTexNorm(Vec<VertexPosTexNorm>),
}
pub use Vertex::*;

impl VertexBuffer {
    /// Use the entire vertex buffer with the primative as a geometry
    pub fn geometry(&self, primative: Primative) -> GeometryData {
        self.subbuffer(0, 0xFFFF_FFFF).geometry(primative)
    }

    /// Use a section of the buffer as a subbuffer
    pub fn subbuffer(&self, start: u32, length: u32) -> VertexSubBuffer {
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

    /// Use a section of the buffer as a subbuffer
    pub fn subbuffer(&self, start: u32, length: u32) -> VertexSubBuffer {
        VertexSubBuffer {
            parent: self.parent,
            start: start + self.start,
            length: length
        }
    }
}

pub enum VertexData {
    Vertex(Vertex),
    Index(Vec<u32>)
}

pub enum Message {
    Vertex(Operation<VertexBuffer, VertexData>)
}

pub struct GraphicsSource(Sender<Message>);
pub struct GraphicsSink(Receiver<Message>);

impl GraphicsSource {
    pub fn new() -> (GraphicsSink, GraphicsSource) {
        let (vx_tx, vx_rx) = channel();
        (GraphicsSink(vx_rx), GraphicsSource(vx_tx))
    }
}

impl WriteEntity<VertexBuffer, Vertex> for GraphicsSource {
    fn write(&mut self, entity: VertexBuffer, data: Vertex) {
        self.0.send(Message::Vertex(
            Operation::Upsert(entity, VertexData::Vertex(data))
        ))
    }
}

impl WriteEntity<VertexBuffer, Vec<u32>> for GraphicsSource {
    fn write(&mut self, entity: VertexBuffer, data: Vec<u32>) {
        self.0.send(Message::Vertex(
            Operation::Upsert(entity, VertexData::Index(data))
        ))
    }
}