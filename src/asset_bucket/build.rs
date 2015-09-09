
extern crate capnpc;

fn main() {
    ::capnpc::compile("bucket", &["schema/bucket.capnp"]).unwrap();
}