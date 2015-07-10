
use entity::{Entity, WriteEntity, Append, EntityBinding, DeleteEntity};

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

#[derive(Clone, Debug)]
pub struct VertexBufferData {
    pub vertex: Vertex,
    pub index: Option<Vec<u32>>
}

/// A Geometry entity
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
pub struct Geometry(pub Entity);


/// A handle for a vertex buffer
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
pub struct VertexBuffer(pub Entity, Length);

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

#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
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
    pub buffer: VertexSubBuffer,
    pub primative: Primative
}

/// describe a sub buffer of the parent buffer
/// The parent VertexBuffer will be the SubBuffer's data
#[derive(Copy, Clone, Hash, Debug)]
pub struct VertexSubBuffer {
    pub parent: Entity,
    pub start: u32,
    pub length: u32,
}

gfx_vertex!( VertexPos {
    a_Position@ position: [f32; 3],
});

impl PartialEq for VertexPos {
    fn eq(&self, other: &VertexPos) -> bool {
        self.position == other.position
    }
}

gfx_vertex!( VertexPosNorm {
    a_Position@ position: [f32; 3],
    a_Normal@ normal: [f32; 3],
});

impl PartialEq for VertexPosNorm {
    fn eq(&self, other: &VertexPosNorm) -> bool {
        self.position == other.position &&
        self.normal == other.normal
    }
}

gfx_vertex!( VertexPosTex {
    a_Position@ position: [f32; 3],
    a_Tex0@ texture: [f32; 2],
});

impl PartialEq for VertexPosTex {
    fn eq(&self, other: &VertexPosTex) -> bool {
        self.position == other.position &&
        self.texture == other.texture
    }
}

gfx_vertex!( VertexPosTexNorm {
    a_Position@ position: [f32; 3],
    a_Normal@ normal: [f32; 3],
    a_Tex0@ texture: [f32; 2],
});

impl PartialEq for VertexPosTexNorm {
    fn eq(&self, other: &VertexPosTexNorm) -> bool {
        self.position == other.position &&
        self.texture == other.texture &&
        self.normal == other.normal
    }
}

#[derive(Clone, Debug)]
pub enum Vertex {
    Pos(Vec<VertexPos>),
    PosTex(Vec<VertexPosTex>),
    PosNorm(Vec<VertexPosNorm>),
    PosTexNorm(Vec<VertexPosTexNorm>),
}
pub use self::Vertex::*;

impl Vertex {
    pub fn index(&self, idx: usize) -> ([f32; 3], Option<[f32; 2]>, Option<[f32; 3]>) {
        match self {
            &Pos(ref v) => (v[idx].position, None, None),
            &PosTex(ref v) => (v[idx].position, Some(v[idx].texture), None),
            &PosNorm(ref v) => (v[idx].position, None, Some(v[idx].normal)),
            &PosTexNorm(ref v) => (v[idx].position, Some(v[idx].texture), Some(v[idx].normal)),
        }
    }
}
