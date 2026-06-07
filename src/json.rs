use serde_json::Value;
use crate::error::Error;

pub fn get_string<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    let mut current = v;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_str()
}

pub fn get_string_required<'a>(v: &'a Value, path: &[&str]) -> Result<&'a str, Error> {
    get_string(v, path).ok_or_else(|| Error::Json(format!("missing field: {}", path.join("."))))
}

pub fn get_array<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Vec<Value>> {
    let mut current = v;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_array()
}

pub fn get_array_required<'a>(v: &'a Value, path: &[&str]) -> Result<&'a Vec<Value>, Error> {
    get_array(v, path).ok_or_else(|| Error::Json(format!("missing array: {}", path.join("."))))
}

pub fn get_object<'a>(v: &'a Value, path: &[&str]) -> Option<&'a serde_json::Map<String, Value>> {
    let mut current = v;
    for key in path {
        current = current.get(*key)?;
    }
    current.as_object()
}

pub fn get_object_required<'a>(v: &'a Value, path: &[&str]) -> Result<&'a serde_json::Map<String, Value>, Error> {
    get_object(v, path).ok_or_else(|| Error::Json(format!("missing object: {}", path.join("."))))
}

pub fn get_value<'a>(v: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = v;
    for key in path {
        current = current.get(*key)?;
    }
    Some(current)
}

pub fn get_value_required<'a>(v: &'a Value, path: &[&str]) -> Result<&'a Value, Error> {
    get_value(v, path).ok_or_else(|| Error::Json(format!("missing value: {}", path.join("."))))
}
