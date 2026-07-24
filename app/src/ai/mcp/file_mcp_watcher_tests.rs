use std::collections::HashMap;
use std::env;

use super::substitute_env_vars;

fn cleanup_env_vars(vars: &[&str]) {
    for var in vars {
        env::remove_var(var);
    }
}

#[test]
fn test_substitute_env_vars_success() {
    let test_vars = ["FOO", "BAZ", "REPEATED"];
    let secrets = HashMap::new();

    env::set_var("FOO", "bar");
    env::set_var("BAZ", "qux");
    env::set_var("REPEATED", "value");

    let input = r#"{"key": "${FOO}"}"#;
    let result =
        substitute_env_vars(input, &secrets).expect("Single variable substitution should succeed");
    assert_eq!(result, r#"{"key": "bar"}"#);

    let input = r#"{"key": "${FOO}", "other": "${BAZ}"}"#;
    let result = substitute_env_vars(input, &secrets)
        .expect("Multiple variable substitution should succeed");
    assert_eq!(result, r#"{"key": "bar", "other": "qux"}"#);

    let input = r#"{"a": "${REPEATED}", "b": "${REPEATED}", "c": "prefix_${REPEATED}_suffix"}"#;
    let result = substitute_env_vars(input, &secrets)
        .expect("Repeated variable substitution should succeed");
    assert_eq!(
        result,
        r#"{"a": "value", "b": "value", "c": "prefix_value_suffix"}"#
    );

    cleanup_env_vars(&test_vars);
}

#[test]
fn test_substitute_env_vars_missing_or_empty() {
    env::remove_var("MISSING_VAR");
    let secrets = HashMap::new();

    let input = r#"{"key": "${MISSING_VAR}"}"#;
    let err_msg = substitute_env_vars(input, &secrets)
        .unwrap_err()
        .to_string();
    assert!(
        err_msg.contains("MISSING_VAR"),
        "Error message should mention MISSING_VAR, got: {err_msg}"
    );

    env::set_var("EMPTY_VAR", "");
    let input = r#"{"key": "${EMPTY_VAR}"}"#;
    let err_msg = substitute_env_vars(input, &secrets)
        .unwrap_err()
        .to_string();
    assert!(
        err_msg.contains("EMPTY_VAR"),
        "Error message should mention EMPTY_VAR, got: {err_msg}"
    );

    cleanup_env_vars(&["EMPTY_VAR"]);
}

#[test]
fn test_substitute_env_vars_from_secure_storage_secrets() {
    env::remove_var("FROM_STORAGE");
    let secrets = HashMap::from([("FROM_STORAGE".to_owned(), "stored-secret".to_owned())]);
    let input = r#"{"key": "${FROM_STORAGE}"}"#;
    let result = substitute_env_vars(input, &secrets).unwrap();
    assert_eq!(result, r#"{"key": "stored-secret"}"#);
}
