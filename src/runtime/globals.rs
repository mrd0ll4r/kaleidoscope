use rlua::{Context, FromLua, Integer, Number, ToLua, Value};
use std::collections::hash_map::IntoIter;
use std::collections::HashMap;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum GlobalValue {
    /// The Lua value `nil`.
    Nil,
    /// The Lua value `true` or `false`.
    Boolean(bool),
    /// An integer number.
    ///
    /// Any Lua number convertible to a `Integer` will be represented as this variant.
    Integer(Integer),
    /// A floating point number.
    Number(Number),
    /// An interned string, managed by Lua.
    ///
    /// Unlike Rust strings, Lua strings may not be valid UTF-8.
    String(String),
}

impl<'lua> FromLua<'lua> for GlobalValue {
    fn from_lua(lua_value: Value<'lua>, _: Context<'lua>) -> rlua::Result<Self> {
        match lua_value {
            Value::Nil => Ok(GlobalValue::Nil),
            Value::Boolean(b) => Ok(GlobalValue::Boolean(b)),
            Value::Integer(i) => Ok(GlobalValue::Integer(i)),
            Value::Number(n) => Ok(GlobalValue::Number(n)),
            Value::String(s) => Ok(GlobalValue::String(s.to_str()?.to_string())),
            _ => Err(rlua::Error::FromLuaConversionError {
                from: "unknown",
                to: "GlobalValue",
                message: Some("expected nil, boolean, integer, number, or string".to_string()),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct DeltaTable {
    v: HashMap<String, GlobalValue>,
}

impl DeltaTable {
    pub fn from_map(m: HashMap<String, GlobalValue>) -> Self {
        DeltaTable { v: m }
    }
}

impl IntoIterator for DeltaTable {
    type Item = (String, GlobalValue);
    type IntoIter = IntoIter<String, GlobalValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.v.into_iter()
    }
}

impl<'lua> FromLua<'lua> for DeltaTable {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> rlua::Result<Self> {
        let v: HashMap<String, GlobalValue> = HashMap::from_lua(value, lua)?;
        Ok(DeltaTable { v })
    }
}

impl<'lua> DeltaTable {
    pub(crate) fn to_lua(&self, lua: Context<'lua>) -> rlua::Result<rlua::Value<'lua>> {
        let t = lua.create_table()?;
        for (k, v) in &self.v {
            let val = match v {
                GlobalValue::Nil => Value::Nil,
                GlobalValue::Boolean(b) => b.to_lua(lua)?,
                GlobalValue::Integer(i) => i.to_lua(lua)?,
                GlobalValue::Number(n) => n.to_lua(lua)?,
                GlobalValue::String(s) => Value::String(lua.create_string(s.as_str())?),
                /*Value::Table(table) => {
                    let mut tab = lua.create_table()?;

                }*/
            };
            t.set(lua.create_string(k)?, val)?;
        }
        Ok(rlua::Value::Table(t))
    }
}
