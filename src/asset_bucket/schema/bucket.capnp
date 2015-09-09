@0xa107353cd868fe79;

using Cxx = import "/capnp/c++.capnp";
$Cxx.namespace("bucket");

struct Bucket {
    encoding @0 :Encoding;

    enum Encoding {
        unpacked @0;
        packed @1;
    }

    columns @1 :Data;
}

struct Entity {
    union {
        local @0 :UInt32;
        external @1 :UInt32;
    }
}

struct Ids {
    maxLocal @0 :UInt32;
}

struct Name {
    id @0 :UInt32; 
    name @1 :Text;
}


struct Columns {
    ids @0 :Ids;
    names @1 :List(Name);
}