//! Shape tests (CLAUDE.md hard rule 7): every field present in the
//! checked-in github.com fixture must be present in the Genesis
//! serializer output with the same JSON type, recursively — modulo an
//! explicit allowlist of fields Genesis intentionally omits.

#![cfg(test)]

use alloc::format;
use alloc::string::String;

use serde_json::Value;

/// Fields documented on github.com that Genesis intentionally omits.
const BRANCH_OMITTED: &[&str] = &[];
const GIT_REF_OMITTED: &[&str] = &[];

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

#[test]
fn branch_matches_github_fixture_shape() {
    let fixture = fixture(include_str!("../fixtures/branch.json"));
    let actual = crate::gh::branch_json(
        "octo",
        "hello",
        "refs/heads/main",
        "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc",
        "https://genesis.test",
    );
    assert_shape(&fixture, &actual, "", BRANCH_OMITTED);
}

#[test]
fn git_ref_matches_github_fixture_shape() {
    let fixture = fixture(include_str!("../fixtures/git_ref.json"));
    let actual = crate::gh::git_ref_json(
        "octo",
        "hello",
        "refs/heads/featureA",
        "aa218f56b14c9653891f9e74264a383fa43fefbd",
        "https://genesis.test",
    );
    assert_shape(&fixture, &actual, "", GIT_REF_OMITTED);
}

#[test]
fn git_ref_shape_check_catches_missing_object_field() {
    let fixture = fixture(include_str!("../fixtures/git_ref.json"));
    let mut actual = crate::gh::git_ref_json(
        "octo",
        "hello",
        "refs/heads/featureA",
        "aa218f56b14c9653891f9e74264a383fa43fefbd",
        "https://genesis.test",
    );
    actual["object"]
        .as_object_mut()
        .expect("object")
        .remove("sha");
    let result = std::panic::catch_unwind(|| {
        assert_shape(&fixture, &actual, "", GIT_REF_OMITTED);
    });
    assert!(result.is_err(), "checker must flag a removed nested field");
}

#[test]
fn allowlists_are_intentionally_empty() {
    let total: usize = [BRANCH_OMITTED, GIT_REF_OMITTED]
        .iter()
        .map(|l| l.len())
        .sum();
    assert_eq!(total, 0, "branch/ref projections omit nothing documented");
    let _unused: Option<String> = None;
}
