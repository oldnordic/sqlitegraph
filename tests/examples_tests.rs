use std::process::Command;

fn run_example(name: &str) -> String {
    let output = Command::new("cargo")
        .args(["run", "--example", name])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env(
            "CARGO_HOME",
            format!("{}/.cargo", env!("CARGO_MANIFEST_DIR")),
        )
        .output()
        .expect("run example");
    assert!(
        output.status.success(),
        "example {name} failed: {:?}",
        output
    );
    String::from_utf8(output.stdout).expect("utf8")
}

#[test]
fn test_basic_usage_example_outputs_expected_sections() {
    let stdout = run_example("basic_usage");
    assert!(stdout.contains("neighbors:"));
    assert!(stdout.contains("pattern matches:"));
    assert!(stdout.contains("reasoning score"));
}

#[test]
fn test_migration_flow_example_reports_shadow_read() {
    let stdout = run_example("migration_flow");
    assert!(stdout.contains("shadow_read"));
    assert!(stdout.contains("cutover active"));
}
