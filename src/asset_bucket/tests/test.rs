
extern crate asset_bucket;
extern crate entity;

use entity::Entity;
use asset_bucket::BucketReader;
use std::io;
use std::collections::HashMap;

fn build_table() -> Vec<(Entity, String)> {
    (0..100).map(|i| {
        (Entity::new(), format!("{}", i))
    }).collect()
}

#[test]
fn names_unpacked() {
    let table = build_table();
    let table: Vec<(Entity, &str)> = table.iter().map(|&(ref x, ref y)| (x.clone(), &y[..])).collect();

    let mut writer = asset_bucket::ColumnWriter::new();
    writer.set_names(&table[..]);

    let mut file: Vec<u8> = Vec::new();
    writer.write_unpacked(&mut file);

    let bucket = BucketReader::read(&mut io::Cursor::new(&file[..])).unwrap();
    let mut hm = HashMap::new();
    bucket.names(&mut hm);

    assert_eq!(hm.len(), table.len());
}

#[test]
fn names_packed() {
    let table = build_table();
    let table: Vec<(Entity, &str)> = table.iter().map(|&(ref x, ref y)| (x.clone(), &y[..])).collect();

    let mut writer = asset_bucket::ColumnWriter::new();
    writer.set_names(&table[..]);

    let mut file: Vec<u8> = Vec::new();
    writer.write_packed(&mut file);

    let bucket = BucketReader::read(&mut io::Cursor::new(&file[..])).unwrap();
    let mut hm = HashMap::new();
    bucket.names(&mut hm);

    assert_eq!(hm.len(), table.len());
}
