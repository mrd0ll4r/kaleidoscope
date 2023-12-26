use crate::runtime::globals::DeltaTable;
use crate::runtime::parameters::{DiscreteParameterValue, ParameterDelta, ParameterTable};
use crate::runtime::UniverseView;
use crate::Result;
use alloy::config::{InputValue, UniverseConfig};
use alloy::event::{
    AddressedEvent, ButtonEvent, ButtonEventFilter, EventFilter, EventFilterEntry, EventFilterKind,
    EventFilterStrategy, EventKind,
};
use alloy::{Address, OutputValue};
use anyhow::{anyhow, ensure};
use chrono::Timelike;
use itertools::Itertools;
use lazy_static::lazy_static;
use log::{debug, error};
use noise::{NoiseFn, Perlin};
use rlua::{Context, FromLua, Function, Lua, ToLua, Value};
use std::collections::hash_map::IntoIter;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt::{Debug, Formatter};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

/// Event type constants.
/// Must be in sync with builtin.lua and README.md!
const EVENT_TYPE_UPDATE: &str = "update";
const EVENT_TYPE_BUTTON_DOWN: &str = "button_down";
const EVENT_TYPE_BUTTON_UP: &str = "button_up";
const EVENT_TYPE_BUTTON_CLICKED: &str = "button_clicked";
const EVENT_TYPE_BUTTON_LONG_PRESS: &str = "button_long_press";
const EVENT_TYPE_ERROR: &str = "error";

/// Program enable/disable constants.
/// Must be in sync with builtin.lua!
const PROGRAM_ENABLE_SIGNAL: i64 = 1;
const PROGRAM_DISABLE_SIGNAL: i64 = 2;
const PROGRAM_ENABLE_TOGGLE_SIGNAL: i64 = 3;

/// Parameter type constants.
/// Must be in sync with builtin.lua!
const PARAMETER_TYPE_DISCRETE: &str = "discrete";
const PARAMETER_TYPE_CONTINUOUS: &str = "continuous";

/// Runtime version.
const VERSION: u16 = 2;

lazy_static! {
    pub static ref PERLIN: Perlin = Perlin::new(0);
}

const BUILTIN_SOURCE: &'static str = include_str!("builtin.lua");

fn must_have_address(aliases: &HashMap<String, Address>, address: &Address) -> Result<()> {
    let found = aliases.values().find(|v| **v == *address).is_some();
    match found {
        true => Ok(()),
        false => Err(anyhow!("address not in use: {}", address)),
    }
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub enum ProgramEnableDelta {
    Enable,
    Disable,
    Toggle,
}

impl<'lua> FromLua<'lua> for ProgramEnableDelta {
    fn from_lua(lua_value: Value<'lua>, _: Context<'lua>) -> rlua::Result<Self> {
        match lua_value {
            Value::Integer(i) => match i {
                PROGRAM_ENABLE_SIGNAL => Ok(ProgramEnableDelta::Enable),
                PROGRAM_DISABLE_SIGNAL => Ok(ProgramEnableDelta::Disable),
                PROGRAM_ENABLE_TOGGLE_SIGNAL => Ok(ProgramEnableDelta::Toggle),
                _ => Err(rlua::Error::FromLuaConversionError {
                    from: "unknown",
                    to: "ProgramEnableDelta",
                    message: Some("expected integer in [1,3]".to_string()),
                }),
            },
            _ => Err(rlua::Error::FromLuaConversionError {
                from: "unknown",
                to: "ProgramEnableDelta",
                message: Some("expected integer in [1,3]".to_string()),
            }),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProgramEnableDeltaTable {
    v: HashMap<String, ProgramEnableDelta>,
}

impl IntoIterator for ProgramEnableDeltaTable {
    type Item = (String, ProgramEnableDelta);
    type IntoIter = IntoIter<String, ProgramEnableDelta>;

    fn into_iter(self) -> Self::IntoIter {
        self.v.into_iter()
    }
}

impl<'lua> FromLua<'lua> for ProgramEnableDeltaTable {
    fn from_lua(value: Value<'lua>, lua: Context<'lua>) -> rlua::Result<Self> {
        let v: HashMap<String, ProgramEnableDelta> = HashMap::from_lua(value, lua)?;
        Ok(ProgramEnableDeltaTable { v })
    }
}

pub struct Program {
    lua: Lua,
    program_epoch: Instant,
    tick_enabled: bool,
    pub(crate) priority: u8,
    pub(crate) outputs: HashSet<Address>,
    pub(crate) event_filters: HashMap<Address, EventFilter>,
    pub(crate) name: String,
    pub(crate) slow_mode: bool,

    // Contains a view of the universe, but only with addresses that this program uses as inputs.
    // Is updated before each tick through events and then pushed to the lua side.
    universe_view: Arc<Mutex<UniverseView>>,

    // Holds events the program has subscribed to.
    event_buffer: Arc<Mutex<VecDeque<AddressedEvent>>>,
}

impl Debug for Program {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Program")
            .field("name", &self.name)
            .field("priority", &self.priority)
            .field("tick_enabled", &self.tick_enabled)
            .field("outputs", &self.outputs)
            .field("program_epoch", &self.program_epoch)
            .field("event_filters", &self.event_filters)
            .finish()
    }
}

impl Program {
    pub async fn new<P: AsRef<Path>>(
        universe_config: &UniverseConfig,
        parameters: Arc<std::sync::Mutex<ParameterTable>>,
        source_file: P,
    ) -> Result<Program> {
        let lua = Lua::new();
        let name = source_file
            .as_ref()
            .file_stem()
            .ok_or_else(|| anyhow!("no file name?"))?
            .to_str()
            .ok_or_else(|| anyhow!("invalid path?"))?
            .to_string();
        debug!("loading program {}...", name);
        let program_source = fs::read_to_string(source_file)?;
        let program_epoch = Instant::now();

        lua.load_from_std_lib(rlua::StdLib::TABLE)?;

        lua.context(|ctx| -> Result<()> {
            let globals = ctx.globals();

            ctx.load(BUILTIN_SOURCE)
                .exec()
                .expect("unable to load builtin source");

            // Inject a bunch of constants after builtins were loaded, but before the program source
            // is loaded.
            Self::inject_pre_load_constants(
                &ctx,
                program_epoch,
                name.clone(),
                parameters.clone(),
                universe_config,
            )?;

            // Load program source.
            ctx.load(&program_source).exec()?;

            // check source version
            let source_version: u16 = globals.get("SOURCE_VERSION")?;
            ensure!(source_version == VERSION, "source version mismatch");

            Ok(())
        })?;

        let setup_values = Self::setup(
            &lua,
            name.clone(),
            parameters.clone(),
            universe_config,
            program_epoch.elapsed().as_secs_f64(),
        )
        .await?;
        debug!("set up program {}: {:?}", name, setup_values);

        // set up event stuff
        let registered_events: HashMap<Address, Vec<EventFilterEntry>> = setup_values
            .event_targets
            .iter()
            .map(|(k, v)| {
                let mut v = v.iter().map(|(e, _, _)| e.clone()).collect_vec();
                v.sort_unstable();
                v.dedup();
                (*k, v)
            })
            .collect();
        let event_filters = registered_events
            .clone()
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    EventFilter {
                        strategy: EventFilterStrategy::Any,
                        entries: v,
                    },
                )
            })
            .collect();

        Ok(Program {
            lua,
            event_filters,
            program_epoch,
            priority: setup_values.priority,
            outputs: setup_values.outputs,
            slow_mode: setup_values.slow_mode,
            tick_enabled: true,
            name,
            universe_view: Arc::new(Mutex::new(UniverseView::new_with_addresses(
                &setup_values.inputs.into_iter().collect(),
            ))),
            event_buffer: Default::default(),
        })
    }

    pub(crate) fn get_global_deltas(&self) -> Result<DeltaTable> {
        self.lua.context(|ctx| -> Result<DeltaTable> {
            let deltas = ctx.globals().get("_global_deltas")?;
            Ok(deltas)
        })
    }

    pub(crate) fn get_program_enable_deltas(&self) -> Result<ProgramEnableDeltaTable> {
        self.lua.context(|ctx| -> Result<ProgramEnableDeltaTable> {
            let deltas = ctx
                .globals()
                .get::<_, Function>("_get_program_enable_deltas")?
                .call(())?;

            Ok(deltas)
        })
    }

    pub(crate) fn update_globals(&self, delta_table: &DeltaTable) -> Result<()> {
        debug!("{}: updating globals", self.name);

        // Call distributor
        self.lua.context(|ctx| -> Result<()> {
            let globals = ctx.globals();
            let handler: Function = globals.get("_update_globals")?;

            handler.call(delta_table.to_lua(ctx))?;

            Ok(())
        })?;

        Ok(())
    }

    pub(crate) fn handle_incoming_parameter_deltas(
        &self,
        deltas: HashMap<String, ParameterDelta>,
    ) -> Result<()> {
        if deltas.is_empty() {
            return Ok(());
        }

        // Encode deltas as one long string, because performance.
        let deltas = deltas
            .into_iter()
            .map(|(param_name, delta)| match delta {
                ParameterDelta::Continuous(d) => {
                    format!("{} {} {}", param_name, "c", d)
                }
                ParameterDelta::Discrete(d) => {
                    format!("{} {} {}", param_name, "d", d)
                }
            })
            .join(";");

        // Call distributor
        self.lua.context(|ctx| -> Result<()> {
            let globals = ctx.globals();
            let handler: Function = globals.get("_handle_parameter_events")?;

            handler.call(deltas)?;

            Ok(())
        })?;

        Ok(())
    }

    pub(crate) async fn handle_incoming_events(
        &self,
        events: Arc<VecDeque<AddressedEvent>>,
    ) -> Result<bool> {
        debug!("{}: applying events", self.name);

        let mut any_matches = false;
        let mut universe_view = self.universe_view.lock().await;
        let mut event_buffer = self.event_buffer.lock().await;

        for event in events.iter() {
            debug!("{}: applying event: {:?}", self.name, event);
            if universe_view.has_address(event.address) {
                universe_view
                    .apply_event(event)
                    .expect("unable to apply event to universe view")
            }

            let filter_for_address = self.event_filters.get(&event.address);
            if let Some(filter) = filter_for_address {
                if filter.matches(&event.event) {
                    debug!("{}: event {:?} matched, adding to buffer", self.name, event);
                    event_buffer.push_back(event.clone());
                    any_matches = true;
                }
            }
        }

        if !event_buffer.is_empty() {
            self.inject_events(&event_buffer).await?;
            event_buffer.clear();
        }

        Ok(any_matches)
    }

    async fn inject_inputs_unfiltered(
        lua: &Lua,
        universe_view: Arc<Mutex<UniverseView>>,
        now: f64,
        time_of_day: u32,
    ) -> Result<()> {
        let values = universe_view.lock().await.clone();
        lua.context(|ctx| -> Result<()> {
            let values = values.to_lua(ctx)?;
            ctx.globals().set("input_values_by_address", values)?;
            ctx.globals().set("NOW", now)?;
            ctx.globals().set("TIME_OF_DAY", time_of_day)?;
            Ok(())
        })?;

        Ok(())
    }

    /// Calculates the time of day.
    /// TODO: when running many programs this might be expensive.
    /// We could cache it and update once per second or so.
    fn calculate_time_of_day() -> u32 {
        let now = chrono::Local::now().time();

        let res = now.hour() * 60 * 60 + now.minute() * 60 + now.second();
        debug!("calculated time of day as {}", res);
        res
    }

    pub async fn inject_inputs(&self) -> Result<()> {
        Self::inject_inputs_unfiltered(
            &self.lua,
            self.universe_view.clone(),
            self.program_epoch.elapsed().as_secs_f64(),
            Self::calculate_time_of_day(),
        )
        .await
    }

    async fn inject_events(&self, events: &VecDeque<AddressedEvent>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        // Encode events as one long string, because performance.
        let events = events
            .into_iter()
            .map(|e| {
                format!(
                    "{} {}",
                    e.address,
                    match &e.event.inner {
                        Ok(inner) => {
                            match inner {
                                EventKind::Update { new_value } => {
                                    format!(
                                        "{} {}",
                                        EVENT_TYPE_UPDATE,
                                        match new_value {
                                            InputValue::Binary(b) => {
                                                format!("{}", b)
                                            }
                                            InputValue::Temperature(t) => {
                                                format!("{}", t)
                                            }
                                            InputValue::Humidity(h) => {
                                                format!("{}", h)
                                            }
                                            InputValue::Pressure(p) => {
                                                format!("{}", p)
                                            }
                                            InputValue::Continuous(c) => {
                                                format!("{}", c)
                                            }
                                        }
                                    )
                                }
                                EventKind::Button(inner) => match inner {
                                    ButtonEvent::Up => format!("{}", EVENT_TYPE_BUTTON_UP),
                                    ButtonEvent::Down => format!("{}", EVENT_TYPE_BUTTON_DOWN),
                                    ButtonEvent::Clicked { duration } => format!(
                                        "{} {}",
                                        EVENT_TYPE_BUTTON_CLICKED,
                                        duration.as_secs_f64()
                                    ),
                                    ButtonEvent::LongPress { seconds } => {
                                        format!("{} {}", EVENT_TYPE_BUTTON_LONG_PRESS, seconds)
                                    }
                                },
                            }
                        }
                        Err(e) => {
                            format!("{} {}", EVENT_TYPE_ERROR, e)
                        }
                    }
                )
            })
            .join(";");

        // Call distributor
        self.lua.context(|ctx| -> Result<()> {
            let globals = ctx.globals();
            let handler: Function = globals.get("_handle_events")?;

            handler.call(events)?;

            Ok(())
        })?;

        Ok(())
    }

    fn raw_tick(&self, now: Instant) -> Result<HashMap<Address, OutputValue>> {
        if !self.tick_enabled {
            return Ok(HashMap::new());
        }

        let output_values_by_address =
            self.lua
                .context(|ctx| -> Result<HashMap<Address, OutputValue>> {
                    let globals = ctx.globals();

                    let now = now.duration_since(self.program_epoch).as_secs_f64();
                    let tick: Function = globals.get("_tick")?;

                    let output_values_by_address: HashMap<Address, OutputValue> = tick.call(now)?;

                    Ok(output_values_by_address)
                })?;

        Ok(output_values_by_address)
    }

    pub fn tick(&self, now: Instant) -> Result<HashMap<Address, OutputValue>> {
        self.raw_tick(now)
    }

    fn inject_pre_load_constants(
        ctx: &rlua::Context,
        program_epoch: Instant,
        program_name: String,
        parameters: Arc<std::sync::Mutex<ParameterTable>>,
        universe: &UniverseConfig,
    ) -> Result<()> {
        let globals = ctx.globals();

        // Inject constants.
        globals.set("START", program_epoch.elapsed().as_secs_f64())?;
        globals.set("NOW", program_epoch.elapsed().as_secs_f64())?;
        globals.set("PROGRAM_NAME", program_name.clone())?;
        globals.set("KALEIDOSCOPE_VERSION", VERSION)?;

        // Inject translations for aliases and groups.
        let input_aliases = ctx.create_table()?;
        let output_aliases = ctx.create_table()?;
        for cfg in &universe.devices {
            for input in cfg.inputs.iter() {
                input_aliases.set(input.alias.clone(), input.address)?;
            }
            for output in cfg.outputs.iter() {
                input_aliases.set(output.alias.clone(), output.address)?;
                output_aliases.set(output.alias.clone(), output.address)?;
            }
        }
        globals.set("input_alias_address", input_aliases)?;
        globals.set("output_alias_address", output_aliases)?;

        // TODO tags
        /*
        let group_addresses = ctx.create_table()?;
        let groups: Vec<String> = virtual_devices
            .iter()
            .flat_map(|c| c.groups.clone())
            .dedup()
            .collect();
        for group in groups {
            let g = group.clone();
            group_addresses.set(
                group,
                virtual_devices
                    .iter()
                    .filter(|c| c.groups.contains(&g))
                    .map(|c| c.address)
                    .collect_vec(),
            )?;
        }
        globals.set("group_addresses", group_addresses)?;
         */

        // Inject Perlin noise functions.
        globals.set(
            "noise2d",
            ctx.create_function(|_, (x, y): (f64, f64)| Ok(PERLIN.get([x, y])))?,
        )?;
        globals.set(
            "noise3d",
            ctx.create_function(|_, (x, y, z): (f64, f64, f64)| Ok(PERLIN.get([x, y, z])))?,
        )?;
        globals.set(
            "noise4d",
            ctx.create_function(|_, (x, y, z, t): (f64, f64, f64, f64)| {
                Ok(PERLIN.get([x, y, z, t]))
            })?,
        )?;

        // Parameter functions
        {
            let parameters = parameters.clone();
            let program_name = program_name.clone();
            globals.set(
                "get_foreign_discrete_parameter_value",
                ctx.create_function(
                    move |_, (p_program_name, parameter_name): (String, String)| {
                        let parameters = parameters.lock().unwrap();
                        parameters
                            .get_discrete_parameter_value(&p_program_name, &parameter_name)
                            .map_err(|e| {
                                error!(
                                    "program {} attempted to access invalid parameter: {:?}",
                                    program_name, e
                                );
                                rlua::Error::external(e)
                            })
                    },
                )?,
            )?;
        }
        {
            let parameters = parameters.clone();
            let program_name = program_name.clone();
            globals.set(
                "get_foreign_continuous_parameter_value",
                ctx.create_function(
                    move |_, (p_program_name, parameter_name): (String, String)| {
                        let parameters = parameters.lock().unwrap();
                        parameters
                            .get_continuous_parameter_value(&p_program_name, &parameter_name)
                            .map_err(|e| {
                                error!(
                                    "program {} attempted to access invalid parameter: {:?}",
                                    program_name, e
                                );
                                rlua::Error::external(e)
                            })
                    },
                )?,
            )?;
        }
        {
            let parameters = parameters.clone();
            let program_name = program_name.clone();
            globals.set(
                "set_foreign_discrete_parameter_value",
                ctx.create_function(
                    move |_,
                          (p_program_name, parameter_name, parameter_value): (
                        String,
                        String,
                        i32,
                    )| {
                        let mut parameters = parameters.lock().unwrap();
                        parameters
                            .set_discrete_parameter(
                                &p_program_name,
                                &parameter_name,
                                parameter_value,
                            )
                            .map_err(|e| {
                                error!(
                                    "program {} attempted to set invalid parameter: {:?}",
                                    program_name, e
                                );
                                rlua::Error::external(e)
                            })
                    },
                )?,
            )?;
        }
        {
            let parameters = parameters.clone();
            let program_name = program_name.clone();
            globals.set(
                "set_foreign_continuous_parameter_value",
                ctx.create_function(
                    move |_,
                          (p_program_name, parameter_name, parameter_value): (
                        String,
                        String,
                        f64,
                    )| {
                        let mut parameters = parameters.lock().unwrap();
                        parameters
                            .set_continuous_parameter(
                                &p_program_name,
                                &parameter_name,
                                parameter_value,
                            )
                            .map_err(|e| {
                                error!(
                                    "program {} attempted to set invalid parameter: {:?}",
                                    program_name, e
                                );
                                rlua::Error::external(e)
                            })
                    },
                )?,
            )?;
        }
        {
            let parameters = parameters.clone();
            let program_name = program_name.clone();
            globals.set(
                "increment_foreign_discrete_parameter_value",
                ctx.create_function(
                    move |_, (p_program_name, parameter_name, delta): (String, String, i32)| {
                        let mut parameters = parameters.lock().unwrap();
                        parameters
                            .increment_discrete_parameter(&p_program_name, &parameter_name, delta)
                            .map_err(|e| {
                                error!(
                                    "program {} attempted to set invalid parameter: {:?}",
                                    program_name, e
                                );
                                rlua::Error::external(e)
                            })
                    },
                )?,
            )?;
        }

        Ok(())
    }

    async fn setup(
        lua: &Lua,
        program_name: String,
        parameters: Arc<std::sync::Mutex<ParameterTable>>,
        universe: &UniverseConfig,
        now: f64,
    ) -> Result<SetupValues> {
        let mut priority: u8 = 0; // TODO use option
        let mut slow_mode = false;
        let mut inputs: HashSet<Address> = HashSet::new();
        let mut outputs: HashSet<Address> = HashSet::new();
        let mut event_targets: HashMap<Address, Vec<(EventFilterEntry, String, String)>> =
            HashMap::new();
        let mut parameter_handlers: HashMap<String, String> = HashMap::new();
        let output_aliases: HashMap<_, _> = universe
            .devices
            .iter()
            .flat_map(|d| &d.outputs)
            .map(|output| (output.alias.clone(), output.address))
            .collect();
        let input_aliases: HashMap<_, _> = universe
            .devices
            .iter()
            .flat_map(|dev| {
                dev.inputs
                    .iter()
                    .map(|input| (input.alias.clone(), input.address))
            })
            .chain(universe.devices.iter().flat_map(|dev| {
                dev.outputs
                    .iter()
                    .map(|output| (output.alias.clone(), output.address))
            }))
            .collect();

        // Inject inputs
        Self::inject_inputs_unfiltered(
            lua,
            Arc::new(Mutex::new(UniverseView::new_from_universe_config(universe))),
            now,
            Self::calculate_time_of_day(),
        )
        .await?;

        // setup
        lua.context(|ctx| -> Result<()> {
            let globals = ctx.globals();
            let setup: Function = globals.get("setup")?;

            ctx.scope(|scope| -> Result<()> {
                // Provide setup-related functions:
                // These are only valid inside of the `scope`, in which we will also call `setup()`.
                // This is good, because these are setup-related and we don't want users to call them
                // from `tick()`.

                let declare_parameter_generic = scope
                    .create_function_mut(|_,
                                          (type_name, param_name, description, event_handler, discrete_values, discrete_initial, continuous_lower, continuous_upper, continuous_initial):
                                          (String, String, String, String, Vec<DiscreteParameterValue>, i32, f64, f64, f64)| {

                        // Check if handler function exists.
                        ctx.globals().get::<_, Function>(event_handler.clone())?;

                        // Try to register a new parameter.
                        let mut parameters = parameters.lock().unwrap();
                        match type_name.as_str() {
                            PARAMETER_TYPE_DISCRETE => {
                                debug!("attempting to create discrete parameter {} for program {} with values {:?} and initial {}",param_name,program_name,discrete_values,discrete_initial);
                                if let Err(e) = parameters.declare_discrete_parameter(program_name.clone(), param_name.clone(),
                                                                                      description,discrete_values, discrete_initial) {
                                    error!("unable to create discrete parameter {} for program {}: {:?}",param_name,program_name,e);
                                    return Err(rlua::Error::external(anyhow!(
                                        "unable to create parameter: {:?}",e
                                    )));
                                }
                            }
                            PARAMETER_TYPE_CONTINUOUS => {
                                debug!("attempting to create continuous parameter {} for program {} with limits [{}, {}] and initial {}",
                                    param_name,program_name,continuous_lower,continuous_upper,continuous_initial);
                                if let Err(e) = parameters.declare_continuous_parameter(program_name.clone(), param_name.clone(),
                                                                                        description,continuous_lower, continuous_upper, continuous_initial) {
                                    error!("unable to create continuous parameter {} for program {}: {:?}",param_name,program_name,e);
                                    return Err(rlua::Error::external(anyhow!(
                                        "unable to create parameter: {:?}",e
                                    )));
                                }
                            }
                            _ => {
                                return Err(rlua::Error::external(anyhow!(
                                    "invalid parameter type: {}",
                                    type_name
                                )));
                            }
                        };

                        parameter_handlers.insert(param_name, event_handler);

                        Ok(())
                    })?;
                globals.set("_declare_parameter_generic", declare_parameter_generic)?;

                let set_priority = scope.create_function_mut(|_, prio| {
                    if prio > 20 {
                        return Err(rlua::Error::external("priority must be <= 20"));
                    }
                    priority = prio;
                    Ok(())
                })?;
                globals.set("set_priority", set_priority)?;

                let set_slow_mode = scope.create_function_mut(|_, p_slow_mode| {
                    slow_mode = p_slow_mode;
                    Ok(())
                })?;
                globals.set("set_slow_mode", set_slow_mode)?;

                let add_input_address = scope.create_function_mut(|_, address: Address| {
                    must_have_address(&input_aliases, &address).map_err(rlua::Error::external)?;

                    inputs.insert(address);

                    Ok(())
                })?;
                globals.set("add_input_address", add_input_address)?;

                let add_input_alias = scope.create_function(|ctx, alias: String| {
                    let addr = *input_aliases.get(&alias).ok_or_else(|| {
                        rlua::Error::external(format!("unknown alias: {}", alias))
                    })?;

                    let add_input_address: Function = ctx.globals().get("add_input_address")?;
                    add_input_address.call(addr)?;

                    Ok(())
                })?;
                globals.set("add_input_alias", add_input_alias)?;

                let add_output_address = scope.create_function_mut(|_, address: Address| {
                    must_have_address(&output_aliases, &address).map_err(rlua::Error::external)?;

                    outputs.insert(address);

                    Ok(())
                })?;
                globals.set("add_output_address", add_output_address)?;

                let add_output_alias = scope.create_function(|ctx, alias: String| {
                    let addr = *output_aliases.get(&alias).ok_or_else(|| {
                        rlua::Error::external(format!("unknown alias: {}", alias))
                    })?;

                    let add_output_address: Function = ctx.globals().get("add_output_address")?;
                    add_output_address.call(addr)?;

                    Ok(())
                })?;
                globals.set("add_output_alias", add_output_alias)?;

                /*
                // TODO tags
                let add_output_group = scope.create_function(|ctx, group: String| {
                    let addresses: Vec<Address> = virtual_devices
                        .iter()
                        .filter(|c| c.groups.contains(&group))
                        .map(|c| c.address)
                        .collect();

                    if addresses.is_empty() {
                        return Err(rlua::Error::external(format!("unknown group: {}", group)));
                    }

                    let add_output_address: Function = ctx.globals().get("add_output_address")?;
                    for address in addresses {
                        add_output_address.call(address)?;
                    }

                    Ok(())
                })?;
                globals.set("add_output_group", add_output_group)?;
                 */

                let add_event_subscription = scope.create_function_mut(
                    |ctx, (alias, type_name, target): (String, String, String)| {
                        // Check if alias exists.
                        let address = *input_aliases.get(&alias).ok_or_else(|| {
                            rlua::Error::external(format!("unknown alias: {}", alias))
                        })?;

                        // Check if target function exists.
                        ctx.globals().get::<_, Function>(target.clone())?;

                        // "Parse" filter.
                        let filter_entry = EventFilterEntry::Kind {
                            kind: match type_name.as_str() {
                                EVENT_TYPE_UPDATE => EventFilterKind::Update,
                                EVENT_TYPE_BUTTON_UP => EventFilterKind::Button {
                                    filter: ButtonEventFilter::Up,
                                },
                                EVENT_TYPE_BUTTON_DOWN => EventFilterKind::Button {
                                    filter: ButtonEventFilter::Down,
                                },
                                EVENT_TYPE_BUTTON_CLICKED => EventFilterKind::Button {
                                    filter: ButtonEventFilter::Clicked,
                                },
                                EVENT_TYPE_BUTTON_LONG_PRESS => EventFilterKind::Button {
                                    filter: ButtonEventFilter::LongPress,
                                },
                                _ => {
                                    return Err(rlua::Error::external(anyhow!(
                                        "invalid event type: {}",
                                        type_name
                                    )));
                                }
                            },
                        };

                        event_targets.entry(address).or_default().push((
                            filter_entry,
                            type_name,
                            target.clone(),
                        ));

                        Ok(())
                    },
                )?;
                globals.set("add_event_subscription", add_event_subscription)?;

                // Actually call setup
                setup.call(())?;

                Ok(())
            })?;

            // Clear values again (so that after this only registered-for inputs are available)
            globals.set("input_values_by_address", rlua::Nil)?;

            // Set up parameter handlers
            globals.set("_parameter_event_handlers",parameter_handlers)?;

            // Set up event routing
            globals.set(
                "_event_handlers",
                event_targets
                    .iter()
                    .map(|(addr, filters)| {
                        (
                            *addr,
                            filters
                                .clone()
                                .into_iter()
                                .map(|(_, type_name, target)| {
                                    let mut m = HashMap::new();
                                    m.insert("handler".to_string(), target.to_lua(ctx).unwrap());
                                    m.insert("type".to_string(), type_name.to_lua(ctx).unwrap());
                                    m
                                })
                                .collect_vec(),
                        )
                    })
                    .collect::<HashMap<Address, Vec<HashMap<String, rlua::prelude::LuaValue>>>>(),
            )?;

            Ok(())
        })?;

        Ok(SetupValues {
            priority,
            inputs,
            outputs,
            event_targets,
            slow_mode,
        })
    }
}

#[derive(Clone, Debug)]
struct SetupValues {
    priority: u8,
    inputs: HashSet<Address>,
    outputs: HashSet<Address>,
    event_targets: HashMap<Address, Vec<(EventFilterEntry, String, String)>>,
    slow_mode: bool,
}
