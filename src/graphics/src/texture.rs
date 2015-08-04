
use std::path::PathBuf;
use entity::{Entity, EntityBinding};
use engine::fibe::{Schedule, task};
use image;
use Graphics;
use future_pulse::Future;

/// A handle for a texture
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
pub struct Texture(pub Entity);

impl Texture {
    /// Create a new texture entity
    pub fn new() -> Texture {
        Texture(Entity::new())
    }

    /// Load a image from Path
    pub fn load(sched: &mut Schedule, path: PathBuf, mut src: Graphics)
        -> Future<Result<Texture, image::ImageError>> {
       
        task(move |_| {
            image::open(path)
                .map(|image| {
                    Texture::new().bind(image).write(&mut src)
                })
        }).start(sched)
    }

    /// Bind some data to the Texture
    pub fn bind<T>(self, data: T) -> EntityBinding<Texture, (T,)> {
        EntityBinding::new(self, data)
    }
}