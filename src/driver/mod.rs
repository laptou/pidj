use std::time::Duration;

pub mod adafruit;

pub struct ThreadDelay;

impl embedded_hal::blocking::delay::DelayUs<u32> for ThreadDelay {
    fn delay_us(&mut self, us: u32) {
        std::thread::sleep(Duration::from_micros(us as u64))
    }
}

impl embedded_hal::blocking::delay::DelayUs<u64> for ThreadDelay {
    fn delay_us(&mut self, us: u64) {
        std::thread::sleep(Duration::from_micros(us))
    }
}

impl embedded_hal::blocking::delay::DelayMs<u32> for ThreadDelay {
    fn delay_ms(&mut self, us: u32) {
        std::thread::sleep(Duration::from_millis(us as u64))
    }
}

impl embedded_hal::blocking::delay::DelayMs<u64> for ThreadDelay {
    fn delay_ms(&mut self, us: u64) {
        std::thread::sleep(Duration::from_millis(us))
    }
}
