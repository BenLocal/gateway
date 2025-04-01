use pingora_limits::rate::Rate;

pub struct RateLimiter {
    limits: Rate,
    max_req_per_second: u32,
}

impl RateLimiter {
    pub fn new(limits: Rate, max_req_per_second: u32) -> Self {
        Self {
            limits,
            max_req_per_second,
        }
    }

    pub fn increase(&self, key: &str) -> isize {
        self.limits.observe(&key, 1)
    }

    pub fn max_req_per_second(&self) -> isize {
        self.max_req_per_second as isize
    }

    pub fn rate(&self, key: &str) -> f64 {
        self.limits.rate(&key)
    }
}
