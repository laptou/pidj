use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

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

/// Computes the intersection of two paths (finds the longest shared segment at
/// the beginning of the paths).
pub fn path_intersection(left: impl AsRef<Path>, right: impl AsRef<Path>) -> PathBuf {
    left.as_ref()
        .into_iter()
        .zip(right.as_ref().into_iter())
        .map_while(|(l, r)| if l == r { Some(l) } else { None })
        .collect()
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    #[test]
    fn path_intersection() {
        let p1 = PathBuf::from("/home/pi/audio/Cymatics - Lofi Starter Pack/Claps/Cymatics - Dreams Lofi Clap 3.wav");
        let p2 = PathBuf::from("/home/pi/audio/Cymatics - Lofi Starter Pack/Claps/Cymatics - Old Clap.wav");

        let o = super::path_intersection(p1, p2);
        let e = PathBuf::from("/home/pi/audio/Cymatics - Lofi Starter Pack/Claps");
        assert_eq!(o, e);
    }
}
