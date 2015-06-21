
extern crate entity;
extern crate graphics;

use graphics::*;
use entity::*;

#[test]
fn create_vb() {
    let (sink, mut source) = GraphicsSource::new();

    let a = VertexBuffer::new()
        .bind((0..100).map(|_| VertexPos{position: [0., 0., 0.]}).collect())
        .write(&mut source);
    assert_eq!(100, a.length().unwrap());

    let b = VertexBuffer::new()
        .bind((0..100).map(|_| VertexPos{position: [0., 0., 0.]}).collect())
        .bind_index((0..1000).collect())
        .write(&mut source);
    assert_eq!(1000, b.length().unwrap());

    drop(source);
    drop(sink);
}

#[test]
fn material_bind() {
    let (sink, mut source) = GraphicsSource::new();

    let _ = Material::new()
        .bind(Ka([1f32, 2., 3., 1.]))
        .bind(Kd([1f32, 2., 3., 1.]))
        .bind(Ks([1f32, 2., 3., 1.]))
        .write(&mut source);

    drop(source);
    drop(sink);
}

#[test]
fn geometry() {
    let (sink, mut source) = GraphicsSource::new();

    let vb = VertexBuffer::new()
        .bind((0..100).map(|_| VertexPos{position: [0., 0., 0.]}).collect())
        .write(&mut source);

    let geo = Geometry::new()
        .bind(vb.geometry(Primative::Triangle))
        .write(&mut source);

    drop((vb, sink, source, geo));
}

/*#[test]
fn draw_bind() {
    let (sink, mut source) = GraphicsSource::new();

    let vb = VertexBuffer::new()
        .bind((0..100).map(|_| VertexPos{position: [0., 0., 0.]}).collect())
        .write(&mut source);

    let geo = Geometry::new()
        .bind(vb.geometry(Primative::Triangle))
        .write(&mut source);

    let mat = Material::new()
        .bind(Ka([1., 2., 3.]))
        .bind(Kd([1., 2., 3.]))
        .bind(Ks([1., 2., 3.]))
        .write(&mut source);

    let eid = Entity::new()
        .bind(DrawBinding(geo, mat))
        .write(&mut source);

    drop((source, sink, eid));
}*/