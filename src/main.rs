#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate prometheus;
#[macro_use]
extern crate log;
#[macro_use]
extern crate failure;

use std::thread;
use std::time::{Duration, Instant};

use alloy::tcp::AsyncClient;
use failure::{Error, ResultExt};
use flexi_logger::{DeferredNow, Logger, LoggerHandle, TS_DASHES_BLANK_COLONS_DOT_BLANK};
use log::Record;

use crate::runtime::runtime::Runtime;

mod prom;
mod runtime;

const REMOTE: &str = "127.0.0.1:3030";

pub(crate) type Result<T> = std::result::Result<T, Error>;

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

    info!("connecting...");
    let (client, push_receiver) = alloy::tcp::AsyncClient::new(REMOTE).await?;

    info!("setting up prometheus...");
    prom::start_prometheus("127.0.0.1:4343".parse().unwrap())
        .context("unable to start prometheus")?;

    info!("testing connection...");
    ping(&client).await?;

    info!("setting up runtime...");
    let mut runtime = Runtime::new(client, push_receiver).await?;

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
            //default => {},
        };
    }

    Ok(())
}

async fn ping(client: &AsyncClient) -> Result<()> {
    let mut pings = Vec::new();
    for _i in 1..100 {
        let before = Instant::now();
        client.ping().await?;
        let elapsed = before.elapsed();
        pings.push(elapsed.as_micros() as f64);

        thread::sleep(Duration::from_millis(20))
    }
    let min = pings.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max = pings.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let mean = statistical::mean(&pings);
    let stddev = statistical::standard_deviation(&pings, Some(mean));
    info!(
        "ping: min/max/mean {:.2}/{:.2}/{:.2} µs, stddev {:.2}",
        min, max, mean, stddev
    );

    Ok(())
}
