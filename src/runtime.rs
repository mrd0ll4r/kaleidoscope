use crate::program::Program;
use crate::Result;
use alloy::api::{SetRequest, SetRequestTarget, SubscriptionRequest};
use alloy::config::VirtualDeviceConfig;
use alloy::event::{
    AddressedEvent, EventFilter, EventFilterEntry, EventFilterKind, EventFilterStrategy, EventKind,
};
use alloy::{Address, Value};
use failure::ResultExt;
use itertools::Itertools;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task;

pub(crate) struct Runtime {
    // ordered by priority descending
    loaded_programs: Vec<Program>,
    current_values: Arc<Mutex<HashMap<Address, Value>>>,
    set_tx: mpsc::Sender<Vec<SetRequest>>,
    events_processed: Arc<Mutex<u64>>,
}

impl Runtime {
    pub(crate) async fn new(mut client: alloy::tcp::AsyncClient) -> Result<Runtime> {
        let configs = client.devices().await?;
        let mut values = HashMap::new();
        let events_counter = Arc::new(Mutex::new(0 as u64));
        let task_events_counter = events_counter.clone();

        for cfg in &configs {
            let val = client.get(cfg.address).await?;
            values.insert(cfg.address, val);
        }

        // Set up a broadcast channel to send events to programs.
        let (bc_sender, _) = broadcast::channel(100);

        // Load and setup programs
        let programs = Self::load_programs("programs/", &configs, values.clone(), &bc_sender)?;

        // TODO check if two programs with the same priority output to the same addresses?

        // Aggregate and dedup event subscriptions from all programs...
        let event_subscriptions = Self::aggregate_event_filters(&programs);
        // ... and subscribe to them, with an added type=change entry if missing, for every address.
        // We want the change events to keep our view of values up to date.
        Self::subscribe_with_change(&client, &event_subscriptions, &configs).await?;

        // Handle incoming events, keep our values up to date etc.
        let incoming_events = client.event_stream()?;
        let values = Arc::new(Mutex::new(values));
        let task_values = values.clone();
        task::spawn(Self::handle_incoming_events_loop(
            incoming_events,
            bc_sender,
            event_subscriptions,
            task_values,
            task_events_counter,
        ));

        let (set_tx, mut set_rx) = mpsc::channel(1);

        task::spawn(async move {
            while let Some(reqs) = set_rx.recv().await {
                //let before = Instant::now();
                let res = client.set(reqs).await;
                //println!("set took {}µs",before.elapsed().as_micros());
                if let Err(e) = res {
                    println!("unable to set: {:?}", e);
                    return;
                }
            }
        });

        Ok(Runtime {
            loaded_programs: programs,
            current_values: values,
            set_tx,
            events_processed: events_counter,
        })
    }

    pub async fn tick(&mut self) -> Result<Duration> {
        let values = Arc::new({ self.current_values.lock().await.clone() });
        let mut set_requests = Vec::new();
        let now = Instant::now();
        for program in &self.loaded_programs {
            program.inject_inputs(values.clone())?;
            program.process_events().await?;
            let outputs = program.tick(now)?;
            outputs.into_iter().for_each(|(k, v)| {
                set_requests.push(SetRequest {
                    value: v,
                    target: SetRequestTarget::Address(k),
                })
            })
        }
        let dur = now.elapsed();

        if !set_requests.is_empty() {
            self.set_tx.send(set_requests).await.unwrap()
        }

        Ok(dur)
    }

    pub fn get_values(&self) -> Arc<Mutex<HashMap<Address, Value>>> {
        self.current_values.clone()
    }

    pub async fn events_processed(&self) -> u64 {
        *self.events_processed.lock().await
    }

    async fn subscribe_with_change(
        client: &alloy::tcp::AsyncClient,
        event_subscriptions: &HashMap<Address, EventFilter>,
        configs: &Vec<VirtualDeviceConfig>,
    ) -> Result<()> {
        let mut subs_to_request = event_subscriptions.clone();
        for cfg in configs {
            let entry = subs_to_request.entry(cfg.address).or_insert(EventFilter {
                strategy: EventFilterStrategy::Any,
                entries: vec![EventFilterEntry::Kind {
                    kind: EventFilterKind::Change,
                }],
            });
            if !entry.entries.contains(&EventFilterEntry::Kind {
                kind: EventFilterKind::Change,
            }) {
                entry.entries.push(EventFilterEntry::Kind {
                    kind: EventFilterKind::Change,
                })
            }
        }
        for (k, v) in subs_to_request {
            client
                .subscribe(SubscriptionRequest {
                    address: k,
                    strategy: v.strategy.clone(),
                    filters: v.entries,
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
        mut incoming: mpsc::Receiver<Vec<AddressedEvent>>,
        bc_sender: broadcast::Sender<AddressedEvent>,
        program_event_subscriptions: HashMap<u16, EventFilter>,
        values: Arc<Mutex<HashMap<Address, Value>>>,
        events_counter: Arc<Mutex<u64>>,
    ) {
        while let Some(events) = incoming.recv().await {
            // Update event counter
            {
                let mut events_counter = events_counter.lock().await;
                *events_counter = *events_counter + events.len() as u64;
            }

            // First, update our view based on change events
            {
                let mut values = values.lock().await;
                for event in &events {
                    if let EventKind::Change { new_value } = event.event.inner {
                        values.insert(event.address, new_value);
                    }
                }
            }

            // Then filter based on program subscriptions and push to broadcast channel.
            events
                .into_iter()
                .filter(|e| {
                    let filter = program_event_subscriptions.get(&e.address);
                    match filter {
                        Some(filter) => filter.matches(&e.event),
                        None => false,
                    }
                })
                .for_each(|e| {
                    bc_sender.send(e);
                    // TODO maybe handle no receivers?
                })
        }
    }

    fn load_programs<P: AsRef<Path>>(
        dir: P,
        virtual_devices: &Vec<VirtualDeviceConfig>,
        current_values: HashMap<Address, Value>,
        bc_sender: &broadcast::Sender<AddressedEvent>,
    ) -> Result<Vec<Program>> {
        let mut programs = Vec::new();
        let files = fs::read_dir(dir)?;
        let files = files
            .filter_map(std::result::Result::ok)
            .filter(|d| d.path().extension() == Some(OsStr::new("lua")))
            .collect_vec();

        for file in files {
            let p = Program::new(
                virtual_devices,
                current_values.clone(),
                file.path(),
                bc_sender.subscribe(),
            )
            .context(format!("unable to load program {:?}", file.path()))?;
            programs.push(p);
        }

        programs.sort_unstable_by_key(|p| p.priority);
        programs.reverse();

        Ok(programs)
    }

    fn aggregate_event_filters(programs: &Vec<Program>) -> HashMap<Address, EventFilter> {
        let mut all = programs.iter().map(|p| p.registered_events.clone()).fold(
            HashMap::<Address, Vec<EventFilterEntry>, _>::new(),
            |mut acc, filters| {
                filters
                    .into_iter()
                    .for_each(|(k, mut v)| acc.entry(k).or_default().append(v.as_mut()));
                acc
            },
        );

        all.iter_mut().for_each(|(_, v)| {
            v.sort_unstable();
            v.dedup();
        });

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
