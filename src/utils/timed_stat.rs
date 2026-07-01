use std::{cmp::Ordering, collections::BinaryHeap};

/// Computes the running average and variance of time series values.
#[derive(Debug, Clone)]
pub struct TimedStat {
    values: BinaryHeap<Entry>,
    last_ts: Option<chrono::Duration>,
    stat: Stat,
    window: chrono::Duration,
}

impl TimedStat {
    pub fn new(window: chrono::Duration) -> Self {
        assert!(window > chrono::Duration::zero());

        Self {
            window,
            values: BinaryHeap::new(),
            last_ts: None,
            stat: Stat::default(),
        }
    }

    pub fn set_last_ts(&mut self, last_ts: chrono::Duration) -> Vec<(chrono::Duration, f64)> {
        self.last_ts = Some(last_ts);
        let stat = &mut self.stat;

        // Discard out-of-window values
        let lower_ts = last_ts - self.window;
        let mut discarded = vec![];

        while let Some(&Entry { time: ts, value }) = self.values.peek() {
            if ts >= lower_ts {
                break;
            }

            self.values.pop();
            discarded.push((ts, value));
            stat.sum -= value;
            stat.sum_squares -= value.powi(2);
        }

        self.update_stat();

        discarded
    }

    pub fn push(&mut self, ts: chrono::Duration, new_value: f64) -> Vec<(chrono::Duration, f64)> {
        // Check if the timestamp succeeds the last timestamp

        self.last_ts = Some(ts);
        self.values.push(Entry {
            time: ts,
            value: new_value,
        });

        let stat = &mut self.stat;
        stat.sum += new_value;
        stat.sum_squares += new_value.powi(2);

        // Discard out-of-window values
        self.set_last_ts(ts)
    }

    pub fn stat(&self) -> &Stat {
        &self.stat
    }

    fn update_stat(&mut self) {
        let stat = &mut self.stat;
        let window_secs = self.window.to_std().unwrap().as_secs_f64();
        let mean = stat.sum / window_secs;
        let var = stat.sum_squares / window_secs - mean.powi(2);
        stat.mean = mean;
        stat.var = var;
        stat.stdev = var.sqrt();
    }
}

#[derive(Debug, Clone)]
pub struct Stat {
    pub sum: f64,
    pub sum_squares: f64,
    pub mean: f64,
    pub var: f64,
    pub stdev: f64,
}

impl Default for Stat {
    fn default() -> Self {
        Self {
            sum: 0.0,
            sum_squares: 0.0,
            mean: 0.0,
            var: 0.0,
            stdev: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    pub time: chrono::Duration,
    pub value: f64,
}

impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for Entry {}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time).reverse()
    }
}

#[cfg(test)]
mod tests {
    use super::TimedStat;
    use chrono::Duration;

    #[test]
    fn single_push_mean_is_value_over_window() {
        let mut ts = TimedStat::new(Duration::seconds(1));
        ts.push(Duration::seconds(0), 2.0);
        assert_eq!(ts.stat().sum, 2.0);
        assert!((ts.stat().mean - 2.0).abs() < 1e-9); // 2.0 / 1s
    }

    #[test]
    fn accumulates_within_window() {
        let mut ts = TimedStat::new(Duration::seconds(10));
        ts.push(Duration::seconds(0), 1.0);
        ts.push(Duration::seconds(1), 3.0);
        assert_eq!(ts.stat().sum, 4.0);
        assert!((ts.stat().mean - 0.4).abs() < 1e-9); // 4 / 10s
    }

    #[test]
    fn evicts_values_outside_window() {
        let mut ts = TimedStat::new(Duration::seconds(1));
        ts.push(Duration::seconds(0), 5.0);
        // last_ts becomes 2s; window lower bound is 1s, so t=0 is evicted.
        let discarded = ts.push(Duration::seconds(2), 3.0);
        assert_eq!(discarded, vec![(Duration::seconds(0), 5.0)]);
        assert_eq!(ts.stat().sum, 3.0);
        assert!((ts.stat().mean - 3.0).abs() < 1e-9);
    }
}
