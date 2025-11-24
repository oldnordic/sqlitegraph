use assert_cmd::Command;

#[test]
fn test_cli_exits_with_success_on_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.arg("--help");
    cmd.assert().success();
}
