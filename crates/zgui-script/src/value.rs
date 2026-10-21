//! Values a script can store in Rust-owned state.
use mlua::{FromLua, IntoLua, Lua, UserData, UserDataMethods};
use std::{cell::RefCell, collections::HashMap, rc::Rc};
use zgui::reactive::{Runtime, Signal};

/// A script value. State holds these, so equal writes are suppressed like any
/// other signal.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Value {
    #[default]
    Nil,
    Bool(bool),
    Num(f64),
    Str(Rc<str>),
    List(Rc<[Value]>),
    /// A table with string keys, sorted by key so equal tables compare equal.
    Map(Rc<[(Rc<str>, Value)]>),
}
impl Value {
    pub fn truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Num(n) => Some(*n),
            Value::Bool(b) => Some(*b as u8 as f64),
            _ => None,
        }
    }
    pub fn list(&self) -> &[Value] {
        match self {
            Value::List(items) => items,
            _ => &[],
        }
    }
    /// A field of a map value.
    pub fn field(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Map(fields) => fields
                .binary_search_by(|(k, _)| (**k).cmp(key))
                .ok()
                .map(|i| &fields[i].1),
            _ => None,
        }
    }
}
impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Nil => Ok(()),
            Value::Bool(b) => write!(f, "{b}"),
            Value::Num(n) if n.fract() == 0. && n.abs() < 1e15 => write!(f, "{}", *n as i64),
            Value::Num(n) => write!(f, "{n}"),
            Value::Str(s) => f.write_str(s),
            Value::List(items) => {
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{item}")?;
                }
                Ok(())
            }
            Value::Map(fields) => {
                f.write_str("{")?;
                for (index, (key, value)) in fields.iter().enumerate() {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{key} = {value}")?;
                }
                f.write_str("}")
            }
        }
    }
}
impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Value::Str(value.into())
    }
}
impl From<f64> for Value {
    fn from(value: f64) -> Self {
        Value::Num(value)
    }
}
impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Value::Bool(value)
    }
}
impl FromLua for Value {
    fn from_lua(value: mlua::Value, _: &Lua) -> mlua::Result<Self> {
        Ok(match value {
            mlua::Value::Nil => Value::Nil,
            mlua::Value::Boolean(b) => Value::Bool(b),
            mlua::Value::Integer(n) => Value::Num(n as f64),
            mlua::Value::Number(n) => Value::Num(n),
            mlua::Value::String(s) => Value::Str(s.to_string_lossy().into()),
            mlua::Value::Table(t) => {
                let len = t.raw_len();
                let mut fields: Vec<(Rc<str>, Value)> = Vec::new();
                let mut items = Vec::with_capacity(len);
                for pair in t.pairs::<mlua::Value, Value>() {
                    let (key, value) = pair?;
                    match key {
                        mlua::Value::Integer(i) if i >= 1 && i as usize <= len => {}
                        mlua::Value::Number(n)
                            if n.fract() == 0. && n >= 1. && n as usize <= len => {}
                        mlua::Value::String(k) => fields.push((k.to_string_lossy().into(), value)),
                        other => {
                            return Err(mlua::Error::runtime(format!(
                                "state tables are lists or have string keys, not {} keys",
                                other.type_name()
                            )));
                        }
                    }
                }
                for item in t.sequence_values::<Value>() {
                    items.push(item?);
                }
                match (items.is_empty(), fields.is_empty()) {
                    (_, true) => Value::List(items.into()),
                    (true, false) => {
                        fields.sort_by(|a, b| a.0.cmp(&b.0));
                        Value::Map(fields.into())
                    }
                    (false, false) => {
                        return Err(mlua::Error::runtime(
                            "a state table is a list or has string keys, not both",
                        ));
                    }
                }
            }
            mlua::Value::UserData(ud) if ud.is::<State>() => ud.borrow::<State>()?.0.get(),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "state holds nil, booleans, numbers, strings, lists and tables, not {}",
                    other.type_name()
                )));
            }
        })
    }
}
impl IntoLua for Value {
    fn into_lua(self, lua: &Lua) -> mlua::Result<mlua::Value> {
        match self {
            Value::Nil => Ok(mlua::Value::Nil),
            Value::Bool(b) => Ok(mlua::Value::Boolean(b)),
            Value::Num(n) => Ok(mlua::Value::Number(n)),
            Value::Str(s) => lua.create_string(&*s).map(mlua::Value::String),
            Value::List(items) => lua
                .create_sequence_from(items.iter().cloned())
                .map(mlua::Value::Table),
            Value::Map(fields) => {
                let table = lua.create_table_with_capacity(0, fields.len())?;
                for (key, value) in fields.iter() {
                    table.raw_set(&**key, value.clone())?;
                }
                Ok(mlua::Value::Table(table))
            }
        }
    }
}

/// A handle to Rust-owned state. Scripts read it with `:get()`, which
/// subscribes the reading text or style, and write it from event handlers.
#[derive(Clone)]
pub struct State(pub Signal<Value>);
impl State {
    /// Sets the value; what it updates runs off the script's clock.
    fn write(&self, lua: &mlua::Lua, value: Value) {
        let vm = lua
            .app_data_ref::<crate::node::Env>()
            .and_then(|env| env.vm.upgrade());
        match vm {
            Some(vm) => vm.off_clock(|| self.0.set(value)),
            None => self.0.set(value),
        };
    }
}
impl UserData for State {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |_, s, ()| Ok(s.0.get()));
        methods.add_method("peek", |_, s, ()| Ok(s.0.with_untracked(Clone::clone)));
        methods.add_method("set", |lua, s, value: Value| {
            s.write(lua, value);
            Ok(())
        });
        methods.add_method("update", |lua, s, f: mlua::Function| {
            let next: Value = f.call(s.0.with_untracked(Clone::clone))?;
            s.write(lua, next);
            Ok(())
        });
        methods.add_method("toggle", |lua, s, ()| {
            s.write(lua, Value::Bool(!s.0.with_untracked(Value::truthy)));
            Ok(())
        });
        methods.add_method("add", |lua, s, by: Option<f64>| {
            let n = s.0.with_untracked(|v| v.as_f64().unwrap_or(0.));
            s.write(lua, Value::Num(n + by.unwrap_or(1.)));
            Ok(())
        });
        methods.add_method("push", |lua, s, item: Value| {
            let mut items = s.0.with_untracked(|v| v.list().to_vec());
            items.push(item);
            s.write(lua, Value::List(items.into()));
            Ok(())
        });
        methods.add_method("remove", |lua, s, index: usize| {
            let mut items = s.0.with_untracked(|v| v.list().to_vec());
            let removed = (1..=items.len())
                .contains(&index)
                .then(|| items.remove(index - 1));
            s.write(lua, Value::List(items.into()));
            Ok(removed.unwrap_or_default())
        });
        methods.add_method("len", |_, s, ()| Ok(s.0.with(|v| v.list().len())));
    }
}

/// Keyed state that outlives any one evaluation of a script, so a reload keeps
/// counters, toggles and lists where they were.
pub struct Store {
    runtime: Runtime,
    signals: RefCell<HashMap<String, Signal<Value>>>,
}
impl Store {
    pub fn new(runtime: &Runtime) -> Self {
        Self {
            runtime: runtime.clone(),
            signals: RefCell::default(),
        }
    }
    /// The existing state for `key`, or new state starting at `initial`.
    pub fn state(&self, key: &str, initial: Value) -> Signal<Value> {
        self.signals
            .borrow_mut()
            .entry(key.to_owned())
            .or_insert_with(|| self.runtime.signal(initial))
            .clone()
    }
    pub fn get(&self, key: &str) -> Option<Signal<Value>> {
        self.signals.borrow().get(key).cloned()
    }
}
