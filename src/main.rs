use alloy::api::SetRequest;
use alloy::config::UniverseConfig;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::Config;
use anyhow::Context;
use flexi_logger::{DeferredNow, Logger, LoggerHandle, TS_DASHES_BLANK_COLONS_DOT_BLANK};
use log::{debug, info, warn, Record};
use reqwest::Url;
use tokio::sync::Mutex;
use tokio::task;

mod config;
mod http;
mod prom;
mod runtime;

pub(crate) type Result<T> = anyhow::Result<T>;

fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> std::result::Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {} [{}] {}:{}: {}",
        now.format(TS_DASHES_BLANK_COLONS_DOT_BLANK),
        record.level(),
        record.metadata().target(),
        //record.module_path().unwrap_or("<unnamed>"),
        record.file().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        &record.args()
    )
}

pub fn set_up_logging() -> std::result::Result<LoggerHandle, Box<dyn std::error::Error>> {
    let logger = Logger::try_with_env_or_str("info")?
        .use_utc()
        .format(log_format);

    let handle = logger.start()?;

    Ok(handle)
}

#[tokio::main]
async fn main() -> Result<()> {
    set_up_logging().unwrap();

    info!("reading config file...");
    let cfg = Config::read_from_file("config.yaml").context("unable to read config file")?;
    debug!("read config {:?}", cfg);

    info!("connecting to Submarine...");
    let submarine_base_url =
        Url::parse(&cfg.submarine_http_url).context("unable to parse submarine base URL")?;
    let submarine_client = reqwest::ClientBuilder::default()
        .build()
        .expect("unable to build HTTP client");
    let universe_config = get_universe_config(&submarine_base_url, &submarine_client)
        .await
        .context("unable to get universe config from submarine")?;
    debug!("got universe config {:?}", universe_config);

    /*
    info!("connecting to AMQP broker...");
    let amqp_client =
        ExchangeSubmarineInput::new(&cfg.amqp_server_address, &[RoutingKeySubscription::All])
            .await
            .context("unable to connect to AMQP broker")?;
    debug!("connected with client {:?}", amqp_client);
     */

    info!("setting up prometheus...");
    let prom_listen_address = cfg
        .prometheus_listen_address
        .parse()
        .context("unable to parse prometheus listen address")?;
    prom::start_prometheus(prom_listen_address).context("unable to start prometheus")?;

    info!("setting up runtime...");
    let runtime = runtime::runtime::Runtime::new(&cfg.fixtures_path, &universe_config)
        .context("unable to set up runtime")?;
    let runtime = Arc::new(Mutex::new(runtime));

    info!("starting HTTP server...");
    let http_server_address = cfg.http_listen_address.parse()?;
    let _http_server = task::spawn(http::run_server(
        http_server_address,
        runtime.clone(),
        Arc::new(universe_config),
    ));
    info!("HTTP server is listening on http://{}", http_server_address);

    info!("starting tick loop");
    let mut print_ticker = tokio::time::interval(Duration::from_secs(2));
    let mut tick_ticker = tokio::time::interval(Duration::from_millis(5));
    // First tick is free :o
    let mut last_print = print_ticker.tick().await;
    tick_ticker.tick().await;

    let mut send_time_avg = 0.0;
    let mut tick_time_avg = 0.0;
    let mut i = 1_u64;
    let mut set_requests = Vec::new();
    loop {
        tokio::select! {
            tick = print_ticker.tick() => {
                let dur = tick.duration_since(last_print).as_secs_f64();

                info!(
                    "avg tick: {:6.2}µs, send: {:6.2}µs, processed {:5} ticks/s",
                    tick_time_avg, send_time_avg,  (i as f64 / dur) as u64
                );

                i = 1;
                send_time_avg = 0.0;
                tick_time_avg = 0.0;
                last_print = tick;
            },
            _tick = tick_ticker.tick() => {
                // Execute a tick.
                // Only lock the runtime for the tick and copy the set requests out.
                set_requests.clear();
                let tick_time_taken = {
                    let mut runtime = runtime.lock().await;
                    let before = Instant::now();
                    let res = runtime.tick();
                    let time_taken = before.elapsed().as_micros() as f64;
                    match res {
                        Ok(reqs) => {
                            set_requests.extend_from_slice(reqs)
                        }
                        Err(err) => {
                            warn!("tick failed: {:?}",err);
                            continue
                        }
                    }
                    time_taken
                };

                // Send set requests to submarine.
                let before = Instant::now();
                if let Err(e) = post_set_requests(&submarine_base_url, &submarine_client, &set_requests).await {
                    warn!("unable to post set requests to submarine: {:?}", e);
                    continue
                }
                let send_time_taken = before.elapsed().as_micros() as f64;

                debug!("inner tick duration: {}µs, send duration: {}µs",tick_time_taken, send_time_taken);

                prom::TICK_DURATION.observe(tick_time_taken);
                prom::SEND_DURATION.observe(send_time_taken);

                send_time_avg += (send_time_taken - send_time_avg) / i as f64;
                tick_time_avg += (tick_time_taken - tick_time_avg) / i as f64;

                i += 1;
            },
        }
    }
}

async fn get_universe_config(
    submarine_base_url: &Url,
    client: &reqwest::Client,
) -> Result<UniverseConfig> {
    let mut u = submarine_base_url.clone();
    u.set_path("api/v1/universe/config");
    let resp = client
        .get(u)
        .send()
        .await
        .context("unable to get universe config from submarine")?
        .json()
        .await
        .context("unable to decode universe config")?;

    Ok(resp)
}

async fn post_set_requests(
    submarine_base_url: &Url,
    client: &reqwest::Client,
    set_requests: &[SetRequest],
) -> Result<()> {
    let mut u = submarine_base_url.clone();
    u.set_path("api/v1/universe/set");

    client
        .post(u)
        .json(set_requests)
        .send()
        .await
        .context("unable to post set requests to submarine")?;

    Ok(())
}
