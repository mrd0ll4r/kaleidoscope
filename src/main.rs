use alloy::amqp::{ExchangeSubmarineInput, RoutingKeySubscription};
use alloy::config::UniverseConfig;
use std::time::{Duration, Instant};

use crate::config::Config;
use anyhow::Context;
use flexi_logger::{DeferredNow, Logger, LoggerHandle, TS_DASHES_BLANK_COLONS_DOT_BLANK};
use log::{debug, info, Record};
use reqwest::Url;

use crate::runtime::runtime::Runtime;

mod config;
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
    let universe_config = get_universe_config(&submarine_base_url)
        .await
        .context("unable to get universe config from submarine")?;
    debug!("got universe config {:?}", universe_config);

    info!("connecting to AMQP broker...");
    let amqp_client =
        ExchangeSubmarineInput::new(&cfg.amqp_server_address, &[RoutingKeySubscription::All])
            .await
            .context("unable to connect to AMQP broker")?;
    debug!("connected with client {:?}", amqp_client);

    info!("setting up prometheus...");
    let prom_listen_address = cfg
        .prometheus_listen_address
        .parse()
        .context("unable to parse prometheus listen address")?;
    prom::start_prometheus(prom_listen_address).context("unable to start prometheus")?;

    info!("setting up runtime...");
    let mut runtime = Runtime::new(universe_config, submarine_base_url, amqp_client).await?;

    info!("starting tick loop");
    let mut print_ticker = tokio::time::interval(Duration::from_secs(2));
    let mut tick_ticker = tokio::time::interval(Duration::from_millis(4));
    // First tick is free :o
    let mut last_print = print_ticker.tick().await;
    tick_ticker.tick().await;

    let mut events_processed = runtime.events_processed().await;
    let mut total_time_avg = 0.0;
    let mut tick_time_avg = 0.0;
    let mut i = 1_u64;
    loop {
        tokio::select! {
            tick = print_ticker.tick() => {
                let dur = tick.duration_since(last_print).as_secs_f64();
                let current_events_processed = runtime.events_processed().await;
                let events_diff = current_events_processed - events_processed;

                info!(
                    "avg tick+send: {:6.2}µs, tick: {:6.2}µs, send: {:6.2}µs, received {:5} events/s, processed {:5} ticks/s",
                    total_time_avg, tick_time_avg, total_time_avg - tick_time_avg, (events_diff as f64 / dur) as u64, (i as f64 / dur) as u64
                );

                runtime.populate_prom().await;
                prom::EVENTS_PROCESSED.inc_by(events_diff as u64);

                i = 1;
                total_time_avg = 0.0;
                tick_time_avg = 0.0;
                last_print = tick;
                events_processed = current_events_processed;
            },
            _tick = tick_ticker.tick() => {
                let before = Instant::now();
                let inner_duration = runtime.tick().await?;
                let time_taken = before.elapsed().as_micros() as f64;
                debug!("inner tick duration: {:?}, tick+send duration: {}µs",inner_duration, time_taken);

                prom::TICK_INNER_DURATION.observe(inner_duration.as_micros() as f64);
                prom::TICK_DURATION.observe(time_taken as f64);

                total_time_avg += (time_taken - total_time_avg) / i as f64;
                tick_time_avg += (inner_duration.as_micros() as f64 - tick_time_avg) / i as f64;

                i += 1;
            },
        }
    }
}

async fn get_universe_config(submarine_base_url: &Url) -> Result<UniverseConfig> {
    let mut u = submarine_base_url.clone();
    u.set_path("api/v1/universe/config");
    let resp = reqwest::get(u)
        .await
        .context("unable to get universe config from submarine")?
        .json()
        .await
        .context("unable to decode universe config")?;

    Ok(resp)
}
