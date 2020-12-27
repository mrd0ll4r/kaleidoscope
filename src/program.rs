use crate::Result;
use alloy::config::VirtualDeviceConfig;
use alloy::event::{
    AddressedEvent, ButtonEvent, ButtonEventFilter, EventFilter, EventFilterEntry, EventFilterKind,
    EventFilterStrategy, EventKind,
};
use alloy::{Address, Value};
use failure::err_msg;
use itertools::Itertools;
use noise::{NoiseFn, Perlin};
use rlua::{Function, Lua, ToLua};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Instant;
use std::{fs, mem};
use tokio::sync::{broadcast, Mutex};
use tokio::task;

/// Event type constants.
/// Must be in sync with builtin.lua and README.md!
const EVENT_TYPE_CHANGE: &str = "change";
const EVENT_TYPE_BUTTON_DOWN: &str = "button_down";
const EVENT_TYPE_BUTTON_UP: &str = "button_up";
const EVENT_TYPE_BUTTON_CLICKED: &str = "button_clicked";
const EVENT_TYPE_BUTTON_LONG_PRESS: &str = "button_long_press";

/// Runtime version.
const VERSION: u16 = 1;

lazy_static! {
    pub static ref PERLIN: Perlin = Perlin::new();
}

const BUILTIN_SOURCE: &'static str = include_str!("builtin.lua");

fn address_in_use(virtual_devices: &Vec<VirtualDeviceConfig>, address: &Address) -> bool {
    virtual_devices
        .iter()
        .find(|c| c.address == *address)
        .is_some()
}

fn must_have_address(virtual_devices: &Vec<VirtualDeviceConfig>, address: &Address) -> Result<()> {
    match address_in_use(virtual_devices, address) {
        true => Ok(()),
        false => Err(err_msg(format!("address not in use: {}", address))),
    }
}

pub struct Program {
    lua: Lua,
    event_buffer: Arc<Mutex<Vec<AddressedEvent>>>,
    program_epoch: Instant,
    inputs: HashSet<Address>,
    tick_enabled: bool,
    pub priority: u8,
    pub outputs: HashSet<Address>,
    pub registered_events: HashMap<Address, Vec<EventFilterEntry>>,
    pub name: String,
}

impl Program {
    pub fn new<P: AsRef<Path>>(
        virtual_devices: &Vec<VirtualDeviceConfig>,
        current_values: HashMap<Address, Value>,
        source_file: P,
        event_channel: broadcast::Receiver<AddressedEvent>,
    ) -> Result<Program> {
        let lua = Lua::new();
        let name = source_file
            .as_ref()
            .file_stem()
            .ok_or_else(|| err_msg("no file name?"))?
            .to_str()
            .ok_or_else(|| err_msg("invalid path?"))?
            .to_string();
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
            Self::inject_pre_load_constants(&ctx, program_epoch, virtual_devices)?;

            // Load program source.
            ctx.load(&program_source).exec()?;

            // check source version
            let source_version: u16 = globals.get("SOURCE_VERSION")?;
            ensure!(source_version == VERSION, "source version mismatch");

            Ok(())
        })?;

        let setup_values = Self::setup(
            &lua,
            virtual_devices,
            current_values,
            program_epoch.elapsed().as_secs_f64(),
        )?;

        // use whatever setup set up
        println!("priority: {}", setup_values.priority);
        println!("inputs: {:?}", setup_values.inputs);
        println!("outputs: {:?}", setup_values.outputs);
        println!("event_targets: {:?}", setup_values.event_targets);

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
        let event_buffer = Arc::new(Mutex::new(Vec::new()));
        let task_event_buffer = event_buffer.clone();
        task::spawn(Self::handle_incoming_events(
            event_channel,
            event_filters,
            task_event_buffer,
        ));

        Ok(Program {
            lua,
            event_buffer,
            program_epoch,
            priority: setup_values.priority,
            outputs: setup_values.outputs,
            inputs: setup_values.inputs,
            tick_enabled: true,
            registered_events,
            name,
        })
    }

    async fn handle_incoming_events(
        mut bc_receiver: broadcast::Receiver<AddressedEvent>,
        registered_events: HashMap<Address, EventFilter>,
        event_buffer: Arc<Mutex<Vec<AddressedEvent>>>,
    ) {
        loop {
            let event = bc_receiver.recv().await;
            match event {
                Ok(event) => {
                    let filter_for_address = registered_events.get(&event.address);
                    if let Some(filter) = filter_for_address {
                        if filter.matches(&event.event) {
                            let mut event_buffer = event_buffer.lock().await;
                            event_buffer.push(event);
                        }
                    }
                }
                Err(e) => {
                    match e {
                        broadcast::error::RecvError::Closed => break,
                        broadcast::error::RecvError::Lagged(_) => {
                            // TOOD
                            continue;
                        }
                    }
                }
            }
        }
    }

    fn inject_inputs_unfiltered(
        lua: &rlua::Lua,
        values: HashMap<Address, Value>,
        now: f64,
    ) -> Result<()> {
        lua.context(|ctx| -> Result<()> {
            ctx.globals().set("input_values_by_address", values)?;
            ctx.globals().set("NOW", now)?;
            Ok(())
        })?;

        Ok(())
    }

    pub fn inject_inputs(&self, values: Arc<HashMap<Address, Value>>) -> Result<()> {
        // TODO use filter_map?
        let subscribed_inputs: HashMap<Address, Value> = values
            .iter()
            .filter(|(k, _)| self.inputs.contains(*k))
            .map(|(k, v)| (*k, *v))
            .collect();

        Self::inject_inputs_unfiltered(
            &self.lua,
            subscribed_inputs,
            self.program_epoch.elapsed().as_secs_f64(),
        )
    }

    pub async fn process_events(&self) -> Result<()> {
        // Get events
        let events = {
            let buf = &mut *self.event_buffer.lock().await;
            mem::take(buf)
        };

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
                    match e.event.inner {
                        EventKind::Change { new_value } => {
                            format!("{} {}", EVENT_TYPE_CHANGE, new_value)
                        }
                        EventKind::Button(inner) => {
                            match inner {
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
                            }
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

    fn raw_tick(&self, now: Instant) -> Result<HashMap<Address, Value>> {
        if !self.tick_enabled {
            return Ok(HashMap::new());
        }

        let output_values_by_address =
            self.lua.context(|ctx| -> Result<HashMap<Address, Value>> {
                let globals = ctx.globals();

                let now = now.duration_since(self.program_epoch).as_secs_f64();
                let tick: Function = globals.get("_tick")?;

                let output_values_by_address: HashMap<Address, Value> = tick.call(now)?;

                Ok(output_values_by_address)
            })?;

        Ok(output_values_by_address)
    }

    pub fn tick(&self, now: Instant) -> Result<HashMap<Address, Value>> {
        let outputs = self.raw_tick(now)?;

        Ok(outputs)
    }

    fn inject_pre_load_constants(
        ctx: &rlua::Context,
        program_epoch: Instant,
        virtual_devices: &Vec<VirtualDeviceConfig>,
    ) -> Result<()> {
        let globals = ctx.globals();

        // Inject constants.
        globals.set("START", program_epoch.elapsed().as_secs_f64())?;
        globals.set("NOW", program_epoch.elapsed().as_secs_f64())?;
        globals.set("KALEIDOSCOPE_VERSION", VERSION)?;

        // Inject translations for aliases and groups.
        let alias_address = ctx.create_table()?;
        for vdev in virtual_devices {
            alias_address.set(vdev.alias.clone(), vdev.address)?;
        }
        globals.set("alias_address", alias_address)?;

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

        Ok(())
    }

    fn setup(
        lua: &Lua,
        virtual_devices: &Vec<VirtualDeviceConfig>,
        current_values: HashMap<Address, Value>,
        now: f64,
    ) -> Result<SetupValues> {
        let mut priority: u8 = 0; // TODO use option
        let mut inputs: HashSet<Address> = HashSet::new();
        let mut outputs: HashSet<Address> = HashSet::new();
        let mut event_targets: HashMap<Address, Vec<(EventFilterEntry, String, String)>> =
            HashMap::new();

        // Inject inputs
        Self::inject_inputs_unfiltered(lua, current_values, now)?;

        // setup
        lua.context(|ctx| -> Result<()> {
            let globals = ctx.globals();
            let setup: Function = globals.get("setup")?;

            ctx.scope(|scope| -> Result<()> {
                // Provide setup-related functions:
                // These are only valid inside of the `scope`, in which we will also call `setup()`.
                // This is good, because these are setup-related and we don't want users to call them
                // from `tick()`.

                let set_priority = scope.create_function_mut(|_, prio| {
                    if prio > 20 {
                        return Err(rlua::Error::external("priority must be <= 20"));
                    }
                    priority = prio;
                    Ok(())
                })?;
                globals.set("set_priority", set_priority)?;

                let add_input_address = scope.create_function_mut(|_, address: Address| {
                    must_have_address(&virtual_devices, &address).map_err(rlua::Error::external)?;

                    if inputs.contains(&address) {
                        return Err(rlua::Error::external(format!(
                            "duplicate input address: {}",
                            address
                        )));
                    }
                    inputs.insert(address);

                    Ok(())
                })?;
                globals.set("add_input_address", add_input_address)?;

                let add_input_alias = scope.create_function(|ctx, alias: String| {
                    let vdev = virtual_devices
                        .iter()
                        .find(|c| *c.alias == alias)
                        .ok_or_else(|| {
                            rlua::Error::external(format!("unknown alias: {}", alias))
                        })?;

                    // Wow... we call from Rust into Lua to call into Rust again, to make the borrow
                    // checker happy.
                    let add_input_address: Function = ctx.globals().get("add_input_address")?;
                    add_input_address.call(vdev.address)?;

                    Ok(())
                })?;
                globals.set("add_input_alias", add_input_alias)?;

                let add_output_address = scope.create_function_mut(|_, address: Address| {
                    must_have_address(&virtual_devices, &address).map_err(rlua::Error::external)?;

                    // We don't check for duplicate output addresses, because someone could use both a group
                    // and individual devices from that group.
                    outputs.insert(address);

                    Ok(())
                })?;
                globals.set("add_output_address", add_output_address)?;

                let add_output_alias = scope.create_function(|ctx, alias: String| {
                    let vdev = virtual_devices
                        .iter()
                        .find(|c| *c.alias == alias)
                        .ok_or_else(|| {
                            rlua::Error::external(format!("unknown alias: {}", alias))
                        })?;

                    let add_output_address: Function = ctx.globals().get("add_output_address")?;
                    add_output_address.call(vdev.address)?;

                    Ok(())
                })?;
                globals.set("add_output_alias", add_output_alias)?;

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

                let add_event_subscription = scope.create_function_mut(
                    |ctx, (alias, type_name, target): (String, String, String)| {
                        // Check if alias exists.
                        let vdev = virtual_devices
                            .iter()
                            .find(|c| *c.alias == alias)
                            .ok_or_else(|| {
                                rlua::Error::external(format!("unknown alias: {}", alias))
                            })?;

                        // Check if target function exists.
                        ctx.globals().get::<_, Function>(target.clone())?;

                        // "Parse" filter.
                        let filter_entry = EventFilterEntry::Kind {
                            kind: match type_name.as_str() {
                                EVENT_TYPE_CHANGE => EventFilterKind::Change,
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
                                    return Err(rlua::Error::external(err_msg(format!(
                                        "invalid event type: {}",
                                        type_name
                                    ))));
                                }
                            },
                        };

                        event_targets.entry(vdev.address).or_default().push((
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
        })
    }
}

struct SetupValues {
    priority: u8,
    inputs: HashSet<Address>,
    outputs: HashSet<Address>,
    event_targets: HashMap<Address, Vec<(EventFilterEntry, String, String)>>,
}
