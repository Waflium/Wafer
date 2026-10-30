use std::collections::HashMap;

use super::metric_category::MetricCategory;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SamplingPhase {
    Extract,
    EndTick,
}

pub struct MetricSampler {
    name: String,
    sampling_phase: SamplingPhase,
    category: MetricCategory,
    sampler: Box<dyn FnMut() -> f64 + Send>,
    before_tick: Option<Box<dyn FnMut() + Send>>,
    threshold_test: Option<Box<dyn FnMut(f64) -> bool + Send>>,
    values: Vec<f64>,
    ticks: Vec<i32>,
    is_running: bool,
    current_value: f64,
    hash_code: i32,
}

pub struct SamplerResult {
    recording: HashMap<i32, f64>,
    first_tick: i32,
    last_tick: i32,
}

impl SamplerResult {
    pub fn value_at_tick(&self, tick: i32) -> f64 {
        self.recording.get(&tick).copied().unwrap_or(0.0)
    }

    pub fn get_first_tick(&self) -> i32 {
        self.first_tick
    }

    pub fn get_last_tick(&self) -> i32 {
        self.last_tick
    }
}

pub struct MetricSamplerBuilder {
    name: String,
    category: MetricCategory,
    sampler: Box<dyn FnMut() -> f64 + Send>,
    sampling_phase: SamplingPhase,
    before_tick: Option<Box<dyn FnMut() + Send>>,
    threshold_test: Option<Box<dyn FnMut(f64) -> bool + Send>>,
}

impl MetricSamplerBuilder {
    pub fn with_before_tick(mut self, before_tick: impl FnMut() + Send + 'static) -> MetricSamplerBuilder {
        self.before_tick = Some(Box::new(before_tick));
        self
    }

    pub fn with_threshold_alert(mut self, threshold_test: impl FnMut(f64) -> bool + Send + 'static) -> MetricSamplerBuilder {
        self.threshold_test = Some(Box::new(threshold_test));
        self
    }

    pub fn with_sampling_phase(mut self, sampling_phase: SamplingPhase) -> MetricSamplerBuilder {
        self.sampling_phase = sampling_phase;
        self
    }

    pub fn build(self) -> MetricSampler {
        MetricSampler::new(self.name, self.sampling_phase, self.category, self.sampler, self.before_tick, self.threshold_test)
    }
}

impl MetricSampler {
    fn new(
        name: String,
        sampling_phase: SamplingPhase,
        category: MetricCategory,
        sampler: Box<dyn FnMut() -> f64 + Send>,
        before_tick: Option<Box<dyn FnMut() + Send>>,
        threshold_test: Option<Box<dyn FnMut(f64) -> bool + Send>>,
    ) -> MetricSampler {
        let hash_code = wafer_core::random::java_string_hash(&name);
        MetricSampler { name, sampling_phase, category, sampler, before_tick, threshold_test, values: Vec::new(), ticks: Vec::new(), is_running: true, current_value: 0.0, hash_code }
    }

    pub fn hash_code(&self) -> i32 {
        self.hash_code
    }

    pub fn with_hash_code(mut self, hash_code: i32) -> MetricSampler {
        self.hash_code = hash_code;
        self
    }

    pub fn create(name: &str, category: MetricCategory, sampler: impl FnMut() -> f64 + Send + 'static) -> MetricSampler {
        MetricSampler::new(name.to_string(), SamplingPhase::EndTick, category, Box::new(sampler), None, None)
    }

    pub fn create_extract_sampler(name: &str, category: MetricCategory, sampler: impl FnMut() -> f64 + Send + 'static) -> MetricSampler {
        MetricSampler::new(name.to_string(), SamplingPhase::Extract, category, Box::new(sampler), None, None)
    }

    pub fn builder(metric_name: &str, category: MetricCategory, sampler: impl FnMut() -> f64 + Send + 'static) -> MetricSamplerBuilder {
        MetricSamplerBuilder { name: metric_name.to_string(), category, sampler: Box::new(sampler), sampling_phase: SamplingPhase::EndTick, before_tick: None, threshold_test: None }
    }

    pub fn on_start_tick(&mut self) {
        assert!(self.is_running, "Not running");
        if let Some(before_tick) = &mut self.before_tick {
            before_tick();
        }
    }

    pub fn on_end_tick(&mut self, current_tick: i32) {
        self.verify_running();
        self.current_value = (self.sampler)();
        self.values.push(self.current_value);
        self.ticks.push(current_tick);
    }

    pub fn on_finished(&mut self) {
        self.verify_running();
        self.values.clear();
        self.ticks.clear();
        self.is_running = false;
    }

    fn verify_running(&self) {
        assert!(self.is_running, "Sampler for metric {} not started!", self.name);
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn sampling_phase(&self) -> SamplingPhase {
        self.sampling_phase
    }

    pub fn get_category(&self) -> MetricCategory {
        self.category
    }

    pub fn result(&self) -> SamplerResult {
        let mut recording = HashMap::new();
        let mut first_tick = i32::MIN;
        let mut last_tick = i32::MIN;
        for (&tick, &value) in self.ticks.iter().zip(&self.values) {
            if first_tick == i32::MIN {
                first_tick = tick;
            }
            recording.insert(tick, value);
            last_tick = tick;
        }
        SamplerResult { recording, first_tick, last_tick }
    }

    pub fn triggers_threshold(&mut self) -> bool {
        let value = self.current_value;
        match &mut self.threshold_test {
            Some(test) => test(value),
            None => false,
        }
    }
}

pub fn value_increased_by_percentage(percentage_increase_threshold: f32) -> impl FnMut(f64) -> bool + Send {
    let mut previous_value = f64::from_bits(1);
    move |value| {
        let result = if previous_value != f64::from_bits(1) && value > previous_value {
            (value - previous_value) / previous_value >= f64::from(percentage_increase_threshold)
        } else {
            false
        };
        previous_value = value;
        result
    }
}
