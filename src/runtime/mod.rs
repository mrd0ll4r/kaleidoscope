use crate::Result;
use alloy::api::TimestampedInputValue;
use alloy::config::{InputValue, UniverseConfig};
use alloy::event::{AddressedEvent, EventKind};
use alloy::Address;
use anyhow::{anyhow, ensure};
use rlua::{Context, ToLua, Value};
use std::collections::HashMap;

mod event;
mod globals;
mod parameters;
mod program;
pub mod runtime;

#[derive(Debug, Clone)]
struct UniverseView {
    v: HashMap<Address, Option<std::result::Result<InputValue, String>>>,
}

impl UniverseView {
    fn new_with_addresses(addresses: &Vec<Address>) -> UniverseView {
        UniverseView {
            v: addresses.iter().map(|addr| (*addr, None)).collect(),
        }
    }

    fn new_from_universe_config(config: &UniverseConfig) -> UniverseView {
        let addresses = config
            .devices
            .iter()
            .flat_map(|dev| dev.inputs.iter().map(|input| input.address))
            .chain(
                config
                    .devices
                    .iter()
                    .flat_map(|device| device.outputs.iter().map(|output| output.address)),
            )
            .collect();
        Self::new_with_addresses(&addresses)
    }

    fn apply_initial_values(
        &mut self,
        values: HashMap<Address, Option<TimestampedInputValue>>,
    ) -> Result<()> {
        for (addr, value) in values.into_iter() {
            if let Some(value) = value {
                ensure!(self.v.contains_key(&addr), "missing address {}", addr);

                self.v.insert(addr, Some(value.value));
            }
        }

        Ok(())
    }

    fn apply_event(&mut self, event: &AddressedEvent) -> Result<()> {
        let v = self
            .v
            .get_mut(&event.address)
            .ok_or(anyhow!("missing address"))?;

        match event.event.inner.clone() {
            Ok(e) => {
                if let EventKind::Update { new_value } = e {
                    *v = Some(Ok(new_value));
                }
            }
            Err(err) => *v = Some(Err(err)),
        }

        Ok(())
    }

    fn has_address(&self, address: Address) -> bool {
        self.v.contains_key(&address)
    }
}

impl<'lua> UniverseView {
    fn to_lua(&self, lua: Context<'lua>) -> rlua::Result<Value<'lua>> {
        let t = lua.create_table()?;
        for (k, v) in &self.v {
            match v {
                None => {
                    t.set(*k, rlua::Value::Nil)?;
                }
                Some(v) => match v {
                    Err(err) => {
                        t.set(*k, rlua::Value::Nil)?;
                        t.set(format!("{}-err", k), lua.create_string(&err)?)?;
                    }
                    Ok(v) => {
                        let v = match v {
                            InputValue::Binary(b) => b.to_lua(lua)?,
                            InputValue::Temperature(t) => t.to_lua(lua)?,
                            InputValue::Humidity(h) => h.to_lua(lua)?,
                            InputValue::Pressure(p) => p.to_lua(lua)?,
                            InputValue::Continuous(c) => c.to_lua(lua)?,
                        };
                        t.set(*k, v)?;
                    }
                },
            }
        }
        Ok(rlua::Value::Table(t))
    }
}
