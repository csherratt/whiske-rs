#[macro_use(route, router)]
extern crate entity;
extern crate fibe;
extern crate engine;
extern crate name;
extern crate config;
extern crate renderer;
extern crate parent;
extern crate snowstorm;

use std::collections::HashMap;
use entity::{Entity, WriteEntity, ReadEntity};

use renderer::{Renderer, DebugText};
use name::{Name, NameSystem};
use config::{Config, ConfigSystem};
use engine::event::{WindowEvent, Key, Action};
use snowstorm::channel::Receiver;
use fibe::{Schedule, task};
use parent::{Parent, ParentSystem};

router!{
    struct Router {
        [rw: Entity, Config] => config: ConfigSystem,
        [rw: Entity, DebugText] => render: Renderer,
        [rw: Entity, Parent] => parent: ParentSystem,
        [rw: Entity, Name] => name: NameSystem
    }
}

impl Clone for Router {
    fn clone(&self) -> Router {
        Router {
            name: self.name.clone(),
            parent: self.parent.clone(),
            config: self.config.clone(),
            render: self.render.clone()
        }
    }
}

impl Router {
    fn next_frame(self) -> Router {
        let Router{
            name, parent, config, render
        } = self;

        let n = name.next_frame();
        let p = parent.next_frame();
        let c = config.next_frame();
        let r = render.next_frame();

        Router{
            name: n.get().unwrap(),
            parent: p.get().unwrap(),
            config: c.get().unwrap(),
            render: r.get().unwrap()
        }
    }
}

fn write_config_menu(hm: &mut HashMap<Entity, Entity>,
                     router: &mut Router) {

    let selected = router.name.lookup("config_menu.selected")
          .and_then(|eid| {
            match router.read(eid) {
                Some(&Config::String(ref s)) => Some(s.clone()),
                _ => None
            }
          });

    let mut start = 0;
    for (eid, config) in router.config.clone().current.iter() {
        let res = router.name.full_path(&router.parent, eid)
            .map(|path| {
                let eid = *hm.entry(*eid)
                  .or_insert_with(|| Entity::new());

                let text = match config {
                    &Config::Bool(s) => format!("{} = {:?}", path, s),
                    &Config::Float(f) => format!("{} = {:?}", path, f),
                    &Config::String(ref s) => format!("{} = \"{}\"", path, s),
                };

                let color = if Some(path) == selected {
                    [1.0, 1.0, 1.0, 1.]
                } else {
                    [0.5, 0.5, 0.5, 1.]
                };

                start += 15;
                (eid, DebugText{
                    text: text,
                    start: [10, start],
                    color: color
                })
            });

        res.map(|(eid, txt)| eid.bind(txt).write(router));
    }
}

fn toggle(router: &mut Router) {
    let rtr = router.clone();
    rtr.name.lookup("config_menu.selected")
        .and_then(|eid| {
            match rtr.read(eid) {
                Some(&Config::String(ref s)) => Some(s.clone()),
                _ => None
            }
        })
        .and_then(|name| {
            rtr.name.lookup(&name)
        })
        .and_then(|eid| {
            match rtr.read(eid) {
                Some(&Config::Bool(ref s)) => Some((eid, !s)),
                _ => None
            }
        })
        .map(|(eid, b)| {
            eid.bind(Config::Bool(b)).write(router);
        });

}

fn hide_config_menu(hm: &mut HashMap<Entity, Entity>,
                    router: &mut Router) {
    for (_, v) in hm.iter() {
        v.bind(DebugText{
            text: "".to_string(),
            start: [0, 0],
            color: [1., 1., 1., 1.]
        }).write(router);
    }
}

pub fn config_menu(sched: &mut Schedule,
               mut input: Receiver<WindowEvent>,
               name: NameSystem,
               parent: ParentSystem,
               config: ConfigSystem,
               render: Renderer) {

    let router = Router{
        name: name,
        parent: parent,
        config: config,
        render: render
    };

    task(move |_| {
        let mut hm = HashMap::new();
        let mut router = router;

        let menu = Entity::new()
            .bind(Name::new("config_menu".to_string()).unwrap())
            .write(&mut router);

        let show_eid = Entity::new()
            .bind(Name::new("show".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .bind(Config::Bool(false))
            .write(&mut router);

        let select = Entity::new()
            .bind(Name::new("selected".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .bind(Config::String("config_menu.show".to_string()))
            .write(&mut router);

        loop {
            let mut show = if let Some(&Config::Bool(v)) = router.read(&show_eid) {
                v
            } else {
                false
            };

            for msg in input.iter() {
                println!("{:?}", msg);
                match msg {
                    &WindowEvent::Key(Key::GraveAccent, _, Action::Press, _) => {
                        show = !show;
                        show_eid.bind(Config::Bool(show)).write(&mut router);
                    }
                    &WindowEvent::Key(Key::Enter, _, Action::Press, _) => {
                        toggle(&mut router);
                    }
                    _ => ()
                }
            }
            input.next_frame();

            if show {
                write_config_menu(&mut hm, &mut router);
            } else {
                hide_config_menu(&mut hm, &mut router);
            }

            router = router.next_frame();
        }
    }).start(sched);
}
