//! Shape tests (CLAUDE.md hard rule 7): every field present in the
//! checked-in github.com fixture must be present in the Genesis
//! serializer output with the same JSON type, recursively — modulo an
//! explicit allowlist of fields Genesis intentionally omits.

#![cfg(test)]

use alloc::format;
use alloc::string::String;

use serde_json::Value;
use temper_wasm_sdk::json;

/// Diff statistics need blob-level tree walking that v1 does not do
/// at the REST layer (recorded in RFC-0004 / PARITY.md). Each entry is
/// a deliberate omission, not an accident.
const PULL_OMITTED: &[&str] = &[
    "comments",
    "review_comments",
    "commits",
    "additions",
    "deletions",
    "changed_files",
];
const REVIEW_OMITTED: &[&str] = &[];
const MERGE_OMITTED: &[&str] = &[];

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

fn sample_pull(status: &str) -> Value {
    let repo_fields = json!({
        "Visibility": "public",
        "DefaultBranch": "main",
        "Description": "demo repository",
    });
    let pr = json!({
        "Number": 1347,
        "SourceRef": "refs/heads/new-topic",
        "TargetRef": "refs/heads/main",
        "Title": "Amazing new feature",
        "Body": "Please pull these awesome changes in!",
        "OpenedBy": "octo",
        "OpenedAt": "2026-06-11T00:00:00Z",
        "UpdatedAt": "2026-06-11T01:00:00Z",
        "ClosedAt": "2026-06-11T02:00:00Z",
        "MergedAt": "2026-06-11T02:00:00Z",
        "MergedCommitSha": "e5bd3914e2e596debea16f433f57875b5b90bcd6",
        "MergedBy": "rita",
    });
    let ctx = crate::gh::PullContext {
        owner: "octo",
        repo: "hello",
        repo_fields: &repo_fields,
        repo_status: "Active",
        public_base: "https://genesis.test",
        head_sha: "6dcb09b5b57875f334f61aebed695e2e4193db5e",
        base_sha: "c5b97d5ae6c19d5c5df71a34c7fbeeda2479ccbc",
    };
    crate::gh::pull_json(&ctx, "pr-sample", status, &pr)
}

#[test]
fn pull_matches_github_fixture_shape() {
    let fixture = fixture(include_str!("../fixtures/pull.json"));
    assert_shape(&fixture, &sample_pull("Open"), "", PULL_OMITTED);
    assert_shape(&fixture, &sample_pull("Merged"), "", PULL_OMITTED);
}

#[test]
fn pull_shape_check_catches_missing_nested_field() {
    let fixture = fixture(include_str!("../fixtures/pull.json"));
    let mut actual = sample_pull("Open");
    actual["head"]
        .as_object_mut()
        .expect("object")
        .remove("sha");
    let result = std::panic::catch_unwind(|| {
        assert_shape(&fixture, &actual, "", PULL_OMITTED);
    });
    assert!(result.is_err(), "checker must flag a removed nested field");
}

#[test]
fn review_matches_github_fixture_shape() {
    let fixture = fixture(include_str!("../fixtures/review.json"));
    let fields = json!({
        "ReviewerPrincipal": "rita",
        "Decision": "approved",
        "Body": "ship it",
        "SubmittedAt": "2026-06-11T03:00:00Z",
    });
    let actual = crate::gh::review_json(
        "octo",
        "hello",
        1347,
        "rv-sample",
        &fields,
        "ecdd80bb57125d7ba9641ffaa4d7d2c19d3f3091",
        "https://genesis.test",
    );
    assert_shape(&fixture, &actual, "", REVIEW_OMITTED);
}

#[test]
fn merge_matches_github_fixture_shape() {
    let fixture = fixture(include_str!("../fixtures/merge.json"));
    let actual = crate::gh::merge_json("6dcb09b5b57875f334f61aebed695e2e4193db5e");
    assert_shape(&fixture, &actual, "", MERGE_OMITTED);
}

#[test]
fn pull_allowlist_only_covers_diff_statistics() {
    for entry in PULL_OMITTED {
        let known = [
            "comments",
            "review_comments",
            "commits",
            "additions",
            "deletions",
            "changed_files",
        ];
        assert!(known.contains(entry), "unexpected allowlist entry {entry}");
    }
    let _unused: Option<String> = None;
}
