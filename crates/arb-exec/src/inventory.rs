use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Inventory {
    pub inner: HashMap<String, f64>,
}

impl Inventory {
    pub fn get(&self, asset: &str) -> f64 {
        self.inner.get(asset).copied().unwrap_or(0.0)
    }
    pub fn update(&mut self, asset: impl Into<String>, delta: f64) {
        let key = asset.into();
        let current = self.inner.get(&key).copied().unwrap_or(0.0);
        self.inner.insert(key, current + delta);
    }
}
