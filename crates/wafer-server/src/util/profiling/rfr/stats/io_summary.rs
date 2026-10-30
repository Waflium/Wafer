use super::seconds;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CountAndSize {
    pub total_count: i64,
    pub total_size: i64,
}

impl CountAndSize {
    pub fn add(self, that: CountAndSize) -> CountAndSize {
        CountAndSize { total_count: self.total_count + that.total_count, total_size: self.total_size + that.total_size }
    }

    pub fn average_size(&self) -> f32 {
        self.total_size as f32 / self.total_count as f32
    }
}

pub struct IoSummary<T> {
    total_count_and_size: CountAndSize,
    largest_size_contributors: Vec<(T, CountAndSize)>,
    recording_duration: i64,
}

impl<T: Clone> IoSummary<T> {
    pub fn new(recording_duration: i64, packet_stats: Vec<(T, CountAndSize)>) -> IoSummary<T> {
        let total_count_and_size = packet_stats.iter().map(|(_, stat)| *stat).fold(CountAndSize::default(), CountAndSize::add);
        let mut sorted = packet_stats;
        sorted.sort_by(|(_, a), (_, b)| b.total_size.cmp(&a.total_size).then(b.total_count.cmp(&a.total_count)));
        sorted.truncate(10);
        IoSummary { total_count_and_size, largest_size_contributors: sorted, recording_duration }
    }

    pub fn get_counts_per_second(&self) -> f64 {
        self.total_count_and_size.total_count as f64 / seconds(self.recording_duration) as f64
    }

    pub fn get_size_per_second(&self) -> f64 {
        self.total_count_and_size.total_size as f64 / seconds(self.recording_duration) as f64
    }

    pub fn get_total_count(&self) -> i64 {
        self.total_count_and_size.total_count
    }

    pub fn get_total_size(&self) -> i64 {
        self.total_count_and_size.total_size
    }

    pub fn largest_size_contributors(&self) -> &[(T, CountAndSize)] {
        &self.largest_size_contributors
    }
}
