use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct TimedStat {
    values: VecDeque<(chrono::Duration, f64)>,
    last_ts: Option<chrono::Duration>,
    stat: Stat,
    window: chrono::Duration,
}

impl TimedStat {
    pub fn new(window: chrono::Duration) -> Self {
        assert!(window > chrono::Duration::zero());

        Self {
            window,
            values: VecDeque::new(),
            last_ts: None,
            stat: Stat::default(),
        }
    }

    pub fn set_last_ts(&mut self, ts: chrono::Duration) -> Vec<(chrono::Duration, f64)> {
        self.last_ts = Some(ts);
        let stat = &mut self.stat;

        // Discard out-of-window values
        let lower_ts = ts - self.window;
        let mut discarded = vec![];

        while let Some(&(ts, value)) = self.values.front() {
            if ts >= lower_ts {
                break;
            }

            self.values.pop_front();
            discarded.push((ts, value));
            stat.sum -= value;
            stat.sum_squares -= value.powi(2);
        }

        self.update_stat();

        discarded
    }

    pub fn push(
        &mut self,
        ts: chrono::Duration,
        new_value: f64,
    ) -> Result<Vec<(chrono::Duration, f64)>, chrono::Duration> {
        // Check if the timestamp succeeds the last timestamp
        match self.last_ts {
            Some(last_ts) if ts < last_ts => {
                return Err(last_ts);
            }
            _ => {}
        }

        self.last_ts = Some(ts);
        self.values.push_back((ts, new_value));

        let stat = &mut self.stat;
        stat.sum += new_value;
        stat.sum_squares += new_value.powi(2);

        // Discard out-of-window values
        let discarded = self.set_last_ts(ts);

        Ok(discarded)
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
