//! Turns a value that serde can write into a Godot `Variant`.

use godot::prelude::*;
use serde::Serialize;
use serde_json::Value;

/// Turns a JSON value into a `Variant`. An object becomes a `Dictionary`,
/// and an array becomes an `Array`.
pub fn from_json(value: &Value) -> Variant {
    match value {
        Value::Null => Variant::nil(),
        Value::Bool(b) => b.to_variant(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_variant(),
            None => n.as_f64().unwrap_or(0.0).to_variant(),
        },
        Value::String(s) => GString::from(s).to_variant(),
        Value::Array(items) => {
            let mut a = Array::<Variant>::new();
            for item in items {
                a.push(&from_json(item));
            }
            a.to_variant()
        }
        Value::Object(map) => {
            let mut d = Dictionary::<Variant, Variant>::new();
            for (k, v) in map {
                d.set(k.as_str(), &from_json(v));
            }
            d.to_variant()
        }
    }
}

/// Turns a value into a `Variant` through JSON.
pub fn to_variant<T: Serialize + ?Sized>(value: &T) -> Variant {
    match serde_json::to_value(value) {
        Ok(v) => from_json(&v),
        Err(_) => Variant::nil(),
    }
}
