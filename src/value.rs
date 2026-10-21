use crate::error::{GraphError, Result};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};

/// Represents any property value supported by RustyGraph, mirroring Neo4j's type system.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropertyValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    DateTime(i64), // Milliseconds since Unix epoch
    List(Vec<PropertyValue>),
    Map(BTreeMap<String, PropertyValue>),
    Bytes(Vec<u8>),
}

impl PropertyValue {
    /// Returns true if this value is Null.
    #[inline]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    /// Attempts to extract an `i64` from this value (converts Int directly, or Float if whole).
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(i) => Some(*i),
            Self::Float(f) if f.fract() == 0.0 && *f >= i64::MIN as f64 && *f <= i64::MAX as f64 => {
                Some(*f as i64)
            }
            _ => None,
        }
    }

    /// Attempts to extract an `f64` from this value (converts Int to Float if needed).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(f) => Some(*f),
            Self::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Attempts to extract a `bool` from this value.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Attempts to extract a string slice from this value.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Attempts to extract byte slice from this value.
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(b) => Some(b.as_slice()),
            _ => None,
        }
    }

    /// Attempts to extract a list of PropertyValues.
    pub fn as_list(&self) -> Option<&[PropertyValue]> {
        match self {
            Self::List(l) => Some(l.as_slice()),
            _ => None,
        }
    }

    /// Attempts to extract a vector of f32 numbers from this value (converts from List of numeric values).
    pub fn as_vector(&self) -> Option<Vec<f32>> {
        match self {
            Self::List(l) => {
                let mut vec = Vec::with_capacity(l.len());
                for item in l {
                    if let Some(f) = item.as_f64() {
                        vec.push(f as f32);
                    } else {
                        return None;
                    }
                }
                Some(vec)
            }
            _ => None,
        }
    }

    /// Attempts to extract a map of string keys to PropertyValues.
    pub fn as_map(&self) -> Option<&BTreeMap<String, PropertyValue>> {
        match self {
            Self::Map(m) => Some(m),
            _ => None,
        }
    }

    /// Returns human-readable type name for error reporting.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "Null",
            Self::Bool(_) => "Bool",
            Self::Int(_) => "Int",
            Self::Float(_) => "Float",
            Self::String(_) => "String",
            Self::Bytes(_) => "Bytes",
            Self::DateTime(_) => "DateTime",
            Self::List(_) => "List",
            Self::Map(_) => "Map",
        }
    }

    /// Navigate a nested dot-separated path (e.g. "address.city" or "tags.0") inside a Map or List.
    pub fn get_path(&self, path: &str) -> Option<&PropertyValue> {
        let mut curr = self;
        for segment in path.split('.') {
            if segment.is_empty() {
                continue;
            }
            match curr {
                Self::Map(m) => {
                    curr = m.get(segment)?;
                }
                Self::List(l) => {
                    let idx = segment.parse::<usize>().ok()?;
                    curr = l.get(idx)?;
                }
                _ => return None,
            }
        }
        Some(curr)
    }

    /// Mutate or insert a nested dot-separated path (e.g. "address.city") inside a Map or List.
    pub fn set_path(&mut self, path: &str, val: PropertyValue) {
        let parts: Vec<&str> = path.split('.').filter(|s| !s.is_empty()).collect();
        if parts.is_empty() {
            *self = val;
            return;
        }

        let mut curr = self;
        for i in 0..(parts.len() - 1) {
            let seg = parts[i];
            if !matches!(curr, Self::Map(_)) {
                *curr = Self::Map(BTreeMap::new());
            }
            if let Self::Map(m) = curr {
                if !m.contains_key(seg) {
                    m.insert(seg.to_string(), Self::Map(BTreeMap::new()));
                }
                curr = m.get_mut(seg).unwrap();
            }
        }

        let last = parts[parts.len() - 1];
        if !matches!(curr, Self::Map(_)) {
            *curr = Self::Map(BTreeMap::new());
        }
        if let Self::Map(m) = curr {
            m.insert(last.to_string(), val);
        }
    }
}

// Equality implementation with canonical total ordering for floats
impl PartialEq for PropertyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Null, Self::Null) => true,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Int(a), Self::Int(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b) == Ordering::Equal,
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b) == Ordering::Equal,
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)) == Ordering::Equal,
            (Self::String(a), Self::String(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::DateTime(a), Self::DateTime(b)) => a == b,
            (Self::List(a), Self::List(b)) => a == b,
            (Self::Map(a), Self::Map(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for PropertyValue {}

impl PartialOrd for PropertyValue {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PropertyValue {
    fn cmp(&self, other: &Self) -> Ordering {
        // Tag ordering helper
        fn tag(v: &PropertyValue) -> u8 {
            match v {
                PropertyValue::Null => 0,
                PropertyValue::Bool(_) => 1,
                PropertyValue::Int(_) | PropertyValue::Float(_) => 2,
                PropertyValue::String(_) => 3,
                PropertyValue::Bytes(_) => 4,
                PropertyValue::DateTime(_) => 5,
                PropertyValue::List(_) => 6,
                PropertyValue::Map(_) => 7,
            }
        }

        let tag_a = tag(self);
        let tag_b = tag(other);

        if tag_a != tag_b {
            return tag_a.cmp(&tag_b);
        }

        match (self, other) {
            (Self::Null, Self::Null) => Ordering::Equal,
            (Self::Bool(a), Self::Bool(b)) => a.cmp(b),
            (Self::Int(a), Self::Int(b)) => a.cmp(b),
            (Self::Float(a), Self::Float(b)) => a.total_cmp(b),
            (Self::Int(a), Self::Float(b)) => (*a as f64).total_cmp(b),
            (Self::Float(a), Self::Int(b)) => a.total_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => a.cmp(b),
            (Self::Bytes(a), Self::Bytes(b)) => a.cmp(b),
            (Self::DateTime(a), Self::DateTime(b)) => a.cmp(b),
            (Self::List(a), Self::List(b)) => a.cmp(b),
            (Self::Map(a), Self::Map(b)) => a.cmp(b),
            _ => unreachable!(),
        }
    }
}

impl Hash for PropertyValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Null => (),
            Self::Bool(b) => b.hash(state),
            Self::Int(i) => i.hash(state),
            Self::Float(f) => f.to_bits().hash(state),
            Self::String(s) => s.hash(state),
            Self::Bytes(b) => b.hash(state),
            Self::DateTime(t) => t.hash(state),
            Self::List(l) => l.hash(state),
            Self::Map(m) => m.hash(state),
        }
    }
}

impl fmt::Display for PropertyValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Null => write!(f, "null"),
            Self::Bool(b) => write!(f, "{b}"),
            Self::Int(i) => write!(f, "{i}"),
            Self::Float(v) => write!(f, "{v}"),
            Self::String(s) => write!(f, "\"{s}\""),
            Self::Bytes(b) => write!(f, "<bytes len={}>", b.len()),
            Self::DateTime(t) => write!(f, "DateTime({t})"),
            Self::List(l) => {
                write!(f, "[")?;
                for (i, val) in l.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{val}")?;
                }
                write!(f, "]")
            }
            Self::Map(m) => {
                write!(f, "{{")?;
                for (i, (k, val)) in m.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{k}: {val}")?;
                }
                write!(f, "}}")
            }
        }
    }
}

// Ergonomic From implementations
impl From<bool> for PropertyValue {
    #[inline]
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<i64> for PropertyValue {
    #[inline]
    fn from(v: i64) -> Self {
        Self::Int(v)
    }
}

impl From<i32> for PropertyValue {
    #[inline]
    fn from(v: i32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<u32> for PropertyValue {
    #[inline]
    fn from(v: u32) -> Self {
        Self::Int(v as i64)
    }
}

impl From<u64> for PropertyValue {
    #[inline]
    fn from(v: u64) -> Self {
        Self::Int(v as i64)
    }
}

impl From<usize> for PropertyValue {
    #[inline]
    fn from(v: usize) -> Self {
        Self::Int(v as i64)
    }
}

impl From<f64> for PropertyValue {
    #[inline]
    fn from(v: f64) -> Self {
        Self::Float(v)
    }
}

impl From<f32> for PropertyValue {
    #[inline]
    fn from(v: f32) -> Self {
        Self::Float(v as f64)
    }
}

impl From<&str> for PropertyValue {
    #[inline]
    fn from(v: &str) -> Self {
        Self::String(v.to_string())
    }
}

impl From<String> for PropertyValue {
    #[inline]
    fn from(v: String) -> Self {
        Self::String(v)
    }
}

impl From<Vec<u8>> for PropertyValue {
    #[inline]
    fn from(v: Vec<u8>) -> Self {
        Self::Bytes(v)
    }
}

impl<T: Into<PropertyValue>> From<Vec<T>> for PropertyValue {
    fn from(v: Vec<T>) -> Self {
        Self::List(v.into_iter().map(Into::into).collect())
    }
}

impl<K: Into<String>, V: Into<PropertyValue>> From<std::collections::HashMap<K, V>>
    for PropertyValue
{
    fn from(v: std::collections::HashMap<K, V>) -> Self {
        Self::Map(
            v.into_iter()
                .map(|(k, val)| (k.into(), val.into()))
                .collect(),
        )
    }
}

impl From<serde_json::Value> for PropertyValue {
    fn from(val: serde_json::Value) -> Self {
        match val {
            serde_json::Value::Null => Self::Null,
            serde_json::Value::Bool(b) => Self::Bool(b),
            serde_json::Value::Number(num) => {
                if let Some(i) = num.as_i64() {
                    Self::Int(i)
                } else if let Some(f) = num.as_f64() {
                    Self::Float(f)
                } else {
                    Self::Null
                }
            }
            serde_json::Value::String(s) => Self::String(s),
            serde_json::Value::Array(arr) => {
                Self::List(arr.into_iter().map(PropertyValue::from).collect())
            }
            serde_json::Value::Object(map) => Self::Map(
                map.into_iter()
                    .map(|(k, v)| (k, PropertyValue::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<PropertyValue> for serde_json::Value {
    fn from(val: PropertyValue) -> Self {
        match val {
            PropertyValue::Null => serde_json::Value::Null,
            PropertyValue::Bool(b) => serde_json::Value::Bool(b),
            PropertyValue::Int(i) => serde_json::Value::Number(serde_json::Number::from(i)),
            PropertyValue::Float(f) => serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            PropertyValue::String(s) => serde_json::Value::String(s),
            PropertyValue::Bytes(b) => serde_json::Value::Array(
                b.into_iter()
                    .map(|byte| serde_json::Value::Number(serde_json::Number::from(byte)))
                    .collect(),
            ),
            PropertyValue::DateTime(t) => serde_json::Value::Number(serde_json::Number::from(t)),
            PropertyValue::List(l) => {
                serde_json::Value::Array(l.into_iter().map(serde_json::Value::from).collect())
            }
            PropertyValue::Map(m) => serde_json::Value::Object(
                m.into_iter()
                    .map(|(k, v)| (k, serde_json::Value::from(v)))
                    .collect(),
            ),
        }
    }
}

// TryFrom conversions for strongly typed extraction
impl TryFrom<PropertyValue> for i64 {
    type Error = GraphError;

    fn try_from(val: PropertyValue) -> Result<Self> {
        val.as_i64().ok_or_else(|| GraphError::TypeMismatch {
            expected: "i64",
            found: val.type_name(),
        })
    }
}

impl TryFrom<PropertyValue> for f64 {
    type Error = GraphError;

    fn try_from(val: PropertyValue) -> Result<Self> {
        val.as_f64().ok_or_else(|| GraphError::TypeMismatch {
            expected: "f64",
            found: val.type_name(),
        })
    }
}

impl TryFrom<PropertyValue> for bool {
    type Error = GraphError;

    fn try_from(val: PropertyValue) -> Result<Self> {
        val.as_bool().ok_or_else(|| GraphError::TypeMismatch {
            expected: "bool",
            found: val.type_name(),
        })
    }
}

impl TryFrom<PropertyValue> for String {
    type Error = GraphError;

    fn try_from(val: PropertyValue) -> Result<Self> {
        match val {
            PropertyValue::String(s) => Ok(s),
            other => Err(GraphError::TypeMismatch {
                expected: "String",
                found: other.type_name(),
            }),
        }
    }
}
