use super::*;
use serde_json::json;

#[test]
fn connections_use_one_limit_and_reject_ambiguous_or_invalid_input() {
    for (input, expected) in [
        (
            json!({"split": "12", "max-connection-per-server": "8"}),
            "8",
        ),
        (json!({"split": 12}), "12"),
        (json!({"stream-max-connections": "4"}), "4"),
    ] {
        let output = sanitize_aria2_options(input.as_object().unwrap()).unwrap();
        assert_eq!(output["stream-max-connections"], expected);
        assert!(!output.contains_key("split"));
        assert!(!output.contains_key("max-connection-per-server"));
    }
    for value in [
        json!(0),
        json!(65),
        json!(-1),
        json!(1.5),
        json!(true),
        json!(null),
        json!("bad"),
    ] {
        let input = json!({"stream-max-connections": value});
        assert!(sanitize_aria2_options(input.as_object().unwrap())
            .unwrap_err()
            .starts_with("task_connections_invalid"));
    }
    let input = json!({"stream-max-connections": 4, "split": 8});
    assert!(sanitize_aria2_options(input.as_object().unwrap())
        .unwrap_err()
        .starts_with("aria2_option_conflict"));
    let input = json!({"split": "12", "max-connection-per-server": "8"});
    let advanced = CreateTaskAdvancedOptions {
        connections: Some(3),
        ..Default::default()
    };
    assert_eq!(
        sanitize_create_task_options(&advanced, input.as_object().unwrap()).unwrap()
            ["stream-max-connections"],
        "3"
    );
}

#[test]
fn unsupported_and_server_owned_options_are_rejected_before_filtering() {
    for (key, value, code) in [
        ("media", json!("auto"), "aria2_option_unsupported"),
        ("min-split-size", json!(null), "aria2_option_unsupported"),
        ("bt-save-metadata", json!(false), "aria2_option_unsupported"),
        (
            "dht-file-path",
            json!("private"),
            "aria2_option_unsupported",
        ),
        ("state-dir", json!("private"), "aria2_option_forbidden"),
        ("rpc-secret", json!("secret"), "aria2_option_forbidden"),
        ("save-session", json!("session"), "aria2_option_forbidden"),
    ] {
        let input = Map::from_iter([(key.to_string(), value)]);
        let error = sanitize_aria2_options(&input).unwrap_err();
        assert!(error.starts_with(code));
        assert!(!error.contains("private") && !error.ends_with("secret"));
    }
    let input = json!({"media": "file", "unknown": "ignored", "header": ["Accept: */*"], "check-certificate": "true"});
    let result = sanitize_aria2_options(input.as_object().unwrap()).unwrap();
    assert!(!result.contains_key("unknown"));
    assert_eq!(result["header"], json!(["Accept: */*"]));
    assert_eq!(result["check-certificate"], "true");
}
