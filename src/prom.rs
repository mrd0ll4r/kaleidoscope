use crate::Result;
use lazy_static::lazy_static;
use prometheus::exponential_buckets;
use prometheus::{register_gauge, register_histogram, Gauge, Histogram};
use std::net::SocketAddr;

// Runtime-related metrics.
lazy_static! {
    pub static ref LOADED_PROGRAMS: Gauge =
        register_gauge!("loaded_programs", "number of programs loaded").unwrap();
    pub static ref ACTIVE_PROGRAMS: Gauge =
        register_gauge!("active_programs", "number of programs currently active").unwrap();
    pub static ref TICK_DURATION: Histogram = register_histogram!(
        "tick_duration",
        "execution time of currently active programs, in microseconds",
        exponential_buckets(100_f64, (1.5_f64).sqrt(), 10).unwrap()
    )
    .unwrap();
    pub static ref SEND_DURATION: Histogram = register_histogram!(
        "send_duration",
        "duration to send set requests of one tick to submarine, in microseconds",
        exponential_buckets(100_f64, (1.5_f64).sqrt(), 10).unwrap()
    )
    .unwrap();
}

pub(crate) fn start_prometheus(addr: SocketAddr) -> Result<()> {
    prometheus_exporter::start(addr)?;
    Ok(())
}
