#[macro_use]
extern crate lazy_static;
//#[macro_use]
//extern crate prometheus;
#[macro_use]
extern crate log;

use crate::net::Client;
use crate::runtime::Runtime;
use failure::Error;
use flexi_logger::{DeferredNow, Logger};
use log::Record;
use std::thread;
use std::time::{Duration, Instant};
use futures::FutureExt;

mod net;
mod program;
mod runtime;
//mod prom;

pub(crate) type Result<T> = std::result::Result<T, Error>;

const REMOTE: &str = "127.0.0.1:3030";

pub(crate) fn log_format(
    w: &mut dyn std::io::Write,
    now: &mut DeferredNow,
    record: &Record,
) -> std::result::Result<(), std::io::Error> {
    write!(
        w,
        "[{}] {} [{}] {}:{}: {}",
        now.now().format("%Y-%m-%d %H:%M:%S%.6f %:z"),
        record.level(),
        record.metadata().target(),
        //record.module_path().unwrap_or("<unnamed>"),
        record.file().unwrap_or("<unnamed>"),
        record.line().unwrap_or(0),
        &record.args()
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    Logger::with_env_or_str("debug")
        .format(log_format)
        //.log_to_file()
        //.directory("logs")
        //.duplicate_to_stderr(Duplicate::All)
        //.rotate(
        //    Criterion::Size(100_000_000),
        //    Naming::Timestamps,
        //    Cleanup::KeepLogFiles(10),
        //)
        .start()?;

    let client = Client::new(REMOTE).await?;

    for i in 1..2 {
        let before = Instant::now();
        client.ping().await?;
        let elapsed = before.elapsed();
        println!("ping {} took {}µs", i, elapsed.as_micros());

        thread::sleep(Duration::from_secs(1))
    }

    let configs = client.devices().await?;
    for config in &configs {
        println!(
            "{:5} {:2} {:20} [{}]",
            config.address,
            if config.read_only { "R" } else { "RW" },
            config.alias,
            config.groups.join(", ")
        );
    }

    let mut runtime = Runtime::new(client).await?;

    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    // First tick is free :o
    let mut last_print = ticker.tick().await;

    let mut events_processed = runtime.events_processed().await;
    let mut total_time_avg = 0.0;
    let mut tick_time_avg = 0.0;
    let mut i = 1_u64;
    loop {
        let before = Instant::now();
        let inner_duration = runtime.tick().await?;
        let time_taken = before.elapsed().as_micros() as f64;

        total_time_avg += (time_taken - total_time_avg) / i as f64;
        tick_time_avg += (inner_duration.as_micros() as f64 - tick_time_avg) / i as f64;

        i += 1;

        futures::select! {
            tick = ticker.tick().fuse() => {
            let dur = tick.duration_since(last_print).as_secs_f64();
            let current_events_processed = runtime.events_processed().await;
            let events_diff = current_events_processed - events_processed;
            println!(
                "avg tick+send: {:6.2}µs, tick: {:6.2}µs, send: {:6.2}µs, received {:5} events/s, processed {:5} ticks/s",
                total_time_avg, tick_time_avg, total_time_avg - tick_time_avg, (events_diff as f64 / dur) as u64, (i as f64 / dur) as u64
            );
            i = 1;
            total_time_avg = 0.0;
            tick_time_avg = 0.0;
            last_print = tick;
            events_processed = current_events_processed;
            },
            default => {},
        };
    }

    Ok(())
}
