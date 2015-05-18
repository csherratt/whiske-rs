
extern crate graphics;

use graphics::*;

#[test]
fn create_vb() {
    let (_, mut source) = GraphicsSource::new();
    let vb = VertexBuffer::new()
                          .bind(Pos(Vec::new()))
                          .bind(Vec::new())
                          .write(&mut source);
}