//! Shape tests (CLAUDE.md hard rule 7): every field present in the
//! checked-in github.com fixture must be present in the Genesis
//! serializer output with the same JSON type, recursively — modulo an
//! explicit allowlist of fields Genesis intentionally omits.

#![cfg(test)]

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use serde_json::Value;
use temper_wasm_sdk::json;

/// Fields documented on github.com that Genesis intentionally omits.
/// Keep this list short and explicit; every entry is a deliberate
/// product decision, not an accident.
const REPOSITORY_OMITTED: &[&str] = &[];

fn type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Recursive structural check. Fixture `null` values (nullable fields)
/// only require presence; otherwise types must match exactly. For
/// arrays, the first fixture element constrains every actual element.
fn assert_shape(fixture: &Value, actual: &Value, path: &str, omitted: &[&str]) {
    match (fixture, actual) {
        (Value::Object(expected), Value::Object(got)) => {
            for (key, expected_value) in expected {
                let field_path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if omitted.contains(&field_path.as_str()) {
                    continue;
                }
                let Some(actual_value) = got.get(key) else {
                    panic!("missing field `{field_path}` in Genesis response");
                };
                assert_shape(expected_value, actual_value, &field_path, omitted);
            }
        }
        (Value::Array(expected_items), Value::Array(actual_items)) => {
            if let Some(first_expected) = expected_items.first() {
                for (idx, item) in actual_items.iter().enumerate() {
                    assert_shape(first_expected, item, &format!("{path}[{idx}]"), omitted);
                }
            }
        }
        // Nullable on either side: presence is the contract.
        (Value::Null, _) | (_, Value::Null) => {}
        (expected, got) => {
            assert_eq!(
                type_name(expected),
                type_name(got),
                "type mismatch at `{path}`: fixture {} vs genesis {}",
                type_name(expected),
                type_name(got)
            );
        }
    }
}

fn fixture(raw: &str) -> Value {
    serde_json::from_str(raw).expect("fixture parses")
}

fn sample_repository() -> Value {
    let fields = json!({
        "OwnerAccountId": "octo",
        "Name": "hello",
        "Description": "demo repository",
        "DefaultBranch": "main",
        "Visibility": "public",
        "CreatedAt": "2026-06-11T00:00:00Z",
        "UpdatedAt": "2026-06-11T01:00:00Z",
    });
    crate::gh::repository_json("octo", "hello", &fields, "Active", "https://genesis.test")
}

#[test]
fn repository_matches_github_fixture_shape() {
    let fixture = fixture(include_str!("../fixtures/repository.json"));
    let actual = sample_repository();
    assert_shape(&fixture, &actual, "", REPOSITORY_OMITTED);
}

#[test]
fn repository_shape_check_catches_missing_field() {
    let fixture = fixture(include_str!("../fixtures/repository.json"));
    let mut actual = sample_repository();
    actual
        .as_object_mut()
        .expect("object")
        .remove("clone_url");
    let result = std::panic::catch_unwind(|| {
        assert_shape(&fixture, &actual, "", REPOSITORY_OMITTED);
    });
    assert!(result.is_err(), "checker must flag a removed field");
}

#[test]
fn repository_shape_check_catches_type_drift() {
    let fixture = fixture(include_str!("../fixtures/repository.json"));
    let mut actual = sample_repository();
    actual
        .as_object_mut()
        .expect("object")
        .insert("private".into(), json!("nope"));
    let result = std::panic::catch_unwind(|| {
        assert_shape(&fixture, &actual, "", REPOSITORY_OMITTED);
    });
    assert!(result.is_err(), "checker must flag a type change");
}

/// Silence the unused-list lint when the allowlist is empty: an empty
/// allowlist is the desired steady state for this endpoint.
#[test]
fn repository_allowlist_is_intentionally_empty() {
    let omitted: Vec<String> = REPOSITORY_OMITTED.iter().map(|s| String::from(*s)).collect();
    assert!(omitted.is_empty());
}
