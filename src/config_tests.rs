use super::*;

#[test]
fn history_path_ends_with_history_json() {
    let path = Config::history_path();
    assert_eq!(path.file_name().unwrap(), "history.json");
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
