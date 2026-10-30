pub mod chunk_gen_stat;
pub mod chunk_identification;
pub mod cpu_load_stat;
pub mod file_io_stat;
pub mod fps_stat;
pub mod heap_stat;
pub mod io_summary;
pub mod packet_identification;
pub mod structure_gen_stat;
pub mod thread_allocation_stat;
pub mod tick_time_stat;
pub mod timed_stat;
pub mod timed_stat_summary;

pub fn seconds(nanos: i64) -> i64 {
    nanos.div_euclid(1_000_000_000)
}

pub fn millis(nanos: i64) -> i64 {
    nanos.div_euclid(1_000_000)
}
