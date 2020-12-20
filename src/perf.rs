use std::thread;
use std::time::{Duration, Instant};

pub struct PerfLimiter {
    pub last_check: Instant,
    pub last_fps_check: Instant,
    pub fps_limit: f64,
    pub counter: u64,
    pub last_counter: u64,

    pub every_nths: u64,
    pub nths_counter: u64,
}

impl PerfLimiter {
    pub fn new(fps_limit: Option<f64>) -> Self {
        let fps_limit = match fps_limit {
            Some(fps_limit) => fps_limit,
            None => 0.0,
        };
        let time = Instant::now();
        let every_nths = if fps_limit as u64 >= 100 {
            fps_limit as u64 / 100
        } else {
            1
        };
        Self {
            last_check: time,
            last_fps_check: time,
            fps_limit,
            counter: 0,
            last_counter: 0,
            every_nths,
            nths_counter: 0,
        }
    }

    pub fn get_fps(&mut self) -> f64 {
        let now = Instant::now();
        let fps =
            (self.counter - self.last_counter) as f64 / (now - self.last_fps_check).as_secs_f64();
        self.last_fps_check = now;
        self.last_counter = self.counter;
        fps
    }

    pub fn wait(&mut self) {
        self.counter += 1;

        if self.every_nths > 1 {
            if self.nths_counter < self.every_nths - 1 {
                self.nths_counter += 1;
                return;
            }
            self.nths_counter = 0;
        }

        let now = Instant::now();
        let diff = now - self.last_check;
        let wait = self.every_nths as f64 / self.fps_limit - diff.as_secs_f64();
        if self.fps_limit == 0.0 || wait <= 0.0 {
            self.last_check = now;
            return;
        }

        thread::sleep(Duration::from_secs_f64(wait));
        self.last_check = Instant::now();
    }

    // true means wait, false means time is over
    pub fn wait_nonblocking(&mut self) -> bool {
        let now = Instant::now();
        let diff = now - self.last_check;
        let wait = 1.0 / self.fps_limit - diff.as_secs_f64();
        if self.fps_limit == 0.0 || wait <= 0.0 {
            self.last_check = now;
            self.counter += 1;
            false
        } else {
            true
        }
    }
}
