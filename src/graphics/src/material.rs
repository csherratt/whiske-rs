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
#[derive(Copy, Clone, Hash, Debug, PartialEq, Eq)]
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

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
pub enum MaterialComponent<T> {
    Ka(T),
    Kd(T),
    Ks(T),
}

impl<T> MaterialComponent<T> {
    pub fn split(self) -> (MaterialKey, T) {
        match self {
            Ka(t) => (Ka(()), t),
            Kd(t) => (Kd(()), t),
            Ks(t) => (Ks(()), t),
        }
    }

    pub fn value(self) -> T {
        match self {
            Ka(a) | Kd(a) | Ks(a) => a
        }
    }

    pub fn key(self) -> MaterialKey {
        match self {
            Ka(_) => Ka(()),
            Kd(_) => Kd(()),
            Ks(_) => Ks(()),
        }
    }
}

/// A MaterialKey can
pub type MaterialKey = MaterialComponent<()>;

pub use self::MaterialComponent::*;
