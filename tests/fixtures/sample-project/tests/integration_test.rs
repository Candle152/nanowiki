use sample_project::cli::commands;

#[test]
fn validates_valid_urls() {
    assert!(commands::validate_url("https://example.com").is_ok());
    assert!(commands::validate_url("http://localhost:8080").is_ok());
}

#[test]
fn rejects_invalid_urls() {
    assert!(commands::validate_url("ftp://bad.com").is_err());
    assert!(commands::validate_url("not-a-url").is_err());
}

#[test]
fn validates_cache_actions() {
    assert!(commands::validate_cache_action("list").is_ok());
    assert!(commands::validate_cache_action("clear").is_ok());
    assert!(commands::validate_cache_action("prune").is_ok());
    assert!(commands::validate_cache_action("delete").is_err());
}
