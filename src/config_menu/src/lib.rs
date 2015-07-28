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
use name::{Name, NameSystem, FullPath, PathLookup, ChildByName, RootName};
use config::{Config, ConfigSystem, GetConfig};
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

impl<'a> ReadEntity<ChildByName<'a>, Entity> for Router {
    fn read(&self, eid: &ChildByName<'a>) -> Option<&Entity> {
        self.name.read(eid)
    }
}
impl<'a> ReadEntity<RootName<'a>, Entity> for Router {
    fn read(&self, eid: &RootName<'a>) -> Option<&Entity> {
        self.name.read(eid)
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

    let spacing = router.config_f64("config_menu.spacing").unwrap_or(15.) as i32;
    let start_x = router.config_f64("config_menu.position.x").unwrap_or(10.) as i32;
    let mut start_y = router.config_f64("config_menu.position.y").unwrap_or(10.) as i32;
    for (eid, config) in router.config.clone().current.iter() {
        let res = router.full_path(&eid)
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

                start_y += spacing;
                (eid, DebugText{
                    text: text,
                    start: [start_x, start_y],
                    color: color
                })
            });

        res.map(|(eid, txt)| eid.bind(txt).write(router));
    }
}

fn select(router: &Router) -> Option<(&Entity, &Entity)> {
    router.name.lookup("config_menu.selected")
        .and_then(|cfg| {
            match router.read(cfg) {
                Some(&Config::String(ref s)) => Some((cfg, s.clone())),
                _ => None
            }
        })
        .and_then(|(cfg, name)| {
            router.name.lookup(&name)
                  .map(|n| (cfg, n))
        })
}

fn toggle(router: &mut Router) {
    let rtr = router.clone();
    select(&rtr)
        .and_then(|(_, eid)| {
            match rtr.read(eid) {
                Some(&Config::Bool(ref s)) => Some((eid, !s)),
                _ => None
            }
        })
        .map(|(eid, b)| {
            eid.bind(Config::Bool(b)).write(router);
        });
}

fn move_down(router: &mut Router) {
    let rtr = router.clone();
    select(&rtr)
        .and_then(|(name, eid)| {
            let mut found = false;
            for (id, _) in router.config.current.iter() {
                if found == true { return Some((name, id)); }
                if eid == id { found = true; }
            }
            None
        })
        .and_then(|(name, eid)| {
            rtr.full_path(eid)
               .map(|path| (name, path))
        })
        .map(|(name, path)| {
            name.bind(Config::String(path)).write(router);
        });
}

fn move_up(router: &mut Router) {
    let rtr = router.clone();
    select(&rtr)
        .and_then(|(name, eid)| {
            let mut last = None;
            for (id, _) in router.config.current.iter() {
                if id == eid { return last; }
                last = Some((name, id));
            }
            None
        })
        .and_then(|(name, eid)| {
            rtr.full_path(eid)
               .map(|path| (name, path))
        })
        .map(|(name, path)| {
            name.bind(Config::String(path)).write(router);
        });
}

fn value_add(router: &mut Router, v: f64) {
    let rate = router.config_f64("config_menu.shift_rate").unwrap_or(1.);

    let rtr = router.clone();
    select(&rtr)
        .and_then(|(_, eid)| {
            match rtr.read(eid) {
                Some(&Config::Float(ref s)) => Some((eid, s + v * rate)),
                _ => None
            }
        })
        .map(|(eid, b)| {
            eid.bind(Config::Float(b)).write(router);
        });
}

fn update_buffer<F>(router: &mut Router, cb: F)
    where F: FnOnce(&mut String)
{
    let rtr = router.clone();
    let mut buffer = rtr
        .config_string("config_menu.buffer")
        .unwrap_or("")
        .to_string();

    cb(&mut buffer);

    rtr.lookup("config_menu.buffer")
       .map(|eid| eid.bind(Config::String(buffer)).write(router));
}

fn char_append(router: &mut Router, c: char) {
    update_buffer(router, |buf| buf.push(c));
}

fn char_pop(router: &mut Router) {
    update_buffer(router, |buf| { buf.pop(); });
}

fn take_selected_value(router: &mut Router) {
    let rtr = router.clone();
    update_buffer(router, |buf| {
        let selected = rtr.config_string("config_menu.selected")
            .unwrap_or("config_menu.selected");
        let eid = rtr.lookup(selected).unwrap();
        *buf = match rtr.read(eid) {
            Some(&Config::String(ref s)) => s.clone(),
            Some(&Config::Float(f)) => format!("{}", f),
            Some(&Config::Bool(b)) => format!("{}", b),
            None => "".to_string()
        };
    });
}

fn write_value(router: &mut Router) {
    let rtr = router.clone();
    let selected = rtr.config_string("config_menu.selected").unwrap_or("");
    let selected_id = if let Some(id) = rtr.lookup(selected) {
        id
    } else {
        return;
    };
    let buffer = rtr.config_string("config_menu.buffer").unwrap_or("");

    match rtr.read(selected_id) {
        Some(&Config::String(_)) => Some(Config::String(buffer.to_string())),
        Some(&Config::Bool(_)) => Some(Config::Bool(buffer == "true")),
        Some(&Config::Float(_)) => match std::str::FromStr::from_str(buffer) {
            Ok(f) => Some(Config::Float(f)),
            Err(_) => None
        },
        None => None
    }.map(|config| selected_id.bind(config).write(router));
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

        Entity::new()
            .bind(Name::new("selected".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .bind(Config::String("config_menu.show".to_string()))
            .write(&mut router);

        Entity::new()
            .bind(Name::new("buffer".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .bind(Config::String("".to_string()))
            .write(&mut router);

        Entity::new()
            .bind(Name::new("shift_rate".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .bind(Config::Float(1.))
            .write(&mut router);

        Entity::new()
            .bind(Name::new("spacing".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .bind(Config::Float(15.))
            .write(&mut router);

        let pos = Entity::new()
            .bind(Name::new("position".to_string()).unwrap())
            .bind(Parent::Child(menu))
            .write(&mut router);

        Entity::new()
            .bind(Name::new("x".to_string()).unwrap())
            .bind(Parent::Child(pos))
            .bind(Config::Float(10.))
            .write(&mut router);

        Entity::new()
            .bind(Name::new("y".to_string()).unwrap())
            .bind(Parent::Child(pos))
            .bind(Config::Float(10.))
            .write(&mut router);

        let mut moved = false;
        let mut updated = false;
        loop {
            let show = if let Some(&Config::Bool(v)) = router.read(&show_eid) {
                v
            } else {
                false
            };

            if moved {
                take_selected_value(&mut router);
                moved = false;
            }

            if updated {
                write_value(&mut router);
                updated = false;
            }

            for msg in input.iter() {
                match msg {
                    &WindowEvent::Key(Key::GraveAccent, _, Action::Press, _) => {
                        show_eid.bind(Config::Bool(!show)).write(&mut router);
                    }
                    &WindowEvent::Key(Key::Space, _, Action::Press, _) |
                    &WindowEvent::Key(Key::Enter, _, Action::Press, _) => {
                        toggle(&mut router);
                    }
                    &WindowEvent::Key(Key::Up, _, Action::Press, _) |
                    &WindowEvent::Key(Key::Up, _, Action::Repeat, _) => {
                        if show {
                            move_up(&mut router);
                            moved = true;
                        }
                    }
                    &WindowEvent::Key(Key::Down, _, Action::Press, _) |
                    &WindowEvent::Key(Key::Down, _, Action::Repeat, _) => {
                        if show {
                            move_down(&mut router);
                            moved = true;
                        }
                    }
                    &WindowEvent::Key(Key::Right, _, Action::Press, _) |
                    &WindowEvent::Key(Key::Right, _, Action::Repeat, _) => {
                        if show {
                            value_add(&mut router, 1.);
                        }
                    }
                    &WindowEvent::Key(Key::Left, _, Action::Press, _) |
                    &WindowEvent::Key(Key::Left, _, Action::Repeat, _) => {
                        if show {
                            value_add(&mut router, -1.);
                        }
                    }
                    &WindowEvent::Char(c) => {
                        if show {
                            char_append(&mut router, c);
                            updated = true;
                        }
                    }
                    &WindowEvent::Key(Key::Backspace, _, Action::Press, _) |
                    &WindowEvent::Key(Key::Backspace, _, Action::Repeat, _) => {
                        if show {
                            char_pop(&mut router);
                            updated = true;
                        }
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
