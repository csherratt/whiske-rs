//   Copyright 2014 Colin Sherratt
//
//   Licensed under the Apache License, Version 2.0 (the "License");
//   you may not use this file except in compliance with the License.
//   You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
//   Unless required by applicable law or agreed to in writing, software
//   distributed under the License is distributed on an "AS IS" BASIS,
//   WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//   See the License for the specific language governing permissions and
//   limitations under the License.

use entity::*;
//use Texture;

/// A Material entity
#[derive(Copy, Clone, Hash, Debug)]
pub struct Material(pub Entity);

impl Material {
    /// Creates a new entity with a new id
    pub fn new() -> Material {
        Material(Entity::new())
    }

    /// Binds an a component to the Material
    pub fn bind<T>(self, data: T) -> EntityBinding<Material, (T,)> {
        EntityBinding::new(self, data)
    }

    /// Delete this entity from a device
    pub fn delete<D>(&self, delete: &mut D) where D: DeleteEntity<Material> {
        delete.delete(*self);
    }
}

#[derive(Copy, Clone, Debug)]
pub enum MaterialComponent {
    KaFlat([f32; 3]),
    KdFlat([f32; 3]),
    KsFlat([f32; 3]),
    /*Ka(Texture),
    Kd(Texture),
    Ks(Texture)*/
}

pub use self::MaterialComponent::*;
