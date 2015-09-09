extern crate capnp;
extern crate entity;

use std::collections::HashMap;
use std::io::{Read, BufReader};
use entity::{Entity, WriteEntity};
use capnp::message::{Reader, ReaderOptions};
use capnp::{serialize, serialize_packed};

pub mod bucket_capnp {
    include!(concat!(env!("OUT_DIR"), "/bucket_capnp.rs"));
}

pub struct BucketReader {
    eids: Vec<entity::Entity>,
    reader: Reader<serialize::OwnedSegments>
}

#[derive(Debug)]
pub struct Name<'a>(pub &'a str);

impl BucketReader {
    pub fn read<R>(read: &mut R) -> Result<BucketReader, capnp::Error>
        where R: Read
    {
        use bucket_capnp::bucket::Encoding;

        let reader = try!(serialize::read_message(read, ReaderOptions::new()));
        let root: bucket_capnp::bucket::Reader = try!(reader.get_root());
        let encoding = try!(root.get_encoding());
        let columns = try!(root.get_columns());

        let reader = match encoding {
            Encoding::Unpacked => {
                try!(serialize::read_message(
                    &mut BufReader::new(columns),
                    ReaderOptions::new()
                ))
            }
            Encoding::Packed => {
                try!(serialize_packed::read_message(
                    &mut BufReader::new(columns),
                    ReaderOptions::new()
                ))
            }
        };

        let max = {
            let root: bucket_capnp::columns::Reader = try!(reader.get_root());
            try!(root.get_ids()).get_max_local()
        };

        Ok(BucketReader {
            eids: (0..max).map(|_| Entity::new()).collect(),
            reader: reader
        })
    }

    pub fn names<'a, W>(&'a self, w: &mut W) -> Result<(), capnp::Error>
        where W: WriteEntity<Entity, Name<'a>>
    {
        let root: bucket_capnp::columns::Reader = try!(self.reader.get_root());
        let names = try!(root.get_names());

        for name in names.iter() {
            w.write(
                self.eids[name.get_id() as usize],
                Name(try!(name.get_name()))
            );
        }

        Ok(())
    }
}

struct EidMapping {
    pub eids: Vec<entity::Entity>,
    pub eids_lookup: HashMap<entity::Entity, u32>,    
}

impl EidMapping {
    fn new() -> EidMapping {
        EidMapping {
            eids: Vec::new(),
            eids_lookup: HashMap::new()
        }
    }

    /// Looks up an entity, iff not found it will be added
    /// to the table of known entities
    fn get_entity_index(&mut self, eid: &Entity) -> u32 {
        if let Some(&id) = self.eids_lookup.get(eid) {
            return id;
        }

        let idx = self.eids.len() as u32;
        self.eids.push(eid.clone());
        self.eids_lookup.insert(eid.clone(), idx);
        idx
    }
}

pub struct ColumnWriter {
    eids: EidMapping,
    builder: capnp::message::Builder<capnp::message::HeapAllocator>
}

impl ColumnWriter {
    pub fn new() -> ColumnWriter {
        let mut builder = capnp::message::Builder::new_default();
        {
            let mut root = builder.init_root::<bucket_capnp::columns::Builder>();
            let mut ids = root.borrow().init_ids();
            ids.set_max_local(0);
        }

        ColumnWriter {
            eids: EidMapping::new(),
            builder: builder
        }
    }

    // this just writes the correct values into the ids
    fn sync(&mut self) {
        let mut root = self.builder.get_root::<bucket_capnp::columns::Builder>().unwrap();
        let mut ids = root.borrow().get_ids().unwrap();
        ids.set_max_local(self.eids.eids.len() as u32);
    }

    pub fn set_names(&mut self, names: &[(Entity, &str)]) {
        {
            let root = self.builder.get_root::<bucket_capnp::columns::Builder>().unwrap();
            let mut name_column = root.init_names(names.len() as u32);

            for (i, &(ref eid, ref name)) in names.iter().enumerate() {
                let mut n = name_column.borrow().get(i as u32);
                n.set_id(self.eids.get_entity_index(eid));
                n.set_name(name);
            }
        }

        self.sync();
    }

    pub fn write_unpacked<W>(&self, w: &mut W) -> Result<(), std::io::Error>
        where W: std::io::Write
    {
        use bucket_capnp::bucket::Encoding;

        let mut data: Vec<u8> = Vec::new();
        try!(capnp::serialize::write_message(&mut data, &self.builder));

        let mut builder = capnp::message::Builder::new_default();
        {
            let mut root = builder.init_root::<bucket_capnp::bucket::Builder>();
            root.set_encoding(Encoding::Unpacked);
            root.set_columns(&data[..]);
        }

        capnp::serialize::write_message(w, &builder)
    }

    pub fn write_packed<W>(&self, w: &mut W) -> Result<(), std::io::Error>
        where W: std::io::Write
    {
        use bucket_capnp::bucket::Encoding;

        let mut data: Vec<u8> = Vec::new();
        try!(capnp::serialize_packed::write_message(&mut data, &self.builder));

        let mut builder = capnp::message::Builder::new_default();
        {
            let mut root = builder.init_root::<bucket_capnp::bucket::Builder>();
            root.set_encoding(Encoding::Packed);
            root.set_columns(&data[..]);
        }

        capnp::serialize::write_message(w, &builder)
    }
}