extern crate hairball;
extern crate hairball_mesh;
extern crate hairball_mesh_index;
extern crate hairball_material;
extern crate hairball_geometry;
extern crate hairball_draw_binding;
extern crate entity;
extern crate graphics;
extern crate name;
extern crate parent;
extern crate renderer;

use std::path::Path;
use std::collections::HashMap;

use entity::{Entity, WriteEntity};
use graphics::{VertexBuffer, Vertex, Material, MaterialComponent};
use graphics::{Geometry, GeometryData, Primative, VertexSubBuffer};
use name::Name;
use parent::Parent;

pub fn load<T, P>(path: P, into: &mut T) -> Result<Vec<Entity>, hairball::Error>
    where P: AsRef<Path>,
          T: WriteEntity<VertexBuffer, Vec<Vertex>> +
             WriteEntity<VertexBuffer, Vec<u32>> +
             WriteEntity<Entity, Name> +
             WriteEntity<Entity, Parent> +
             WriteEntity<Material, MaterialComponent<[f32; 4]>> +
             WriteEntity<Geometry, GeometryData> +
             WriteEntity<Entity, renderer::DrawBinding>
{
    let reader = try!(hairball::Reader::read(path));
    let mapping = reader.into_mapping(|_| Entity::new());

    // Read names
    for i in 0..mapping.entities_len() {
        if let (Some(eid), Some(e)) = (mapping.entity(i), mapping.get_entity(i)) {
            use hairball::Entity::*;
            match e {
                Local(l) => {
                    if let Some(name) = l.name.map(|p| p.to_owned()).and_then(|n| Name::new(n)) {
                        println!("{:?}", name);
                        into.write(*eid, name);
                    }
                    if let Some(parent) = l.parent.and_then(|i| mapping.entity(i as usize)) {
                        into.write(*eid, Parent::Child(*parent));
                    }
                }
                // TODO
                External(_) => ()
            }
        }
    }

    let mut vbs = HashMap::new();
    if let Some(reader) = hairball_mesh::read(&mapping) {
        for (eid, vb) in reader {
            // TODO == Use zero-copy
            let vb: Vec<Vertex> = vb.into_iter().map(|x| x.owned()).collect();
            vbs.insert(*eid, VertexBuffer::from_entity(*eid).bind(vb));
        }
    }

    if let Some(reader) = hairball_mesh_index::read(&mapping) {
        for (eid, index) in reader {
            // TODO == Use zero-copy
            if let Some(vb) = vbs.remove(eid) {
                vb.bind_index(index).write(into);
            } else {
                VertexBuffer::from_entity(*eid).bind(index).write(into);
            }
        }
    }

    // Write what is left from the buffer
    for (_, v) in vbs {
        v.write(into);
    }

    if let Some(reader) = hairball_material::read(&mapping) {
        for (eid, component, value) in reader {
            use hairball_material::Value;
            use hairball_material::Component::*;
            use graphics::{Ka, Kd, Ks};

            let value = if let Value::Color(v) = value { v } else { continue };
            let value = match component {
                Ambient => Ka(value),
                Diffuse => Kd(value),
                Specular => Ks(value),
            };

            into.write(Material(*eid), value);
        }
    }

    if let Some(reader) = hairball_geometry::read(&mapping) {
        for (eid, geo) in reader {
            let geo = GeometryData{
                buffer: VertexSubBuffer{
                    start: geo.start,
                    length: geo.length,
                    parent: *geo.mesh,
                },
                primative: Primative::Triangle
            };
            into.write(Geometry(*eid), geo);
        }
    }

    let mut out = Vec::new();
    if let Some(reader) = hairball_draw_binding::read(&mapping) {
        for (eid, db) in reader {
            let db = renderer::DrawBinding(
                Geometry(*db.geometry),
                Material(*db.material)
            );
            into.write(*eid, db);
            out.push(*eid);
        }
    }

    Ok(out)
}