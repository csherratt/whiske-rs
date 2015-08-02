
extern crate graphics;
extern crate future_pulse;
extern crate fibe;
extern crate obj;
extern crate pulse;
extern crate image;
extern crate genmesh;

use std::collections::HashMap;
use std::path::PathBuf;
use std::io::{self, BufReader};
use std::fs::File;
use std::sync::Arc;

use image::ImageError;
use pulse::{SelectMap, Signals};
use fibe::{Schedule, task};
use future_pulse::Future;
use graphics::{Graphics, Texture, VertexBuffer,
    Ka, Kd, Ks, VertexPosTexNorm, Geometry, Primative,
    PosTexNorm
};
use obj::{Mtl, Material};
use genmesh::{
    Triangulate,
    MapToVertices,
    Vertices,
    LruIndexer,
    Indexer
};

// From the supplied materials load every texture
fn load_textures(sched: &mut Schedule,
                 path: PathBuf,
                 materials: &[Material],
                 src: &Graphics)
    -> HashMap<String, Future<Result<Texture, ImageError>>> {
    
    let mut map = HashMap::new();
    for m in materials.iter() {
        let text = [&m.map_ka, &m.map_kd, &m.map_ks, &m.map_ke];
        for t in text.iter() {
            if let &Some(ref t) = *t {
                let insert = map.get(t).is_none();
                if insert {
                    let mut path = path.clone();
                    path.pop();
                    path.push(t);

                    map.insert(
                        t.clone(),
                        Texture::load(sched, path, src.clone())
                    );
                }
            }
        }
    }
    map
}

/// Load the material returning it as a future
fn load_material(sched: &mut Schedule, path: PathBuf) -> Future<Result<obj::Mtl, io::Error>> {
    task(move |_| {
        File::open(&path)
             .map(|file| {
                obj::Mtl::load(&mut BufReader::new(file))
             })
    }).start(sched)
}

fn resolve_materials(materials: Vec<Material>,
                     texture: HashMap<String, Texture>,
                     mut src: Graphics) -> HashMap<String, graphics::Material> {

    let mut res = HashMap::new();
    for m in materials {
        let mat = graphics::Material::new();

        m.ka.map(|v| mat.bind(Ka([v[0], v[1], v[2], 1.])).write(&mut src));
        m.kd.map(|v| mat.bind(Kd([v[0], v[1], v[2], 1.])).write(&mut src));
        m.ks.map(|v| mat.bind(Ks([v[0], v[1], v[2], 1.])).write(&mut src));

        m.map_ka.map(|ref text|{
            texture.get(text).map(|t| mat.bind(Ka(*t)).write(&mut src));
        });
        m.map_kd.map(|ref text|{
            texture.get(text).map(|t| mat.bind(Kd(*t)).write(&mut src));
        });
        m.map_ks.map(|ref text|{
            texture.get(text).map(|t| mat.bind(Ks(*t)).write(&mut src));
        });

        res.insert(m.name, mat);
    }

    res
}

fn load_geometry(sched: &mut Schedule,
                 object: obj::Obj<String>,
                 src: Graphics) -> HashMap<(String, String), (Geometry, Option<String>)> {

    let mut res = HashMap::new();
    let object = Arc::new(object);
    let o2 = object.clone();
    for o in o2.object_iter() {

        for g in o.group_iter() {
            let mut src = src.clone();
            let object = object.clone();
            let name = (o.name.clone(), format!("{}.{}", g.name, g.index));
            let idx = g.indices.clone();
            let geo = Geometry::new();
            res.insert(name, (geo, g.material.clone()));

            task(move |_| {
                let mut vertices = Vec::new();
                let indices: Vec<u32> = {
                    let object = object.clone();
                    let mut indexer = LruIndexer::new(64, |_, v| {
                        let (p, t, n): (usize, Option<usize>, Option<usize>) = v;
                        let vert = match (t, n) {
                            (Some(t), Some(n)) => {
                                VertexPosTexNorm {
                                    position: object.position()[p],
                                    texture: object.texture()[t],
                                    normal: object.normal()[n]
                                }
                            }
                            (Some(t), _) => {
                                VertexPosTexNorm {
                                    position: object.position()[p],
                                    texture: object.texture()[t],
                                    normal: [1., 0., 0.]
                                }
                            }
                            (_, Some(n)) => {
                                VertexPosTexNorm {
                                    position: object.position()[p],
                                    texture: [0., 0.],
                                    normal: object.normal()[n]
                                }
                            }
                            (_, _) => {
                                VertexPosTexNorm {
                                    position: object.position()[p],
                                    texture: [0., 0.],
                                    normal: [1., 0., 0.]
                                }
                            }
                        };
                        vertices.push(vert)
                    });

                    idx.iter()
                       .map(|x| *x)
                       .triangulate()
                       .vertex(|v| indexer.index(v) as u32)
                       .vertices()
                       .collect()
                };

                let vb = VertexBuffer::new()
                    .bind(PosTexNorm(vertices))
                    .bind_index(indices)
                    .write(&mut src);

                geo.bind(vb.geometry(Primative::Triangle))
                   .write(&mut src);
            }).start(sched);
        }
    }
    res
}

pub fn load_obj(sched: &mut Schedule, path: PathBuf, src: Graphics)
    -> Result<Future<Object>, std::io::Error> {

    File::open(path.clone()).map(|f| {
        task(move |sched| {
            let mut f = BufReader::new(f);
            let obj = obj::Obj::load(&mut f);

            let mut materials_future = SelectMap::new();
            for m in obj.materials().iter() {
                let mut p = path.clone();
                p.pop();
                p.push(&m);
                let m = load_material(sched, p);
                let s = m.signal();
                materials_future.add(s, m);
            }

            let geo = load_geometry(sched, obj, src.clone());

            let mut materials: Vec<Material> = Vec::new();
            for (_, mat) in materials_future {
                let mat = mat.get().unwrap();
                for m in mat.materials { 
                    materials.push(m);
                }
            }

            let mut textures_future = SelectMap::new();
            for (k, v) in load_textures(sched, path, &materials[..], &src) {
                let sig = v.signal();
                textures_future.add(sig, (k, v));
            }

            let mut textures = HashMap::new();
            for (_, (k, v)) in textures_future {
                textures.insert(k, v.get().unwrap());
            }

            let mat = resolve_materials(materials, textures, src);
            let mut res = HashMap::new();
            for (k, (g, m)) in geo {
                res.insert(k, (g, m.and_then(|v| mat.get(&v).map(|x| *x))));
            }
            res
        }).start(sched)
    })
}

pub fn load(sched: &mut Schedule, path: PathBuf, src: Graphics)
    -> Result<Future<Object>, Error> {

    match path.extension() {
        Some(s) => {
            if s == "obj" {
                match load_obj(sched, path.clone(), src) {
                    Err(e) => Err(e.into()),
                    Ok(x) => Ok(x)
                }
            } else {
                Err(Error::UnknownType)
            }
        }
        None => Err(Error::UnknownType)
    }
}

#[derive(Debug)]
pub enum Error {
    UnknownType,
    Io(std::io::Error)
}

impl Into<Error> for std::io::Error {
    fn into(self) -> Error {
        Error::Io(self)
    }
}

pub type Object = HashMap<(String, String), (Geometry, Option<graphics::Material>)>;