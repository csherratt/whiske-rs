
extern crate graphics;
extern crate future_pulse;
extern crate fibe;
extern crate pulse;
extern crate image;
extern crate genmesh;
extern crate obj;

use std::collections::HashMap;
use std::path::PathBuf;

use fibe::{Schedule};
use future_pulse::Future;
use graphics::{Graphics, Geometry};
use obj::Material;

mod wavefront_obj;

pub fn load(sched: &mut Schedule, path: PathBuf, src: Graphics)
    -> Result<Future<Object>, Error> {

    match path.extension() {
        Some(s) => {
            if s == "obj" {
                match wavefront_obj::load(sched, path.clone(), src) {
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