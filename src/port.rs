//! Ports: stdin/stdout, files, string ports.
//!
//! 端口统一抽象为"可读字符/可写字符串"的对象，reader 通过 CharSource
//! 挂在任意输入端口上实现 `read`。当前输入/输出端口是动态绑定
//! （thread_local），`with-input-from-file` 等通过替换它们生效。

use std::cell::RefCell;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::rc::Rc;

pub enum PortInner {
    Stdin {
        buf: VecDeque<char>,
    },
    Stdout,
    FileIn {
        reader: BufReader<File>,
        peeked: Option<Option<char>>,
    },
    FileOut(BufWriter<File>),
    StringIn {
        chars: Vec<char>,
        pos: usize,
    },
    StringOut(String),
    Closed,
}

pub struct Port {
    pub inner: RefCell<PortInner>,
    pub input: bool,
    pub output: bool,
}

impl Port {
    pub fn stdin() -> Rc<Port> {
        Rc::new(Port {
            inner: RefCell::new(PortInner::Stdin {
                buf: VecDeque::new(),
            }),
            input: true,
            output: false,
        })
    }
    pub fn stdout() -> Rc<Port> {
        Rc::new(Port {
            inner: RefCell::new(PortInner::Stdout),
            input: false,
            output: true,
        })
    }
    pub fn open_input_file(path: &str) -> Result<Rc<Port>, String> {
        let f = File::open(path).map_err(|e| format!("open-input-file: {}: {}", path, e))?;
        Ok(Rc::new(Port {
            inner: RefCell::new(PortInner::FileIn {
                reader: BufReader::new(f),
                peeked: None,
            }),
            input: true,
            output: false,
        }))
    }
    pub fn open_output_file(path: &str) -> Result<Rc<Port>, String> {
        let f = File::create(path).map_err(|e| format!("open-output-file: {}: {}", path, e))?;
        Ok(Rc::new(Port {
            inner: RefCell::new(PortInner::FileOut(BufWriter::new(f))),
            input: false,
            output: true,
        }))
    }
    pub fn open_input_string(s: &str) -> Rc<Port> {
        Rc::new(Port {
            inner: RefCell::new(PortInner::StringIn {
                chars: s.chars().collect(),
                pos: 0,
            }),
            input: true,
            output: false,
        })
    }
    pub fn open_output_string() -> Rc<Port> {
        Rc::new(Port {
            inner: RefCell::new(PortInner::StringOut(String::new())),
            input: false,
            output: true,
        })
    }

    pub fn is_closed(&self) -> bool {
        matches!(&*self.inner.borrow(), PortInner::Closed)
    }

    pub fn read_char(&self) -> Option<char> {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            PortInner::Stdin { buf } => {
                if buf.is_empty() {
                    let mut s = String::new();
                    match std::io::stdin().read_line(&mut s) {
                        Ok(0) => return None,
                        Ok(_) => buf.extend(s.chars()),
                        Err(_) => return None,
                    }
                }
                buf.pop_front()
            }
            PortInner::FileIn { reader, peeked } => {
                if let Some(p) = peeked.take() {
                    return p;
                }
                read_one_char(reader)
            }
            PortInner::StringIn { chars, pos } => {
                let c = chars.get(*pos).copied();
                if c.is_some() {
                    *pos += 1;
                }
                c
            }
            _ => None,
        }
    }

    pub fn peek_char(&self) -> Option<char> {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            PortInner::Stdin { .. } => {
                drop(inner);
                // cheap approach: read then we cannot unread; buffer handles it
                // read_char on Stdin consumes from internal buffer only after
                // filling, so emulate peek by reading into buf.
                let mut inner = self.inner.borrow_mut();
                if let PortInner::Stdin { buf } = &mut *inner {
                    if buf.is_empty() {
                        let mut s = String::new();
                        match std::io::stdin().read_line(&mut s) {
                            Ok(0) => return None,
                            Ok(_) => buf.extend(s.chars()),
                            Err(_) => return None,
                        }
                    }
                    buf.front().copied()
                } else {
                    None
                }
            }
            PortInner::FileIn { reader, peeked } => {
                if peeked.is_none() {
                    *peeked = Some(read_one_char(reader));
                }
                peeked.unwrap()
            }
            PortInner::StringIn { chars, pos } => chars.get(*pos).copied(),
            _ => None,
        }
    }

    pub fn write_str(&self, s: &str) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            PortInner::Stdout => {
                let out = std::io::stdout();
                let mut h = out.lock();
                h.write_all(s.as_bytes()).map_err(|e| e.to_string())?;
                h.flush().map_err(|e| e.to_string())
            }
            PortInner::FileOut(w) => w.write_all(s.as_bytes()).map_err(|e| e.to_string()),
            PortInner::StringOut(buf) => {
                buf.push_str(s);
                Ok(())
            }
            _ => Err("write: not an output port".into()),
        }
    }

    pub fn flush(&self) -> Result<(), String> {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            PortInner::Stdout => {
                let out = std::io::stdout();
                let mut h = out.lock();
                h.flush().map_err(|e| e.to_string())
            }
            PortInner::FileOut(w) => w.flush().map_err(|e| e.to_string()),
            _ => Ok(()),
        }
    }

    pub fn close(&self) {
        let mut inner = self.inner.borrow_mut();
        if matches!(&*inner, PortInner::FileOut(_)) {
            // flush by replacing
            let old = std::mem::replace(&mut *inner, PortInner::Closed);
            if let PortInner::FileOut(mut w) = old {
                let _ = w.flush();
            }
        } else {
            *inner = PortInner::Closed;
        }
    }

    pub fn get_output_string(&self) -> Result<String, String> {
        let inner = self.inner.borrow();
        match &*inner {
            PortInner::StringOut(s) => Ok(s.clone()),
            _ => Err("get-output-string: not a string output port".into()),
        }
    }
}

fn read_one_char(r: &mut BufReader<File>) -> Option<char> {
    // UTF-8 aware single char read
    let mut first = [0u8; 1];
    match r.read(&mut first) {
        Ok(0) => None,
        Ok(_) => {
            let b = first[0];
            if b < 0x80 {
                Some(b as char)
            } else {
                let len = if b >= 0xF0 {
                    3
                } else if b >= 0xE0 {
                    2
                } else {
                    1
                };
                let mut rest = vec![0u8; len];
                if r.read_exact(&mut rest).is_err() {
                    return None;
                }
                let mut bytes = vec![b];
                bytes.extend(rest);
                String::from_utf8(bytes).ok()?.chars().next()
            }
        }
        Err(_) => None,
    }
}

thread_local! {
    static CURRENT_IN: RefCell<Rc<Port>> = RefCell::new(Port::stdin());
    static CURRENT_OUT: RefCell<Rc<Port>> = RefCell::new(Port::stdout());
}

pub fn current_input() -> Rc<Port> {
    CURRENT_IN.with(|p| p.borrow().clone())
}

pub fn current_output() -> Rc<Port> {
    CURRENT_OUT.with(|p| p.borrow().clone())
}

pub fn set_current_input(p: Rc<Port>) {
    CURRENT_IN.with(|c| *c.borrow_mut() = p);
}

pub fn set_current_output(p: Rc<Port>) {
    CURRENT_OUT.with(|c| *c.borrow_mut() = p);
}
