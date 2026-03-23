use super::*;

#[test]
fn history_path_ends_with_history_json() {
    let path = Config::history_path();
    assert_eq!(path.file_name().unwrap(), "history.json");
}

#[test]
fn log_path_ends_with_log_txt() {
    let path = Config::log_path();
    assert_eq!(path.file_name().unwrap(), "log.txt");
    assert_eq!(path.parent().and_then(|p| p.file_name()).unwrap(), "logs");
}

#[test]
fn config_path_ends_with_config_toml() {
    let path = Config::config_path();
    assert_eq!(path.file_name().unwrap(), "config.toml");
}

#[test]
fn resolved_app_run_dir_returns_config_value_when_set() {
    let config = Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: Some(String::from("/custom/dir")),
        auto_pull: false,
    };
    assert_eq!(config.resolved_app_run_dir(), "/custom/dir");
}

#[test]
fn resolved_app_run_dir_falls_back_when_not_set() {
    let config = Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: None,
        auto_pull: false,
    };
    let result = config.resolved_app_run_dir();
    assert!(!result.is_empty());
}

#[test]
fn resolved_app_run_dir_empty_string_falls_back() {
    let config = Config {
        owner: String::from("owner"),
        local_base_dir: String::from("/base"),
        app_run_dir: Some(String::new()),
        auto_pull: false,
    };
    let result = config.resolved_app_run_dir();
    assert!(!result.is_empty());
}
