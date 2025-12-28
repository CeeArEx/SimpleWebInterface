use serde::{Deserialize, Serialize};
use web_sys::window;

pub struct LocalStorage;

impl LocalStorage {
    pub fn get<T: for<'de> Deserialize<'de>>(key: &str) -> Option<T> {
        let window = window()?;
        let storage = window.local_storage().ok()??;
        let json = storage.get_item(key).ok()??;
        serde_json::from_str(&json).ok()
    }

    pub fn set<T: Serialize>(key: &str, value: &T) {
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(json) = serde_json::to_string(value) {
                    let _ = storage.set_item(key, &json);
                }
            }
        }
    }
}