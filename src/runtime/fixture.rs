use crate::runtime::runtime::TickState;
use alloy::api::{SetRequest, SetRequestTarget};
use alloy::config::UniverseConfig;
use alloy::program::ParameterSetRequest;
use alloy::{Address, OutputValue, HIGH, LOW};
use anyhow::{anyhow, bail, ensure, Context, Result};
use chrono::Timelike;
use lazy_static::lazy_static;
use log::{debug, trace};
use mlua::{Function, IntoLua, Lua, Table};
use noise::{NoiseFn, Perlin};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Number of ticks to skip execution for slow-mode programs.
const SLOW_MODE_NUM_SKIP_TICKS: usize = 999;

/// Parameter type constants.
/// Must be in sync with Lua builtins!
const PARAMETER_TYPE_DISCRETE: &str = "discrete";
const PARAMETER_TYPE_CONTINUOUS: &str = "continuous";

/// Runtime version.
const VERSION: u16 = 3;

lazy_static! {
    pub static ref PERLIN: Perlin = Perlin::new(0);
}

const FIXTURE_BUILTIN_SOURCE: &'static str = include_str!("lua/fixture_builtin.lua");
const PROGRAM_BUILTIN_SOURCE: &'static str = include_str!("lua/program_builtin.lua");

pub(crate) struct Fixture {
    pub(crate) name: String,
    pub(crate) source_path: PathBuf,
    pub(crate) addresses: HashSet<Address>,
    programs: Vec<FixtureProgram>,
    current_program_index: usize,
}

impl Fixture {
    pub(crate) fn new<P: AsRef<Path>>(
        source: P,
        universe_config: &UniverseConfig,
    ) -> Result<Fixture> {
        let base_path = source
            .as_ref()
            .parent()
            .and_then(|p| Some(p.to_path_buf()))
            .unwrap_or_else(PathBuf::new);

        // Load and setup fixture
        let setup_values = Self::setup_fixture(source.as_ref(), universe_config)
            .context("unable to set up fixture")?;
        debug!(
            "set up fixture at {:?}: {:?}",
            source.as_ref(),
            setup_values
        );

        let output_aliases: HashMap<_, _> = universe_config
            .devices
            .iter()
            .flat_map(|d| &d.outputs)
            .filter(|o| setup_values.outputs.contains(&o.address))
            .map(|ref o| (o.alias.clone(), o.address))
            .collect();

        // Load and setup programs
        let mut lua_programs = Vec::new();
        for (program_name, program_source) in setup_values.program_sources.iter() {
            let program_source_path = base_path.clone().join(program_source);

            let program =
                LuaFixtureProgram::new(&program_source_path, output_aliases.clone(), 0).context(
                    format!("unable to load program at {:?}", program_source_path),
                )?;

            lua_programs.push((program_name.clone(), program))
        }

        let mut programs = Vec::new();

        // Add builtin programs
        if !setup_values.disable_builtin_programs {
            programs.push(FixtureProgram {
                name: "OFF".to_string(),
                inner: FixtureProgramType::BundledConstant(
                    BundledConstantFixtureProgram::new_fixed_value(
                        setup_values.outputs.clone(),
                        LOW,
                    ),
                ),
            });
            programs.push(FixtureProgram {
                name: "ON".to_string(),
                inner: FixtureProgramType::BundledConstant(
                    BundledConstantFixtureProgram::new_fixed_value(
                        setup_values.outputs.clone(),
                        HIGH,
                    ),
                ),
            });
        }

        // Add EXTERNAL program, indicating the fixture is not controlled by Kaleidoscope.
        programs.push(FixtureProgram {
            name: "EXTERNAL".to_string(),
            inner: FixtureProgramType::External,
        });

        // Add MANUAL program, with parameters for all aliases
        if !setup_values.disable_manual_program {
            programs.push(FixtureProgram {
                name: "MANUAL".to_string(),
                inner: FixtureProgramType::BundledManual(BundledManualFixtureProgram::new(
                    output_aliases.clone(),
                )),
            });
        }

        // Add Lua programs
        for (name, program) in lua_programs.into_iter() {
            if programs.iter().find(|p| p.name == name).is_some() {
                bail!("duplicate/conflicting program name: {}", name)
            }
            programs.push(FixtureProgram {
                name,
                inner: FixtureProgramType::Lua(program),
            })
        }

        ensure!(
            !programs.is_empty(),
            "no programs defined and builtin programs disabled"
        );

        Ok(Fixture {
            name: setup_values.name,
            addresses: setup_values.outputs,
            source_path: source.as_ref().to_path_buf(),
            programs,
            current_program_index: 0,
        })
    }

    pub(crate) fn get_program(&self, name: &str) -> Option<&FixtureProgram> {
        self.programs.iter().find(|p| p.name == name)
    }

    pub(crate) fn get_program_mut(&mut self, name: &str) -> Option<&mut FixtureProgram> {
        self.programs.iter_mut().find(|p| p.name == name)
    }

    pub(crate) fn alloy_metadata(
        &self,
        universe_config: &UniverseConfig,
    ) -> alloy::program::FixtureMetadata {
        alloy::program::FixtureMetadata {
            programs: self
                .programs
                .iter()
                .map(|p| (p.name.clone(), p.alloy_metadata()))
                .collect(),
            selected_program: self
                .programs
                .get(self.current_program_index)
                .unwrap()
                .name
                .clone(),
            output_aliases: universe_config
                .devices
                .iter()
                .flat_map(|d| &d.outputs)
                .filter(|o| self.addresses.contains(&o.address))
                .map(|ref o| o.alias.clone())
                .collect(),
        }
    }

    fn setup_fixture<P: AsRef<Path>>(
        source: P,
        universe_config: &UniverseConfig,
    ) -> Result<FixtureSetupValues> {
        let lua = Lua::new();
        debug!("loading fixture at {:?}...", source.as_ref());
        let fixture_source = fs::read_to_string(source).context("unable to read fixture source")?;

        lua.load_from_std_lib(mlua::StdLib::TABLE)?;

        let globals = lua.globals();

        lua.load(FIXTURE_BUILTIN_SOURCE)
            .exec()
            .expect("unable to load fixture builtin source");

        // Load program source.
        lua.load(&fixture_source)
            .exec()
            .context("unable to execute builtin source")?;

        // check source version
        let source_version: u16 = globals.get("SOURCE_VERSION")?;
        ensure!(source_version == VERSION, "source version mismatch");

        let setup_values =
            Self::setup(&lua, universe_config).context("failed to execute fixture::setup")?;

        Ok(setup_values)
    }

    pub(crate) fn set_active_program(&mut self, to: &str) -> Result<()> {
        let pos = self
            .programs
            .iter()
            .position(|p| &p.name == to)
            .ok_or(anyhow!("not found"))?;
        self.switch_program(pos)
            .expect("invalid index in set_active_program");
        Ok(())
    }

    pub(crate) fn cycle_active_program(&mut self) -> Result<String> {
        if self.programs.is_empty() {
            bail!("no programs available")
        }
        let mut next_index = (self.current_program_index + 1) % self.programs.len();
        while match self.programs[next_index].name.as_str() {
            "MANUAL" | "EXTERNAL" => true,
            _ => false,
        } {
            // Skip those two
            next_index = (next_index + 1) % self.programs.len();
        }
        self.switch_program(next_index)
            .expect("invalid index in cycle_active_program");
        Ok(self.programs[next_index].name.clone())
    }

    fn switch_program(&mut self, to: usize) -> Result<()> {
        ensure!(to <= self.programs.len(), "invalid index");

        self.current_program_index = to;
        self.programs
            .get_mut(self.current_program_index)
            .unwrap()
            .enable();

        Ok(())
    }

    pub(crate) fn run_current_program(
        &mut self,
        state: &TickState,
        output_requests: &mut Vec<SetRequest>,
    ) -> Result<()> {
        self.programs
            .get_mut(self.current_program_index)
            .unwrap()
            .run(state, output_requests)
    }

    fn setup(lua: &Lua, universe: &UniverseConfig) -> Result<FixtureSetupValues> {
        let mut disable_builtin = false;
        let mut disable_manual = false;
        let mut name = String::new();
        let mut outputs: HashSet<Address> = HashSet::new();
        let mut program_sources: Vec<(String, String)> = Vec::new();
        let output_aliases: HashMap<_, _> = universe
            .devices
            .iter()
            .flat_map(|d| &d.outputs)
            .map(|output| (output.alias.clone(), output.address))
            .collect();

        let globals = lua.globals();
        let setup: Function = globals.get("setup")?;

        lua.scope(|scope| -> mlua::Result<()> {
            // Provide setup-related functions:
            // These are only valid inside the `scope`, in which we will also call `setup()`.

            let set_name = scope.create_function_mut(|_, p_name| {
                name = p_name;
                Ok(())
            })?;
            globals.set("fixture_name", set_name)?;

            let disable_builtin_programs = scope.create_function_mut(|_, p_disable_builtins| {
                disable_builtin = p_disable_builtins;
                Ok(())
            })?;
            globals.set("disable_builtin_programs", disable_builtin_programs)?;

            let disable_manual_program = scope.create_function_mut(|_, p_disable_manual| {
                disable_manual = p_disable_manual;
                Ok(())
            })?;
            globals.set("disable_manual_program", disable_manual_program)?;

            let add_program_source =
                scope.create_function_mut(|_, (program_name, source_path): (String, String)| {
                    if let Some(_) = program_sources
                        .iter()
                        .find(|(name, _)| **name == program_name)
                    {
                        return Err(mlua::Error::external(format!(
                            "duplicate program name: {}",
                            source_path
                        )));
                    }

                    program_sources.push((program_name, source_path));

                    Ok(())
                })?;
            globals.set("add_program", add_program_source)?;

            let add_output_address = scope.create_function_mut(|_, address: Address| {
                if let None = output_aliases.iter().find(|(_a, b)| **b == address) {
                    return Err(mlua::Error::external(format!(
                        "address not in use: {}",
                        address
                    )));
                }

                outputs.insert(address);

                Ok(())
            })?;
            globals.set("add_output_address", add_output_address)?;

            let add_output_alias = scope.create_function(|ctx, alias: String| {
                let addr = *output_aliases
                    .get(&alias)
                    .ok_or_else(|| mlua::Error::external(format!("unknown alias: {}", alias)))?;

                let add_output_address: Function = ctx.globals().get("add_output_address")?;
                add_output_address.call(addr)?;

                Ok(())
            })?;
            globals.set("add_output_alias", add_output_alias)?;

            // Actually call setup
            setup.call(())?;

            Ok(())
        })?;

        Ok(FixtureSetupValues {
            name,
            program_sources,
            outputs,
            disable_builtin_programs: disable_builtin,
            disable_manual_program: disable_manual,
        })
    }
}

#[derive(Clone, Debug)]
struct FixtureSetupValues {
    name: String,
    program_sources: Vec<(String, String)>,
    outputs: HashSet<Address>,
    disable_builtin_programs: bool,
    disable_manual_program: bool,
}

pub(crate) struct FixtureProgram {
    name: String,
    inner: FixtureProgramType,
}

impl FixtureProgram {
    fn enable(&mut self) {
        match &mut self.inner {
            FixtureProgramType::BundledConstant(p) => p.enable(),
            FixtureProgramType::Lua(p) => p.enable(),
            FixtureProgramType::BundledManual(p) => p.enable(),
            FixtureProgramType::External => {}
        }
    }

    fn run(&mut self, state: &TickState, output_requests: &mut Vec<SetRequest>) -> Result<()> {
        match &mut self.inner {
            FixtureProgramType::BundledConstant(p) => p.run(state, output_requests),
            FixtureProgramType::Lua(p) => p.run(state, output_requests),
            FixtureProgramType::BundledManual(p) => p.run(state, output_requests),
            FixtureProgramType::External => {
                // NOP
                Ok(())
            }
        }
    }

    pub(crate) fn alloy_metadata(&self) -> alloy::program::ProgramMetadata {
        match &self.inner {
            FixtureProgramType::BundledConstant(_) | FixtureProgramType::External => {
                alloy::program::ProgramMetadata {
                    parameters: Default::default(),
                }
            }
            FixtureProgramType::Lua(p) => alloy::program::ProgramMetadata {
                parameters: p
                    .parameters
                    .iter()
                    .map(|p| (p.name.clone(), p.alloy_metadata()))
                    .collect(),
            },
            FixtureProgramType::BundledManual(p) => alloy::program::ProgramMetadata {
                parameters: p
                    .parameters
                    .iter()
                    .map(|p| (p.name.clone(), p.alloy_metadata()))
                    .collect(),
            },
        }
    }

    pub(crate) fn get_parameter(&self, name: &str) -> Option<&FixtureProgramParameter> {
        match &self.inner {
            FixtureProgramType::BundledConstant(_) | FixtureProgramType::External => None,
            FixtureProgramType::Lua(p) => p.parameters.iter().find(|param| param.name == name),
            FixtureProgramType::BundledManual(p) => {
                p.parameters.iter().find(|param| param.name == name)
            }
        }
    }

    pub(crate) fn get_parameter_mut(&mut self, name: &str) -> Option<&mut FixtureProgramParameter> {
        match &mut self.inner {
            FixtureProgramType::BundledConstant(_) | FixtureProgramType::External => None,
            FixtureProgramType::Lua(p) => {
                p.dirty_parameters = true;
                p.parameters.iter_mut().find(|param| param.name == name)
            }
            FixtureProgramType::BundledManual(p) => {
                p.dirty_parameters = true;
                p.parameters.iter_mut().find(|param| param.name == name)
            }
        }
    }
}

enum FixtureProgramType {
    BundledConstant(BundledConstantFixtureProgram),
    BundledManual(BundledManualFixtureProgram),
    External,
    Lua(LuaFixtureProgram),
}

struct BundledConstantFixtureProgram {
    addresses: HashSet<Address>,
    output_value: OutputValue,
    reset: bool,
}

impl BundledConstantFixtureProgram {
    fn new_fixed_value(addresses: HashSet<Address>, output: OutputValue) -> Self {
        BundledConstantFixtureProgram {
            addresses,
            output_value: output,
            reset: true,
        }
    }

    fn enable(&mut self) {
        self.reset = true
    }

    fn run(&mut self, _state: &TickState, output_requests: &mut Vec<SetRequest>) -> Result<()> {
        if self.reset {
            debug!(
                "{:?}: was reset, running to set value {} on all outputs",
                self.addresses, self.output_value
            );
            self.reset = false;
            output_requests.extend(self.addresses.iter().map(|addr| SetRequest {
                value: self.output_value,
                target: SetRequestTarget::Address(*addr),
            }))
        }

        Ok(())
    }
}

struct BundledManualFixtureProgram {
    outputs: Vec<Address>,
    parameters: Vec<FixtureProgramParameter>,
    dirty_parameters: bool,
    reset: bool,
}

impl BundledManualFixtureProgram {
    fn new(aliases: HashMap<String, Address>) -> Self {
        let mut tmp = aliases.into_iter().collect::<Vec<_>>();
        tmp.sort_by_key(|(_, addr)| *addr);

        let addresses = tmp.iter().map(|(_, addr)| *addr).collect();
        let parameters = tmp
            .into_iter()
            .map(|(alias, _)| alias)
            .map(|alias| FixtureProgramParameter {
                name: alias,
                value: FixtureProgramParameterType::Continuous {
                    lower_limit_incl: 0.0,
                    upper_limit_incl: 1.0,
                    current: 0.0,
                },
            })
            .collect();

        BundledManualFixtureProgram {
            outputs: addresses,
            parameters,
            dirty_parameters: true,
            reset: true,
        }
    }

    fn enable(&mut self) {
        self.reset = true
    }

    fn run(&mut self, _state: &TickState, output_requests: &mut Vec<SetRequest>) -> Result<()> {
        if !self.reset && !self.dirty_parameters {
            // Nothing to do.
            trace!(
                "{:?}: not running because no change in parameters and not reset",
                self.outputs
            );
            return Ok(());
        }
        debug!("{:?}: reset or change in parameters, running", self.outputs);
        self.reset = false;
        self.dirty_parameters = false;

        // Build output requests from parameter values.
        output_requests.extend(self.outputs.iter().zip(self.parameters.iter()).map(
            |(addr, param)| match param.value {
                FixtureProgramParameterType::Discrete { .. } => {
                    panic!("discrete parameter in builtin manual program")
                }
                FixtureProgramParameterType::Continuous { current, .. } => SetRequest {
                    target: SetRequestTarget::Address(*addr),
                    value: alloy::map_to_value((0.0, 1.0), current),
                },
            },
        ));

        Ok(())
    }
}

struct LuaFixtureProgram {
    parameters: Vec<FixtureProgramParameter>,
    slow_mode: bool,
    skip_ticks_until_next_run: usize,
    dirty_parameters: bool,
    lua: Lua,
    epoch: Instant,
}

impl LuaFixtureProgram {
    fn new<P: AsRef<Path>>(
        source: P,
        output_aliases: HashMap<String, Address>,
        time_of_day: u32,
    ) -> Result<Self> {
        let lua = Lua::new();
        debug!("loading program at {:?}...", source.as_ref());
        let program_source = fs::read_to_string(source.as_ref())?;
        let program_epoch = Instant::now();

        lua.load_from_std_lib(mlua::StdLib::TABLE)?;

        lua.load(PROGRAM_BUILTIN_SOURCE)
            .exec()
            .expect("unable to load program builtin source");

        // Inject a bunch of constants after builtins were loaded, but before the program source
        // is loaded.
        Self::inject_pre_load_constants(&lua, program_epoch, output_aliases)?;

        // Load program source.
        lua.load(&program_source).exec()?;

        // Check source version
        let source_version: u16 = lua.globals().get("SOURCE_VERSION")?;
        ensure!(source_version == VERSION, "source version mismatch");

        let setup_values = Self::setup(&lua, time_of_day).context("unable to set up program")?;
        debug!(
            "set up program at {:?}: {:?}",
            source.as_ref(),
            setup_values
        );

        Ok(LuaFixtureProgram {
            parameters: setup_values.parameters,
            slow_mode: setup_values.slow_mode,
            skip_ticks_until_next_run: 0,
            lua,
            epoch: program_epoch,
            dirty_parameters: true,
        })
    }

    fn inject_pre_load_constants(
        lua: &Lua,
        epoch: Instant,
        output_aliases: HashMap<String, Address>,
    ) -> Result<()> {
        lua.globals()
            .set("output_alias_address", output_aliases)
            .context("unable to set output alias mappings")?;

        lua.globals().set("START", epoch.elapsed().as_secs_f64())?;

        // Inject Perlin noise functions.
        lua.globals().set(
            "noise2d",
            lua.create_function(|_, (x, y): (f64, f64)| Ok(PERLIN.get([x, y])))?,
        )?;
        lua.globals().set(
            "noise3d",
            lua.create_function(|_, (x, y, z): (f64, f64, f64)| Ok(PERLIN.get([x, y, z])))?,
        )?;
        lua.globals().set(
            "noise4d",
            lua.create_function(|_, (x, y, z, t): (f64, f64, f64, f64)| {
                Ok(PERLIN.get([x, y, z, t]))
            })?,
        )?;

        Ok(())
    }

    fn inject_environment(lua: &Lua, time_of_day: u32) -> Result<()> {
        lua.globals()
            .set("TIME_OF_DAY", time_of_day)
            .context("unable to set time of day")?;

        Ok(())
    }

    fn setup(lua: &Lua, time_of_day: u32) -> Result<ProgramSetupValues> {
        let mut slow_mode = false;
        let mut parameters: Vec<FixtureProgramParameter> = Vec::new();

        // Inject inputs
        Self::inject_environment(lua, time_of_day).context("unable to inject environment")?;

        // Run setup
        let globals = lua.globals();
        let setup: Function = globals.get("setup")?;

        lua.scope(|scope| -> mlua::Result<()> {
            // Provide setup-related functions:
            // These are only valid inside of the `scope`, in which we will also call `setup()`.
            // This is good, because these are setup-related and we don't want users to call them
            // from `tick()`.

            let declare_parameter_generic =
                scope.create_function_mut(|_, parameter_table: Table| {
                    let param_name: String = parameter_table.get("_name")?;
                    if parameters.iter().find(|p| *p.name == param_name).is_some() {
                        return Err(mlua::Error::external(format!(
                            "duplicate parameter name: {}",
                            param_name
                        )));
                    }

                    let param_type: String = parameter_table.get("_type")?;
                    match param_type.as_str() {
                        PARAMETER_TYPE_CONTINUOUS => {
                            let lower: f64 = parameter_table.get("_lower")?;
                            let upper: f64 = parameter_table.get("_upper")?;
                            let default: f64 = parameter_table.get("_default")?;

                            parameters.push(FixtureProgramParameter {
                                name: param_name,
                                value: FixtureProgramParameterType::Continuous {
                                    lower_limit_incl: lower,
                                    upper_limit_incl: upper,
                                    current: default,
                                },
                            });
                        }
                        PARAMETER_TYPE_DISCRETE => {
                            let num_levels: usize = parameter_table.get("_i")?;

                            let mut levels = Vec::new();
                            let levels_val: Table = parameter_table.get("_levels")?;
                            for i in 0..num_levels {
                                let level_table: Table = levels_val.get(i)?;
                                let level_name: String = level_table.get("_name")?;
                                let level_desc: String = level_table.get("_desc")?;

                                levels.push(FixtureProgramParameterDiscreteLevel {
                                    name: level_name,
                                    description: level_desc,
                                })
                            }

                            if levels.is_empty() {
                                return Err(mlua::Error::external(anyhow!(
                                    "missing levels for discrete parameter {}",
                                    param_name
                                )));
                            }

                            parameters.push(FixtureProgramParameter {
                                name: param_name,
                                value: FixtureProgramParameterType::Discrete {
                                    levels,
                                    current_index: 0,
                                },
                            });
                        }
                        _ => {
                            return Err(mlua::Error::external(anyhow!(
                                "invalid parameter type: {}",
                                param_type
                            )))
                        }
                    }

                    Ok(())
                })?;
            globals.set("_declare_parameter_generic", declare_parameter_generic)?;

            let set_slow_mode = scope.create_function_mut(|_, p_slow_mode| {
                slow_mode = p_slow_mode;
                Ok(())
            })?;
            globals.set("set_slow_mode", set_slow_mode)?;

            // Actually call setup
            setup.call(())?;

            Ok(())
        })?;

        Ok(ProgramSetupValues {
            parameters,
            slow_mode,
        })
    }

    fn inject_parameters(&mut self) -> Result<()> {
        if !self.dirty_parameters {
            return Ok(());
        }

        let t: HashMap<_, _> = self
            .parameters
            .iter()
            .map(|p| {
                match &p.value {
                    FixtureProgramParameterType::Discrete {
                        levels,
                        current_index,
                    } => levels
                        .get(*current_index)
                        .unwrap()
                        .name
                        .clone()
                        .into_lua(&self.lua),
                    FixtureProgramParameterType::Continuous { current, .. } => {
                        current.into_lua(&self.lua)
                    }
                }
                .map(|v| (p.name.clone(), v))
            })
            .collect::<mlua::Result<HashMap<_, _>>>()
            .context("unable to build parameter table")?;

        self.lua
            .globals()
            .set("_parameter_values", t)
            .context("unable to set parameter value global")?;

        self.dirty_parameters = false;
        Ok(())
    }

    fn enable(&mut self) {
        self.skip_ticks_until_next_run = 0
    }

    fn run(&mut self, state: &TickState, output_requests: &mut Vec<SetRequest>) -> Result<()> {
        if self.skip_ticks_until_next_run == 0 || self.dirty_parameters {
            // Update parameters
            self.inject_parameters()
                .context("unable to inject parameters")?;

            // Inject environment
            let time_of_day = state.local_time.hour() * 60 * 60
                + state.local_time.minute() * 60
                + state.local_time.second();
            Self::inject_environment(&self.lua, time_of_day)?;

            // Run tick
            let output_values_by_address: mlua::Result<HashMap<Address, OutputValue>> = {
                let globals = self.lua.globals();

                let now = state.timestamp.duration_since(self.epoch).as_secs_f64();
                let tick: Function = globals.get("_tick")?;

                tick.call(now)
            };
            debug!("_tick returned {:?}", output_values_by_address);

            let output_values = output_values_by_address.context("failed to execute _tick")?;
            output_requests.extend(output_values.into_iter().map(|(addr, val)| SetRequest {
                value: val,
                target: SetRequestTarget::Address(addr),
            }));

            if self.slow_mode {
                self.skip_ticks_until_next_run = SLOW_MODE_NUM_SKIP_TICKS;
            }
        } else {
            self.skip_ticks_until_next_run -= 1;
        }

        Ok(())
    }
}

#[derive(Clone, Debug)]
struct ProgramSetupValues {
    parameters: Vec<FixtureProgramParameter>,
    slow_mode: bool,
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct FixtureProgramParameter {
    name: String,
    value: FixtureProgramParameterType,
}

impl FixtureProgramParameter {
    pub(crate) fn alloy_metadata(&self) -> alloy::program::ProgramParameter {
        match &self.value {
            FixtureProgramParameterType::Discrete {
                levels,
                current_index,
            } => alloy::program::ProgramParameter {
                inner: alloy::program::ParameterType::Discrete {
                    levels: levels
                        .clone()
                        .into_iter()
                        .map(|l| {
                            (
                                l.name,
                                alloy::program::DiscreteParameterLevel {
                                    description: l.description,
                                },
                            )
                        })
                        .collect(),
                    current_level: levels.get(*current_index).unwrap().name.clone(),
                },
            },
            FixtureProgramParameterType::Continuous {
                lower_limit_incl,
                upper_limit_incl,
                current,
            } => alloy::program::ProgramParameter {
                inner: alloy::program::ParameterType::Continuous {
                    lower_limit_incl: *lower_limit_incl,
                    upper_limit_incl: *upper_limit_incl,
                    current: *current,
                },
            },
        }
    }

    pub(crate) fn set(&mut self, to: ParameterSetRequest) -> Result<()> {
        self.value.set(to)
    }

    pub(crate) fn cycle(&mut self) -> Result<String> {
        self.value.cycle()
    }
}

#[derive(Clone, Debug, Serialize)]
enum FixtureProgramParameterType {
    Discrete {
        levels: Vec<FixtureProgramParameterDiscreteLevel>,
        current_index: usize,
    },
    Continuous {
        lower_limit_incl: f64,
        upper_limit_incl: f64,
        current: f64,
    },
}

impl FixtureProgramParameterType {
    fn set(&mut self, to: ParameterSetRequest) -> Result<()> {
        match self {
            FixtureProgramParameterType::Discrete {
                levels,
                current_index,
            } => {
                if let ParameterSetRequest::Discrete { level } = to {
                    if let Some(index) = levels.iter().position(|l| &l.name == &level) {
                        *current_index = index;
                        Ok(())
                    } else {
                        bail!("level not found")
                    }
                } else {
                    bail!("continuous value supplied to discrete parameter")
                }
            }
            FixtureProgramParameterType::Continuous {
                lower_limit_incl,
                upper_limit_incl,
                current,
            } => {
                if let ParameterSetRequest::Continuous { value } = to {
                    ensure!(
                        value <= *upper_limit_incl && value >= *lower_limit_incl,
                        "value is out of range"
                    );
                    *current = value;
                    Ok(())
                } else {
                    bail!("discrete value supplied to continuous parameter")
                }
            }
        }
    }

    fn cycle(&mut self) -> Result<String> {
        match self {
            FixtureProgramParameterType::Continuous { .. } => {
                bail!("continuous parameter can not be cycled")
            }
            FixtureProgramParameterType::Discrete {
                levels,
                current_index,
            } => {
                *current_index = (*current_index + 1) % levels.len();
                Ok(levels[*current_index].name.clone())
            }
        }
    }
}

#[derive(Clone, Debug, Serialize)]
struct FixtureProgramParameterDiscreteLevel {
    name: String,
    description: String,
}
