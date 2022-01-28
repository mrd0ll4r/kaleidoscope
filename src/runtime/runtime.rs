use crate::runtime::program::Program;
use crate::runtime::UniverseView;
use crate::Result;
use alloy::api::{SetRequest, SubscriptionRequest};
use alloy::config::UniverseConfig;
use alloy::event::{
    AddressedEvent, EventFilter, EventFilterEntry, EventFilterKind, EventFilterStrategy,
};
use alloy::tcp::PushedMessage;
use alloy::{Address, OutputValue};
use failure::{err_msg, ResultExt};
use itertools::Itertools;
use std::cell::Cell;
use std::collections::{HashMap, VecDeque};
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, Mutex};
use tokio::task;

pub(crate) struct Runtime {
    _universe_view: Arc<Mutex<UniverseView>>,
    set_tx: mpsc::Sender<Vec<SetRequest>>,
    events_processed: Arc<Mutex<u64>>,

    // ordered by priority descending
    loaded_programs: Vec<Rc<WrappedProgram>>,
    tick_values: Vec<TickValue>,

    // stores events received asynchronously, to be handled with the next tick
    event_buffer: Arc<Mutex<VecDeque<AddressedEvent>>>,
}

impl Runtime {
    fn pre_tick(&mut self) {
        self.loaded_programs
            .iter()
            .for_each(|p| p.executed_this_tick.set(false));
        self.tick_values.iter_mut().for_each(|v| v.reset())
    }

    fn post_tick_collect_values(&self) -> Vec<SetRequest> {
        self.tick_values
            .iter()
            .map(|tv| (tv.address, tv.get_highest_priority_value()))
            .filter(|(_, v)| v.is_some())
            .map(|(a, v)| SetRequest {
                value: v.unwrap(),
                address: a,
            })
            .collect()
    }

    fn apply_tick_generated_outputs(
        &mut self,
        values: HashMap<Address, OutputValue>,
        priority: u8,
    ) {
        values.into_iter().for_each(|(a, v)| {
            let value_index = self
                .tick_values
                .binary_search_by_key(&a, |v| v.address)
                .expect(format!("missing tick_value for address {}", a).as_str());
            self.tick_values[value_index].set_value_for_priority(priority, v);
        })
    }
}

#[derive(Debug)]
struct WrappedProgram {
    program: Program,
    executed_this_tick: Cell<bool>,
    enabled: Cell<bool>,
}

#[derive(Debug)]
struct TickValue {
    address: Address,
    produced_values: [Option<OutputValue>; 20],
    // sorted by priority descending
    programs_for_this_value: Vec<Rc<WrappedProgram>>,
    highest_priority_set_value: Option<u8>,
}

impl TickValue {
    fn has_higher_priority_program_available(&self) -> bool {
        let prog = self.get_highest_priority_non_executed_program();
        match prog {
            None => false,
            Some(prog) => match self.highest_priority_set_value {
                None => true,
                Some(prio) => prio < prog.program.priority,
            },
        }
    }

    fn get_highest_priority_non_executed_program(&self) -> Option<Rc<WrappedProgram>> {
        self.programs_for_this_value
            .iter()
            .find(|p| !p.executed_this_tick.get())
            .map(|p| p.clone())
    }

    fn get_highest_priority_value(&self) -> Option<OutputValue> {
        self.highest_priority_set_value
            .map(|prio| self.produced_values[(prio - 1) as usize])
            .flatten()
    }

    fn set_value_for_priority(&mut self, priority: u8, value: OutputValue) {
        assert!(priority <= 20);
        assert!(priority > 0);
        debug!(
            "setting value {} with priority {} for address {}, current values: {:?}",
            value, priority, self.address, self.produced_values
        );
        let index = (priority - 1) as usize;
        self.produced_values[index] = self.produced_values[index].or_else(|| Some(value));
        self.highest_priority_set_value = self
            .highest_priority_set_value
            .or_else(|| Some(0))
            .map(|prio| prio.max(priority))
    }

    fn reset(&mut self) {
        self.produced_values.iter_mut().for_each(|v| *v = None);
        self.highest_priority_set_value = None
    }
}

impl Runtime {
    pub(crate) async fn new(
        client: alloy::tcp::AsyncClient,
        mut push_receiver: tokio::sync::mpsc::Receiver<PushedMessage>,
    ) -> Result<Runtime> {
        // The server should send their config and initial values, so we first wait for those.
        let config = {
            let msg = push_receiver
                .recv()
                .await
                .ok_or(err_msg("did not receive universe config from server"))?;
            match msg {
                PushedMessage::Event(_) => {
                    bail!("did not receive universe config as first message from server")
                }
                PushedMessage::Config(cfg) => cfg,
            }
        };

        let addresses: Vec<_> = config
            .devices
            .iter()
            .flat_map(|dev| {
                dev.inputs
                    .iter()
                    .map(|d| d.address)
                    .chain(dev.outputs.iter().map(|d| d.address))
            })
            .collect();
        let universe_view = Arc::new(Mutex::new(UniverseView::new_with_addresses(&addresses)));
        let events_counter = Arc::new(Mutex::new(0 as u64));
        let task_events_counter = events_counter.clone();

        // Load and setup programs
        let programs = Self::load_programs("programs/", &config).await?;

        // TODO check if two programs with the same priority output to the same addresses?

        // Aggregate and dedup event subscriptions from all programs, add an Update filter for every address
        let event_subscriptions = Self::aggregate_event_filters(&programs, &addresses);
        let event_buffer = Arc::new(Mutex::new(VecDeque::new()));

        // Handle incoming events, keep our values up to date, buffer events etc.
        let task_universe_view = universe_view.clone();
        task::spawn(Self::handle_incoming_events_loop(
            push_receiver,
            task_universe_view,
            task_events_counter,
            event_buffer.clone(),
        ));

        // Subscribe to any events needed by the programs, with an added type=update entry if missing, for every address.
        // We want the change events to keep our view of values up to date.
        Self::subscribe_with_change(&client, &event_subscriptions).await?;

        let (set_tx, mut set_rx) = mpsc::channel(1);

        task::spawn(async move {
            debug!("starting runtime set sender loop");
            while let Some(reqs) = set_rx.recv().await {
                debug!("set sender: got requests {:?}", reqs);

                let before = Instant::now();
                let res = client.set(reqs).await;
                debug!("set took {}µs", before.elapsed().as_micros());

                if let Err(e) = res {
                    error!("unable to set: {:?}", e);
                    return;
                }
            }
            debug!("quitting runtime set sender loop");
        });

        let programs: Vec<_> = programs
            .into_iter()
            .map(|p| {
                Rc::new(WrappedProgram {
                    program: p,
                    executed_this_tick: Cell::new(false),
                    enabled: Cell::new(false),
                })
            })
            .collect();

        let tick_values = addresses
            .into_iter()
            .map(|addr| TickValue {
                address: addr,
                produced_values: Default::default(),
                programs_for_this_value: programs
                    .iter()
                    .filter(|p| p.program.outputs.contains(&addr))
                    .cloned()
                    .sorted_by_key(|p| p.program.priority)
                    .rev()
                    .collect(),
                highest_priority_set_value: None,
            })
            .sorted_by_key(|tv| tv.address)
            .collect();

        Ok(Runtime {
            loaded_programs: programs,
            _universe_view: universe_view,
            set_tx,
            events_processed: events_counter,
            tick_values,
            event_buffer,
        })
    }

    pub async fn populate_prom(&self) {
        crate::prom::ACTIVE_PROGRAMS.set(
            self.loaded_programs
                .iter()
                .filter(|p| p.enabled.get())
                .count() as f64,
        );
        crate::prom::LOADED_PROGRAMS.set(self.loaded_programs.len() as f64);
    }

    pub async fn tick(&mut self) -> Result<Duration> {
        debug!("starting tick");
        // Clear flags and computed values
        self.pre_tick();

        let events = {
            let mut buf = self.event_buffer.lock().await;
            let buf_cloned = buf.clone();
            buf.clear();
            Arc::new(buf_cloned)
        };
        let now = Instant::now();

        // Handle events for all programs, regardless of whether their tick is enabled or whatnot
        for p in self.loaded_programs.iter() {
            match p.program.handle_incoming_events(events.clone()).await {
                Ok(_) => {}
                Err(err) => {
                    warn!(
                        "program {}: unable to apply events: {:?}",
                        p.program.name, err
                    )
                }
            }
        }

        // Iterate over all addresses
        // Find first without any value set
        // Execute highest-priority program for that address that has not been executed yet
        // Apply that program's outputs to all addresses
        // Repeat until all addresses are set or all programs have been executed
        loop {
            let next_program_to_execute = self
                .tick_values
                .iter()
                .find(|tv| tv.has_higher_priority_program_available())
                .map(|tv| tv.get_highest_priority_non_executed_program())
                .flatten();
            debug!("next program to execute: {:?}", next_program_to_execute);

            match next_program_to_execute {
                None => {
                    debug!("no more addresses to fill, finishing tick");
                    break;
                }
                Some(program) => {
                    program.program.inject_inputs().await?;
                    program.executed_this_tick.set(true);
                    let outputs = program.program.tick(now);
                    match outputs {
                        Ok(outputs) => {
                            debug!(
                                "program {} produced outputs {:?}",
                                program.program.name, outputs
                            );
                            self.apply_tick_generated_outputs(outputs, program.program.priority);
                        }
                        Err(err) => {
                            warn!(
                                "program {}: unable to execute: {:?}",
                                program.program.name, err
                            )
                            // TODO stop executing this for... a while?
                        }
                    }
                }
            }

            if self
                .loaded_programs
                .iter()
                .find(|p| !p.executed_this_tick.get())
                .is_none()
            {
                debug!("all programs executed, finishing tick");
                break;
            }
        }
        let dur = now.elapsed();

        // Collect set requests from generated values
        let set_requests = self.post_tick_collect_values();
        debug!("produced set requests: {:?}", set_requests);
        if !set_requests.is_empty() {
            self.set_tx.send(set_requests).await.unwrap()
        }

        Ok(dur)
    }

    pub async fn events_processed(&self) -> u64 {
        *self.events_processed.lock().await
    }

    async fn subscribe_with_change(
        client: &alloy::tcp::AsyncClient,
        event_subscriptions: &HashMap<Address, EventFilter>,
    ) -> Result<()> {
        for (k, v) in event_subscriptions {
            client
                .subscribe(SubscriptionRequest {
                    address: *k,
                    strategy: v.strategy.clone(),
                    filters: v.entries.clone(),
                })
                .await?;
        }

        Ok(())
    }

    // This has to:
    // - Receive from client
    // - Apply change events to our view of the address space
    // - Filter for events for programs
    // - Forward to broadcast channel
    async fn handle_incoming_events_loop(
        mut incoming: mpsc::Receiver<PushedMessage>,
        universe_view: Arc<Mutex<UniverseView>>,
        events_counter: Arc<Mutex<u64>>,
        event_buffer: Arc<Mutex<VecDeque<AddressedEvent>>>,
    ) {
        debug!("starting incoming events loop");
        while let Some(push_msg) = incoming.recv().await {
            debug!("got push message {:?}", push_msg);
            if let PushedMessage::Event(event) = push_msg {
                debug!("got event {:?}", event);
                // Update event counter
                {
                    let mut events_counter = events_counter.lock().await;
                    *events_counter += 1;
                }

                // First, update our view based on change events
                {
                    let mut values = universe_view.lock().await;

                    values
                        .apply_event(&event)
                        .expect("unable to apply event to universe view");
                }

                // TODO we could filter these by events that any program is subscribed to
                event_buffer.lock().await.push_back(event)
            } else {
                error!("received updated config from server");
                return;
            }
        }
        debug!("quitting incoming events loop");
    }

    async fn load_programs<P: AsRef<Path>>(
        dir: P,
        universe_config: &UniverseConfig,
    ) -> Result<Vec<Program>> {
        let mut programs = Vec::new();
        let files = fs::read_dir(dir)?;
        let files = files
            .filter_map(std::result::Result::ok)
            .filter(|d| d.path().extension() == Some(OsStr::new("lua")))
            .collect_vec();

        for file in files {
            info!("attempting to load program at {:?}...", file.path());
            let p = Program::new(universe_config, file.path())
                .await
                .context(format!("unable to load program at {:?}", file.path()))?;
            programs.push(p);
        }

        Ok(programs
            .into_iter()
            .sorted_by_key(|p| p.priority)
            .rev()
            .collect())
    }

    fn aggregate_event_filters(
        programs: &Vec<Program>,
        addresses: &Vec<Address>,
    ) -> HashMap<Address, EventFilter> {
        let mut all = programs.iter().map(|p| p.event_filters.clone()).fold(
            HashMap::<Address, Vec<EventFilterEntry>, _>::new(),
            |mut acc, filters| {
                filters
                    .into_iter()
                    .for_each(|(k, v)| acc.entry(k).or_default().extend(v.entries.into_iter()));
                acc
            },
        );

        // Add a Update entry for every address
        addresses.iter().for_each(|addr| {
            all.entry(*addr).or_default().push(EventFilterEntry::Kind {
                kind: EventFilterKind::Update,
            })
        });

        // Sort, dedup
        all.iter_mut().for_each(|(_, v)| {
            v.sort_unstable();
            v.dedup();
        });

        // Make them into ANY event filters
        let filters = all
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

        return filters;
    }
}
