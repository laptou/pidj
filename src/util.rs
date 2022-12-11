use std::time::{Duration, Instant};

pub struct Interval {
    last_tick: Instant,
    period: Duration,
}

impl Interval {
    pub fn new(period: Duration) -> Self {
        Self {
            last_tick: Instant::now(),
            period,
        }
    }

    pub fn tick(&mut self) {
        let current_tick = Instant::now();
        let last_tick_duration = current_tick - self.last_tick;
        self.last_tick = current_tick;

        if last_tick_duration < self.period {
            std::thread::sleep(self.period - last_tick_duration);
        }
    }
}
