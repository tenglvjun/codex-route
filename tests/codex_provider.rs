use codex_route::codex_provider::{extract_active_wire_api, is_responses_wire_api};

#[test]
fn active_provider_wire_api_is_selected_over_inactive_provider() {
    let config = r#"
model_provider = "custom"
wire_api = "chat"
[model_providers.custom]
wire_api = "RESPONSES"
[model_providers.other]
wire_api = "anthropic"
"#;
    assert_eq!(
        extract_active_wire_api(config).as_deref(),
        Some("responses")
    );
    assert!(is_responses_wire_api(config));
}

#[test]
fn missing_wire_api_defaults_to_responses() {
    assert!(is_responses_wire_api("model_provider = \"custom\""));
}

#[test]
fn chat_wire_api_is_not_supported_by_the_route() {
    assert!(!is_responses_wire_api("wire_api = \"chat\""));
}
