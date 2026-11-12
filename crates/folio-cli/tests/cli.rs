use std::process::Command;
use tempfile::tempdir;

fn folio() -> Command {
    Command::new(env!("CARGO_BIN_EXE_folio"))
}

#[test]
fn renders_html_to_stdout() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("note.md");
    std::fs::write(&file, "# Hello\n\nA **quiet** note.\n").unwrap();

    let output = folio().arg("render").arg(&file).output().unwrap();
    assert!(output.status.success(), "{:?}", output);
    let html = String::from_utf8(output.stdout).unwrap();
    assert!(html.contains("id=\"hello\""));
    assert!(html.contains("<strong>quiet</strong>"));
    assert!(html.ends_with('\n'));
}

#[test]
fn json_output_contains_meta_and_html() {
    let dir = tempdir().unwrap();
    let file = dir.path().join("note.md");
    std::fs::write(&file, "---\ntitle: Note\n---\n# Visible\n\nWords here.\n").unwrap();

    let output = folio()
        .args(["render", file.to_str().unwrap(), "--json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["meta"]["title"], "Note");
    assert_eq!(value["meta"]["stats"]["words"], 3);
    assert!(value["html"].as_str().unwrap().contains("Visible"));
}

#[test]
fn writes_out_file() {
    let dir = tempdir().unwrap();
    let input = dir.path().join("note.md");
    let out = dir.path().join("note.html");
    std::fs::write(&input, "# Hi\n").unwrap();

    let status = folio()
        .args([
            "render",
            input.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(status.success());
    assert!(std::fs::read_to_string(&out).unwrap().contains("id=\"hi\""));
}

#[test]
fn reads_stdin_with_dash() {
    let mut child = folio()
        .args(["render", "-"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write;
        let stdin = child.stdin.as_mut().unwrap();
        stdin.write_all(b"# From stdin\n").unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .contains("from-stdin"));
}

#[test]
fn missing_input_is_a_usage_error() {
    let output = folio().arg("render").output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("missing input"));
}

#[test]
fn unknown_option_is_a_usage_error() {
    let output = folio().args(["render", "x.md", "--wat"]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("unknown option"));
}

#[test]
fn missing_file_is_reported() {
    let output = folio()
        .args(["render", "/definitely/not/here.md"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("cannot read"));
}

#[test]
fn themes_list_contains_builtins() {
    let output = folio().args(["themes", "list"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("blend"));
    assert!(stdout.contains("Blend Dark"));
    assert!(stdout.contains("editorial"));
}

#[test]
fn scaffold_prints_a_valid_theme() {
    let output = folio()
        .args(["themes", "scaffold", "quiet-paper"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let theme: folio_core::Theme = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(theme.id, "quiet-paper");
    theme.validate().unwrap();
}

#[test]
fn version_is_printed() {
    let output = folio().arg("--version").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .unwrap()
        .starts_with("folio "));
}
