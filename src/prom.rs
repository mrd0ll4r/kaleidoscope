use crate::Result;
use prometheus::exponential_buckets;
use prometheus::{Gauge, GaugeVec, Histogram, IntCounter, IntGauge, IntGaugeVec};
use std::net::SocketAddr;
use std::thread;
use std::time::Duration;
use systemstat::{saturating_sub_bytes, Platform, System};

// Runtime-related metrics.
lazy_static! {
    pub static ref LOADED_PROGRAMS: Gauge =
        register_gauge!("loaded_programs",
        "number of programs loaded").unwrap();
    pub static ref ACTIVE_PROGRAMS: Gauge =
        register_gauge!("active_programs", "number of programs currently active").unwrap();

    pub static ref TICK_INNER_DURATION: Histogram = register_histogram!(
        "tick_inner_duration",
        "execution time of currently active programs, in microseconds",
        exponential_buckets(100_f64, (1.5_f64).sqrt(), 10).unwrap()
    )
    .unwrap();
    pub static ref TICK_DURATION: Histogram = register_histogram!(
        "tick_duration",
        "duration of a tick, i.e. one round of program execution + communication with Submarine, in microseconds",
        exponential_buckets(100_f64, (1.5_f64).sqrt(), 10).unwrap()
    )
    .unwrap();
}

// Event-related metrics.
lazy_static! {
    pub static ref EVENTS_PROCESSED: IntCounter =
        register_int_counter!("events_processed", "number of events processed").unwrap();
}

// System-related metrics
lazy_static! {
    pub static ref SYSTEM_MEMORY_USED: IntGauge = register_int_gauge!(
        "system_memory_used",
        "amount of memory used (=total-free) in bytes"
    )
    .unwrap();
    pub static ref SYSTEM_LOAD_AVERAGE: GaugeVec =
        register_gauge_vec!("system_load_average", "Linux load average", &["duration"]).unwrap();
    pub static ref SYSTEM_CPU_TEMPERATURE: Gauge =
        register_gauge!("system_cpu_temperature", "CPU temperature in celsius").unwrap();
    pub static ref SYSTEM_NETWORK_STATS: IntGaugeVec = register_int_gauge_vec!(
        "system_network_stats",
        "network statistics by interface and value",
        &["interface", "value"]
    )
    .unwrap();
}

pub(crate) fn start_prometheus(addr: SocketAddr) -> Result<()> {
    thread::Builder::new()
        .name("prom-system-stats".to_string())
        .spawn(track_system_stats)?;
    prometheus_exporter::start(addr)?;
    Ok(())
}

fn track_system_stats() {
    let sys = System::new();
    let load_avg_one = SYSTEM_LOAD_AVERAGE
        .get_metric_with_label_values(&["1m"])
        .unwrap();
    let load_avg_five = SYSTEM_LOAD_AVERAGE
        .get_metric_with_label_values(&["5m"])
        .unwrap();
    let load_avg_fifteen = SYSTEM_LOAD_AVERAGE
        .get_metric_with_label_values(&["15m"])
        .unwrap();

    loop {
        match sys.memory() {
            Ok(mem) => {
                let used = saturating_sub_bytes(mem.total, mem.free);
                SYSTEM_MEMORY_USED.set(used.as_u64() as i64);
            }
            Err(x) => warn!("unable to get memory stats: {}", x),
        }

        match sys.load_average() {
            Ok(load_avg) => {
                load_avg_one.set(load_avg.one as f64);
                load_avg_five.set(load_avg.five as f64);
                load_avg_fifteen.set(load_avg.fifteen as f64)
            }
            Err(x) => warn!("unable to get load average: {}", x),
        }

        match sys.cpu_temp() {
            Ok(cpu_temp) => {
                SYSTEM_CPU_TEMPERATURE.set(cpu_temp as f64);
            }
            Err(x) => warn!("unable to get CPU temperature: {}", x),
        }

        match sys.networks() {
            Ok(netifs) => {
                for netif in netifs.values() {
                    let stats = sys.network_stats(&netif.name);
                    match stats {
                        Ok(stats) => {
                            SYSTEM_NETWORK_STATS
                                .get_metric_with_label_values(&[netif.name.as_str(), "rx_bytes"])
                                .unwrap()
                                .set(stats.rx_bytes.as_u64() as i64);
                            SYSTEM_NETWORK_STATS
                                .get_metric_with_label_values(&[netif.name.as_str(), "tx_bytes"])
                                .unwrap()
                                .set(stats.tx_bytes.as_u64() as i64);
                            SYSTEM_NETWORK_STATS
                                .get_metric_with_label_values(&[netif.name.as_str(), "rx_packets"])
                                .unwrap()
                                .set(stats.rx_packets as i64);
                            SYSTEM_NETWORK_STATS
                                .get_metric_with_label_values(&[netif.name.as_str(), "tx_packets"])
                                .unwrap()
                                .set(stats.tx_packets as i64);
                            SYSTEM_NETWORK_STATS
                                .get_metric_with_label_values(&[netif.name.as_str(), "rx_errors"])
                                .unwrap()
                                .set(stats.rx_errors as i64);
                            SYSTEM_NETWORK_STATS
                                .get_metric_with_label_values(&[netif.name.as_str(), "tx_errors"])
                                .unwrap()
                                .set(stats.tx_errors as i64);
                        }
                        Err(e) => {
                            warn!("unable to get stats for interface {}: {}", netif.name, e);
                        }
                    }
                }
            }
            Err(x) => warn!("unable to get interfaces: {}", x),
        }

        thread::sleep(Duration::from_secs(2));
    }
}
