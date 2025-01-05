use crate::runtime::fixture::Fixture;
use alloy::api::SetRequest;
use alloy::config::UniverseConfig;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Local};
use log::{debug, warn};
use std::fs;
use std::path::Path;
use std::time::Instant;

pub(crate) struct TickState {
    pub(crate) timestamp: Instant,
    pub(crate) local_time: DateTime<Local>,
}

struct WrappedFixture {
    inner: Fixture,
    set_requests: Vec<SetRequest>,
}

impl WrappedFixture {
    fn wrap(fixture: Fixture) -> WrappedFixture {
        let num_outputs = fixture.addresses.len();
        WrappedFixture {
            inner: fixture,
            set_requests: Vec::with_capacity(num_outputs),
        }
    }

    fn tick(&mut self, state: &TickState) -> Result<&[SetRequest]> {
        self.set_requests.clear();
        self.inner
            .run_current_program(state, &mut self.set_requests)?;

        debug!(
            "{}::run_current_program produced set requests {:?}",
            self.inner.name, self.set_requests
        );

        Ok(&self.set_requests)
    }
}

pub(crate) struct Runtime {
    fixtures: Vec<WrappedFixture>,
    set_requests: Vec<SetRequest>,
}

impl Runtime {
    pub(crate) fn new<P: AsRef<Path>>(
        fixtures_root: P,
        universe_config: &UniverseConfig,
    ) -> Result<Runtime> {
        let mut fixtures: Vec<Fixture> = Vec::new();
        for entry in fs::read_dir(&fixtures_root).context("unable to list fixtures")? {
            let entry = entry.context("unable to enumerate fixtures sources")?;
            let path = entry.path();
            if path.is_dir() {
                // Skip
                continue;
            }

            // Attempt to load as a fixture
            let fix = Fixture::new(&path, universe_config)
                .context(format!("unable to load fixture at {:?}", &path))?;

            if let Some(f) = fixtures.iter().find(|f| f.name == fix.name) {
                bail!(
                    "duplicate fixture: {} in file {:?} (other was {:?})",
                    fix.name,
                    &path,
                    &f.source_path
                )
            }

            fixtures.push(fix)
        }

        Ok(Runtime {
            fixtures: fixtures.into_iter().map(WrappedFixture::wrap).collect(),
            set_requests: Vec::with_capacity(16),
        })
    }

    pub(crate) fn tick(&mut self) -> Result<&[SetRequest]> {
        self.set_requests.clear();

        let now = Instant::now();
        let dt = Local::now();
        let ts = TickState {
            timestamp: now.clone(),
            local_time: dt,
        };

        for fixture in self.fixtures.iter_mut() {
            match fixture.tick(&ts) {
                Err(err) => {
                    warn!("unable to tick fixture {}: {:?}", fixture.inner.name, err)
                }
                Ok(res) => self.set_requests.extend(res.iter().cloned()),
            }
        }
        debug!("tick took {}µs", now.elapsed().as_micros());
        debug!("tick produced set requests {:?}", self.set_requests);

        Ok(&self.set_requests)
    }

    pub(crate) fn alloy_metadata(
        &self,
        universe: &UniverseConfig,
    ) -> alloy::program::KaleidoscopeMetadata {
        alloy::program::KaleidoscopeMetadata {
            fixtures: self
                .fixtures
                .iter()
                .map(|f| &f.inner)
                .map(|f| (f.name.clone(), f.alloy_metadata(universe)))
                .collect(),
        }
    }

    pub(crate) fn get_fixture(&self, name: &str) -> Option<&Fixture> {
        self.fixtures
            .iter()
            .find(|f| f.inner.name == name)
            .map(|f| &f.inner)
    }

    pub(crate) fn get_fixture_mut(&mut self, name: &str) -> Option<&mut Fixture> {
        self.fixtures
            .iter_mut()
            .find(|f| f.inner.name == name)
            .map(|f| &mut f.inner)
    }
}
