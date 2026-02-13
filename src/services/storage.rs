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

    pub fn set<T: Serialize + ?Sized>(key: &str, value: &T) {
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                if let Ok(json) = serde_json::to_string(value) {
                    let _ = storage.set_item(key, &json);
                }
            }
        }
    }

    pub fn remove(key: &str) {
        if let Some(window) = window() {
            if let Ok(Some(storage)) = window.local_storage() {
                let _ = storage.remove_item(key);
            }
        }
    }

    pub fn get_vec<T: for<'de> Deserialize<'de> + Default + serde::Serialize>(key: &str) -> Vec<T> {
        LocalStorage::get(key).unwrap_or_default()
    }

    pub fn set_vec<T: Serialize>(key: &str, value: &[T]) {
        LocalStorage::set(key, value);
    }

    pub fn push_vec<T: Serialize + Clone + for<'de> Deserialize<'de> + Default>(key: &str, item: &T) -> Vec<T> {
        let mut vec: Vec<T> = LocalStorage::get_vec(key);
        vec.push(item.clone());
        LocalStorage::set(key, &vec);
        vec
    }

    pub fn remove_from_vec<T: PartialEq + Serialize + Clone + for<'de> Deserialize<'de> + Default>(key: &str, item: &T) -> Vec<T> {
        let mut vec: Vec<T> = LocalStorage::get_vec(key);
        vec.retain(|x| x != item);
        LocalStorage::set(key, &vec);
        vec
    }
}
