
pub use glfw::{self, Key, MouseButton, Action, Modifiers, Scancode};

#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
pub enum WindowEvent {
    TimeStamp(f64),
    Pos(i32, i32),
    Size(i32, i32),
    Close,
    Refresh,
    Focus(bool),
    Iconify(bool),
    FramebufferSize(i32, i32),
    MouseButton(MouseButton, Action, Modifiers),
    CursorPos(f64, f64),
    CursorEnter(bool),
    Scroll(f64, f64),
    Key(Key, Scancode, Action, Modifiers),
    Char(char),
}

impl WindowEvent {
    pub fn from_glfw(event: glfw::WindowEvent) -> WindowEvent {
       match event {
            glfw::WindowEvent::Pos(x, y) => WindowEvent::Pos(x, y),
            glfw::WindowEvent::Size(w, h) => WindowEvent::Size(w, h),
            glfw::WindowEvent::Close => WindowEvent::Close,
            glfw::WindowEvent::Refresh => WindowEvent::Refresh,
            glfw::WindowEvent::Focus(focus) => WindowEvent::Focus(focus),
            glfw::WindowEvent::Iconify(icon) => WindowEvent::Iconify(icon),
            glfw::WindowEvent::FramebufferSize(w, h) => WindowEvent::FramebufferSize(w, h),
            glfw::WindowEvent::MouseButton(a, b, c) => WindowEvent::MouseButton(a, b, c),
            glfw::WindowEvent::CursorPos(x, y) => WindowEvent::CursorPos(x, y),
            glfw::WindowEvent::CursorEnter(enter) => WindowEvent::CursorEnter(enter),
            glfw::WindowEvent::Scroll(x, y) => WindowEvent::CursorPos(x, y),
            glfw::WindowEvent::Key(a, b, c, d) => WindowEvent::Key(a, b, c, d),
            glfw::WindowEvent::Char(c) => WindowEvent::Char(c)
       } 
    }
}