
extern crate graphics;
extern crate future_pulse;
extern crate fibe;
extern crate obj;
extern crate pulse;
extern crate image;
extern crate genmesh;

use std::collections::HashMap;
use std::path::PathBuf;
use std::io::{BufReader, Error};
use std::fs::File;
use std::sync::Arc;

use image::ImageError;

use pulse::{SelectMap, Signals};
use fibe::{Schedule, task};
use future_pulse::{Future, Set};
use graphics::{GraphicsSource, Texture, VertexBuffer,
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
                 src: &GraphicsSource)
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
fn load_material(sched: &mut Schedule, path: PathBuf) -> Future<Result<obj::Mtl, Error>> {

    let (future, set) = Future::new();
    task(move |_| {
        set.set(
            File::open(&path)
             .map(|file| {
                obj::Mtl::load(&mut BufReader::new(file))
              })
        )

    }).start(sched);
    future
}

/// Collect the results of a materials into a vector
fn collect_materials(sched: &mut Schedule,
                     r: Set<Vec<Material>>,
                     mut v: Vec<Material>,
                     mut m: SelectMap<Future<Result<Mtl, Error>>>) {
    
    if let Some((_, x)) = m.try_next() {
        for m in x.get().unwrap().materials {
            v.push(m);
        }
    }

    if m.len() == 0 {
        r.set(v);
        return
    } else {
        let sig = m.signal();
        task(move |sched| collect_materials(sched, r, v, m)).after(sig).start(sched);
    }
}

fn collect_textures(sched: &mut Schedule,
                    r: Set<HashMap<String, Texture>>,
                    mut v: HashMap<String, Texture>,
                    mut m: SelectMap<(String, Future<Result<Texture, ImageError>>)>) {

    if let Some((_, (k, t))) = m.try_next() {
        v.insert(k, t.get().unwrap());
    }

    if m.len() == 0 {
        r.set(v);
        return
    } else {
        let sig = m.signal();
        task(move |sched| collect_textures(sched, r, v, m)).after(sig).start(sched);
    }
}

fn resolve_materials(materials: Vec<Material>,
                     texture: HashMap<String, Texture>,
                     mut src: GraphicsSource) -> HashMap<String, graphics::Material> {

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
                 src: GraphicsSource) -> HashMap<String, (Geometry, Option<String>)> {

    let mut res = HashMap::new();
    let object = Arc::new(object);
    let o2 = object.clone();
    for o in o2.object_iter() {
        let object = object.clone();
        let mut src = src.clone();
        let g = o.group_iter().next().unwrap(); // expect one group only

        let idx = g.indices.clone();
        let geo = Geometry::new();
        res.insert(o.name.clone(), (geo, g.material.clone()));

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
    res
}

pub fn load(sched: &mut Schedule, path: PathBuf, src: GraphicsSource)
    -> Future<HashMap<String, (Geometry, Option<graphics::Material>)>> {
    let (fres, fset) = Future::new();

    task(move |sched| {
        File::open(path.clone()).map(move |f| {
            let mut f = BufReader::new(f);
            let obj = obj::Obj::load(&mut f);

            let mut materials = SelectMap::new();
            for m in obj.materials().iter() {
                let mut p = path.clone();
                p.pop();
                p.push(&m);
                let m = load_material(sched, p);
                let s = m.signal();
                materials.add(s, m);
            }

            let sig = materials.signal();
            let (future, set) = Future::new();
            task(move |sched| {
                collect_materials(sched, set, Vec::new(), materials)
            }).after(sig).start(sched);

            let geo = load_geometry(sched, obj, src.clone());

            let sig = future.signal();
            task(move |sched| {
                let mut mapping = SelectMap::new();
                let materials = future.get();
                for (k, v) in load_textures(sched, path, &materials[..], &src) {
                    let sig = v.signal();
                    mapping.add(sig, (k, v));
                }

                let (future_text, set_text) = Future::new();
                let sig = mapping.signal();
                task(move |sched| {
                    collect_textures(sched, set_text, HashMap::new(), mapping);
                }).after(sig).start(sched);

                let sig = future_text.signal();
                task(move |_| {
                    let texture = future_text.get();
                    let mat = resolve_materials(materials, texture, src);
                    let mut res = HashMap::new();
                    for (k, (g, m)) in geo {
                        res.insert(k, (g, m.and_then(|v| mat.get(&v).map(|x| *x))));
                    }
                    fset.set(res);
                }).after(sig).start(sched);

            }).after(sig).start(sched);
            

        });
    }).start(sched);

    fres
}