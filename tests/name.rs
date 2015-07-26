extern crate parent;
extern crate fibe;
#[macro_use(route, router)]
extern crate entity;
extern crate name;

use entity::*;
use parent::{parent, Parent, ParentSystem};
use fibe::*;
use name::*;

router!{
    struct Router {
        [rw: Entity, Name] => name: NameSystem,
        [rw: Entity, Parent] => parent: ParentSystem
    }
}

impl Router {
    fn next_frame(self) -> Router {
        let Router{name, parent} = self;
        let name = name.next_frame();
        let parent = parent.next_frame();
        Router {
            name: name.get().unwrap(),
            parent: parent.get().unwrap()
        }
    }
}

#[test]
fn name_something() {
    let mut sched = Frontend::new();
    let parent = parent(&mut sched);
    let mut name = name(&mut sched, parent);

    let eid = Entity::new()
        .bind(Name::new("foo".to_string()).unwrap())
        .write(&mut name);

    name = name.next_frame().get().unwrap();

    assert_eq!(&**name.read(&eid).unwrap(), "foo");
}

#[test]
fn name_reject() {
    assert!(Name::new("".to_string()).is_none());
    assert!(Name::new("foo".to_string()).is_some());
    assert!(Name::new("foo.bar".to_string()).is_none());
    assert!(Name::new("foo/bar".to_string()).is_none());
}

#[test]
fn parent_child() {
    let mut sched = Frontend::new();
    let parent = parent(&mut sched);
    let name = name(&mut sched, parent.clone());

    let mut rtr = Router{
        name: name,
        parent: parent
    };

    let parent = Entity::new()
        .bind(Name::new("foo".to_string()).unwrap())
        .write(&mut rtr);
    let child = Entity::new()
        .bind(Name::new("bar".to_string()).unwrap())
        .bind(Parent::Child(parent))
        .write(&mut rtr);

    rtr = rtr.next_frame();

    let p: &Name = rtr.read(&parent).unwrap();
    assert_eq!(&**p, "foo");

    let c: &Name = rtr.read(&child).unwrap();
    assert_eq!(&**c, "bar");

    let x: &Entity = rtr.name.read(&ChildByName(parent, "bar")).unwrap();
    assert_eq!(*x, child);
}

#[test]
fn lookup_path() {
    let mut sched = Frontend::new();
    let parent = parent(&mut sched);
    let name = name(&mut sched, parent.clone());

    let mut rtr = Router{
        name: name,
        parent: parent
    };

    let parent = Entity::new()
        .bind(Name::new("foo".to_string()).unwrap())
        .write(&mut rtr);
    let child0 = Entity::new()
        .bind(Name::new("bar".to_string()).unwrap())
        .bind(Parent::Child(parent))
        .write(&mut rtr);
    let child1 = Entity::new()
        .bind(Name::new("baz".to_string()).unwrap())
        .bind(Parent::Child(parent))
        .write(&mut rtr);
    let child2 = Entity::new()
        .bind(Name::new("baz".to_string()).unwrap())
        .bind(Parent::Child(child0))
        .write(&mut rtr);

    rtr = rtr.next_frame();

    assert_eq!(&parent, rtr.name.lookup("foo").unwrap());
    assert_eq!(&child0, rtr.name.lookup("foo.bar").unwrap());
    assert_eq!(&child1, rtr.name.lookup("foo.baz").unwrap());
    assert_eq!(&child2, rtr.name.lookup("foo.bar.baz").unwrap());
    assert_eq!(None, rtr.name.lookup("foo.bar."));
    assert_eq!(None, rtr.name.lookup("foo.ba"));
    assert_eq!(None, rtr.name.lookup("foo."));
    assert_eq!(None, rtr.name.lookup("......"));
}