use std::collections::HashMap;

#[derive(Default)]
pub struct Storage {
    data: HashMap<String, Vec<u8>>,
}

impl Storage {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: String, value: Vec<u8>) {
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&Vec<u8>> {
        self.data.get(key)
    }

    pub fn remove(&mut self, key: &str) -> Option<Vec<u8>> {
        self.data.remove(key)
    }
}
