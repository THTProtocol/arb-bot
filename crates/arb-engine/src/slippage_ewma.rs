use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct SlippageEwma {
    alpha: f64,
    values: HashMap<(String, String), f64>,
}

impl SlippageEwma {
    pub fn new(alpha: f64) -> Self {
        Self {
            alpha,
            values: HashMap::new(),
        }
    }

    pub fn update(&mut self, key: (String, String), realized_bps: f64, expected_bps: f64) {
        let delta = (realized_bps - expected_bps).max(0.0);
        let entry = self.values.entry(key).or_insert(delta);
        *entry = self.alpha * delta + (1.0 - self.alpha) * *entry;
    }

    pub fn get(&self, key: &(String, String)) -> f64 {
        self.values.get(key).copied().unwrap_or(0.0)
    }
}
