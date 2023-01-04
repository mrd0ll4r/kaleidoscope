use rlua::{Context, FromLua, ToLua, Value};
use std::collections::HashMap;
use std::mem;

#[derive(Clone, Debug)]
pub struct ParameterTable {
    program_parameters: HashMap<String, ProgramParameters>,
    deltas: HashMap<String, HashMap<String, ParameterDelta>>,
}

#[derive(Clone, Debug, Default)]
pub struct ProgramParameters {
    parameters: HashMap<String, ProgramParameter>,
}

#[derive(Clone, Debug)]
pub struct ProgramParameter {
    pub description: String,
    pub parameter: ParameterType,
}

#[derive(Clone, Debug)]
pub enum ParameterType {
    Continuous(ContinuousProgramParameter),
    Discrete(DiscreteProgramParameter),
}

#[derive(Clone, Debug)]
pub enum ParameterDelta {
    Continuous(f64),
    Discrete(i32),
}

impl ParameterTable {
    pub(crate) fn new() -> ParameterTable {
        Self {
            program_parameters: Default::default(),
            deltas: Default::default(),
        }
    }

    pub(crate) fn get_deltas(&mut self) -> HashMap<String, HashMap<String, ParameterDelta>> {
        mem::take(&mut self.deltas)
    }

    fn get_parameter_mut(
        &mut self,
        program_name: &str,
        parameter_name: &str,
    ) -> crate::Result<&mut ProgramParameter> {
        self.program_parameters
            .get_mut(program_name)
            .and_then(|params| params.parameters.get_mut(parameter_name))
            .ok_or(format_err!(
                "unknown parameter {} for program {}",
                parameter_name,
                program_name
            ))
    }

    fn get_parameter(
        &self,
        program_name: &str,
        parameter_name: &str,
    ) -> crate::Result<&ProgramParameter> {
        self.program_parameters
            .get(program_name)
            .and_then(|params| params.parameters.get(parameter_name))
            .ok_or(format_err!(
                "unknown parameter {} for program {}",
                parameter_name,
                program_name
            ))
    }

    pub fn increment_discrete_parameter(
        &mut self,
        program_name: &str,
        parameter_name: &str,
        delta: i32,
    ) -> crate::Result<()> {
        let param = self.get_parameter_mut(program_name, parameter_name)?;

        let new_val = match param.parameter {
            ParameterType::Continuous(_) => {
                bail!(
                    "parameter {} for program {} is continuous",
                    program_name,
                    parameter_name
                )
            }
            ParameterType::Discrete(ref mut param) => param.try_inc_by(delta)?,
        };

        self.deltas
            .entry(program_name.to_string())
            .or_default()
            .insert(
                parameter_name.to_string(),
                ParameterDelta::Discrete(new_val),
            );

        Ok(())
    }

    pub fn set_discrete_parameter(
        &mut self,
        program_name: &str,
        parameter_name: &str,
        value: i32,
    ) -> crate::Result<()> {
        let param = self.get_parameter_mut(program_name, parameter_name)?;

        match param.parameter {
            ParameterType::Continuous(_) => {
                bail!(
                    "parameter {} for program {} is continuous",
                    program_name,
                    parameter_name
                )
            }
            ParameterType::Discrete(ref mut param) => param.try_set(value),
        }?;

        self.deltas
            .entry(program_name.to_string())
            .or_default()
            .insert(parameter_name.to_string(), ParameterDelta::Discrete(value));

        Ok(())
    }

    pub fn set_continuous_parameter(
        &mut self,
        program_name: &str,
        parameter_name: &str,
        value: f64,
    ) -> crate::Result<()> {
        let param = self.get_parameter_mut(program_name, parameter_name)?;

        match param.parameter {
            ParameterType::Continuous(ref mut param) => param.try_set(value)?,
            ParameterType::Discrete(_) => {
                bail!(
                    "parameter {} for program {} is discrete",
                    program_name,
                    parameter_name
                )
            }
        };

        self.deltas
            .entry(program_name.to_string())
            .or_default()
            .insert(
                parameter_name.to_string(),
                ParameterDelta::Continuous(value),
            );

        Ok(())
    }

    pub fn get_discrete_parameter_value(
        &self,
        program_name: &str,
        parameter_name: &str,
    ) -> crate::Result<i32> {
        match self.get_parameter(program_name, parameter_name)?.parameter {
            ParameterType::Continuous(_) => {
                bail!(
                    "parameter {} for program {} is continuous",
                    program_name,
                    parameter_name
                )
            }
            ParameterType::Discrete(ref param) => Ok(param.current),
        }
    }

    pub fn get_continuous_parameter_value(
        &self,
        program_name: &str,
        parameter_name: &str,
    ) -> crate::Result<f64> {
        match self.get_parameter(program_name, parameter_name)?.parameter {
            ParameterType::Continuous(ref param) => Ok(param.current),
            ParameterType::Discrete(_) => {
                bail!(
                    "parameter {} for program {} is discrete",
                    program_name,
                    parameter_name
                )
            }
        }
    }

    fn has_parameter_for_program(&self, program_name: &str, parameter_name: &str) -> bool {
        self.program_parameters
            .get(program_name)
            .map(|params| params.parameters.contains_key(parameter_name))
            .unwrap_or(false)
    }

    pub fn declare_discrete_parameter(
        &mut self,
        program_name: String,
        parameter_name: String,
        description: String,
        values: Vec<DiscreteParameterValue>,
        initial_value: i32,
    ) -> crate::Result<()> {
        // Check for duplicates.
        if self.has_parameter_for_program(&program_name, &parameter_name) {
            bail!(
                "duplicate parameter name {} for program {}",
                parameter_name,
                program_name
            );
        }

        // Check if initial value is valid.
        let mut param = DiscreteProgramParameter { values, current: 0 };
        param.try_set(initial_value)?;

        // TODO Check for duplicates in the possible values?

        // Add parameter.
        self.program_parameters
            .entry(program_name)
            .or_default()
            .parameters
            .insert(
                parameter_name,
                ProgramParameter {
                    description,
                    parameter: ParameterType::Discrete(param),
                },
            );

        Ok(())
    }

    pub fn declare_continuous_parameter(
        &mut self,
        program_name: String,
        parameter_name: String,
        description: String,
        lower: f64,
        upper: f64,
        initial: f64,
    ) -> crate::Result<()> {
        // Check for duplicates.
        if self.has_parameter_for_program(&program_name, &parameter_name) {
            bail!(
                "duplicate parameter name {} for program {}",
                parameter_name,
                program_name
            );
        }

        // Check if initial value is valid.
        let mut param = ContinuousProgramParameter {
            lower,
            upper,
            current: 0_f64,
        };
        param.try_set(initial)?;

        // Add parameter.
        self.program_parameters
            .entry(program_name)
            .or_default()
            .parameters
            .insert(
                parameter_name,
                ProgramParameter {
                    description,
                    parameter: ParameterType::Continuous(param),
                },
            );

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DiscreteProgramParameter {
    pub values: Vec<DiscreteParameterValue>,
    pub current: i32,
}

impl DiscreteProgramParameter {
    fn try_inc_by(&mut self, delta: i32) -> crate::Result<i32> {
        let new_val = (self.current + delta) % self.values.len() as i32;

        self.try_set(new_val)
    }

    fn try_set(&mut self, to_value: i32) -> crate::Result<i32> {
        if !self.values.iter().any(|v| v.value == to_value) {
            bail!(
                "invalid value {}, expected one of {:?}",
                to_value,
                self.values
            )
        }

        self.current = to_value;

        Ok(to_value)
    }
}

#[derive(Clone, Debug)]
pub struct ContinuousProgramParameter {
    pub lower: f64,
    pub upper: f64,
    pub current: f64,
}

impl ContinuousProgramParameter {
    fn try_set(&mut self, to_value: f64) -> crate::Result<()> {
        if to_value > self.upper || to_value < self.lower {
            bail!(
                "value {} out of [{}, {}] bounds",
                to_value,
                self.lower,
                self.upper
            )
        }

        self.current = to_value;

        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct DiscreteParameterValue {
    pub label: String,
    pub value: i32,
}

impl<'lua> FromLua<'lua> for DiscreteParameterValue {
    fn from_lua(lua_value: Value<'lua>, _: Context<'lua>) -> rlua::Result<Self> {
        match lua_value {
            Value::Table(table) => {
                let label: String = table.get("_label")?;
                let value: i32 = table.get("_value")?;
                Ok(DiscreteParameterValue { label, value })
            }
            _ => Err(rlua::Error::FromLuaConversionError {
                from: "unknown",
                to: "DiscreteParameterValue",
                message: Some("expected table with _label and _value keys".to_string()),
            }),
        }
    }
}

impl<'lua> ToLua<'lua> for DiscreteParameterValue {
    fn to_lua(self, lua: Context<'lua>) -> rlua::Result<Value<'lua>> {
        let t = lua.create_table()?;
        t.set(lua.create_string("_label")?, self.label.to_lua(lua)?)?;
        t.set(lua.create_string("_value")?, self.value.to_lua(lua)?)?;
        Ok(rlua::Value::Table(t))
    }
}
