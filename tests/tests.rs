
extern crate graphics;

use graphics::*;

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
        .bind(KaFlat([1., 2., 3.]))
        .bind(KdFlat([1., 2., 3.]))
        .bind(KsFlat([1., 2., 3.]))
        .write(&mut source);

    drop(source);
    drop(sink);
}
