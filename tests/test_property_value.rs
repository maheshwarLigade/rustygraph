use rusty_graph::prelude::*;
use std::collections::{BTreeMap, HashMap};

#[test]
fn test_property_value_variants_and_constructors() {
    let null_v = PropertyValue::Null;
    assert!(null_v.is_null());
    assert_eq!(null_v.type_name(), "Null");

    let bool_v = PropertyValue::from(true);
    assert_eq!(bool_v.as_bool(), Some(true));
    assert_eq!(bool_v.type_name(), "Bool");

    let int_v = PropertyValue::from(42i64);
    assert_eq!(int_v.as_i64(), Some(42));
    assert_eq!(int_v.as_f64(), Some(42.0));
    assert_eq!(int_v.type_name(), "Int");

    let float_v = PropertyValue::from(3.14159f64);
    assert_eq!(float_v.as_f64(), Some(3.14159));
    assert_eq!(float_v.as_i64(), None); // fractional float cannot be converted to i64
    assert_eq!(float_v.type_name(), "Float");

    let whole_float = PropertyValue::from(100.0f64);
    assert_eq!(whole_float.as_i64(), Some(100)); // whole float can be converted to i64

    let str_v = PropertyValue::from("hello world");
    assert_eq!(str_v.as_str(), Some("hello world"));
    assert_eq!(str_v.type_name(), "String");

    let bytes_v = PropertyValue::from(vec![1u8, 2, 3, 4]);
    assert_eq!(bytes_v.as_bytes(), Some(&[1u8, 2, 3, 4][..]));
    assert_eq!(bytes_v.type_name(), "Bytes");

    let list_v = PropertyValue::from(vec![1, 2, 3]);
    assert_eq!(list_v.as_list().unwrap().len(), 3);
    assert_eq!(list_v.type_name(), "List");

    let mut map = HashMap::new();
    map.insert("key1", PropertyValue::from(10));
    map.insert("key2", PropertyValue::from("val"));
    let map_v = PropertyValue::from(map);
    assert_eq!(map_v.as_map().unwrap().len(), 2);
    assert_eq!(map_v.type_name(), "Map");
}

#[test]
fn test_property_value_total_ordering_and_equality() {
    let int_42 = PropertyValue::from(42);
    let float_42 = PropertyValue::from(42.0);
    assert_eq!(int_42, float_42);
    assert_eq!(int_42.cmp(&float_42), std::cmp::Ordering::Equal);

    let int_10 = PropertyValue::from(10);
    let int_20 = PropertyValue::from(20);
    assert!(int_10 < int_20);

    // Heterogeneous variant ordering: Null < Bool < Number < String
    let null_v = PropertyValue::Null;
    let bool_v = PropertyValue::from(false);
    let str_v = PropertyValue::from("abc");
    assert!(null_v < bool_v);
    assert!(bool_v < int_10);
    assert!(int_20 < str_v);

    // Floating point specials ordering
    let nan_v = PropertyValue::from(f64::NAN);
    let inf_v = PropertyValue::from(f64::INFINITY);
    let neg_inf_v = PropertyValue::from(f64::NEG_INFINITY);
    assert!(neg_inf_v < int_10);
    assert!(int_10 < inf_v);
    assert!(inf_v < nan_v);
}

#[test]
fn test_property_value_hash_consistency() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(PropertyValue::from(100));
    set.insert(PropertyValue::from("Rust"));
    set.insert(PropertyValue::from(true));

    assert!(set.contains(&PropertyValue::from(100)));
    assert!(set.contains(&PropertyValue::from("Rust")));
    assert!(set.contains(&PropertyValue::from(true)));
    assert!(!set.contains(&PropertyValue::from(101)));
}

#[test]
fn test_property_value_try_from_conversions() {
    let int_val = PropertyValue::from(99i64);
    let as_i64: Result<i64> = int_val.clone().try_into();
    assert_eq!(as_i64.unwrap(), 99);

    let as_str: Result<String> = int_val.try_into();
    assert!(as_str.is_err());
    match as_str {
        Err(GraphError::TypeMismatch { expected, found }) => {
            assert_eq!(expected, "String");
            assert_eq!(found, "Int");
        }
        _ => panic!("Expected TypeMismatch error"),
    }
}

#[test]
fn test_property_value_json_serde_round_trip() {
    // Nested structure: Map with String, List, Int, Bool
    let mut inner_map = BTreeMap::new();
    inner_map.insert("name".to_string(), PropertyValue::from("Rusty"));
    inner_map.insert("scores".to_string(), PropertyValue::from(vec![95, 98, 100]));
    inner_map.insert("active".to_string(), PropertyValue::from(true));
    let original = PropertyValue::Map(inner_map);

    let json_str = serde_json::to_string(&original).expect("serialize JSON");
    let deserialized: PropertyValue = serde_json::from_str(&json_str).expect("deserialize JSON");

    assert_eq!(original, deserialized);
}

#[test]
fn test_property_value_display_formatting() {
    assert_eq!(format!("{}", PropertyValue::Null), "null");
    assert_eq!(format!("{}", PropertyValue::from(true)), "true");
    assert_eq!(format!("{}", PropertyValue::from(123)), "123");
    assert_eq!(format!("{}", PropertyValue::from("graph")), "\"graph\"");
    assert_eq!(format!("{}", PropertyValue::from(vec![1, 2])), "[1, 2]");
}
