use brainstone_agent::ToolError;

#[test]
fn tool_errors_integrate_with_standard_error_consumers() {
    let error = ToolError::new("example_lookup_failed", "lookup failed: café");
    let standard: &dyn std::error::Error = &error;
    assert_eq!(standard.to_string(), "lookup failed: café");
    assert_eq!(error.code, "example_lookup_failed");
    assert!(standard.source().is_none());
}
