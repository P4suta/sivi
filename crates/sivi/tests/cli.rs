//! The command line as a process: the exit statuses, and which stream carries what.

use assert_cmd::Command;
use std::path::Path;

/// The fixtures, relative to the crate, which is the working directory of a test.
const DECKS: &str = "../../fixtures/decks";
const THEMES: &str = "../../fixtures/themes";
const PLAIN_THEME: &str = "../../themes/plain";

const SUCCESS: i32 = 0;
const DIAGNOSTIC: i32 = 1;
const USAGE: i32 = 2;

fn sivi() -> Command {
    let mut command = Command::new(assert_cmd::cargo::cargo_bin("sivi"));
    command.env_remove("SOURCE_DATE_EPOCH").env("NO_COLOR", "1");
    command
}

fn deck(name: &str) -> String {
    format!("{DECKS}/{name}/deck.md")
}

#[test]
fn a_passing_check_is_silent_and_a_failing_one_names_its_code_on_standard_error() {
    let passed = sivi()
        .args(["check", &deck("pass-basic")])
        .assert()
        .code(SUCCESS);
    let silent = passed.get_output();
    assert_eq!(silent.stdout, Vec::<u8>::new());
    assert_eq!(silent.stderr, Vec::<u8>::new());
    let failed = sivi()
        .args(["check", &deck("fail-overflow-body")])
        .assert()
        .code(DIAGNOSTIC);
    let refused = failed.get_output();
    assert_eq!(refused.stdout, Vec::<u8>::new());
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(stderr.contains("error[E0401]"), "{stderr}");
    assert!(
        stderr.contains("fail-overflow-body/deck.md:9:1"),
        "{stderr}"
    );
}

#[test]
fn an_unreadable_deck_and_a_bad_epoch_are_usage_errors() {
    let missing = sivi()
        .args(["check", &deck("nowhere")])
        .assert()
        .code(USAGE);
    let stderr = String::from_utf8_lossy(&missing.get_output().stderr);
    assert!(stderr.starts_with("error: cannot read"), "{stderr}");
    let epoch = sivi()
        .env("SOURCE_DATE_EPOCH", "soon")
        .args(["check", &deck("pass-basic")])
        .assert()
        .code(USAGE);
    let epoch_stderr = String::from_utf8_lossy(&epoch.get_output().stderr);
    assert!(epoch_stderr.contains("SOURCE_DATE_EPOCH"), "{epoch_stderr}");
}

#[test]
fn the_json_document_goes_to_standard_output_alone() {
    let run = sivi()
        .args(["check", "--format", "json", &deck("fail-capacity")])
        .assert()
        .code(DIAGNOSTIC);
    let output = run.get_output();
    assert_eq!(output.stderr, Vec::<u8>::new());
    let document: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["format"], "sivi.diagnostics");
    assert_eq!(document["diagnostics"][0]["code"], "E0306");
    assert_eq!(document["output"], serde_json::Value::Null);
}

#[test]
fn a_build_publishes_the_pdf_or_streams_it_and_never_both_documents() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("out.pdf");
    let built = sivi()
        .args(["build", &deck("pass-basic"), "-o"])
        .arg(&output)
        .assert()
        .code(SUCCESS);
    assert_eq!(built.get_output().stdout, Vec::<u8>::new());
    let bytes = sivi::io::read(&output).unwrap();
    assert!(bytes.starts_with(b"%PDF-"));
    let streamed = sivi()
        .args(["build", &deck("pass-basic"), "-o", "-"])
        .assert()
        .code(SUCCESS);
    assert!(streamed.get_output().stdout.starts_with(b"%PDF-"));
    let both = sivi()
        .args(["build", &deck("pass-basic"), "-o", "-", "--format", "json"])
        .assert()
        .code(USAGE);
    let stderr = String::from_utf8_lossy(&both.get_output().stderr);
    assert!(stderr.contains("standard output"), "{stderr}");
}

#[test]
fn a_warning_passes_unless_warnings_are_denied() {
    let warned = sivi()
        .args(["check", &deck("warn-head")])
        .assert()
        .code(SUCCESS);
    let stderr = String::from_utf8_lossy(&warned.get_output().stderr);
    assert!(stderr.contains("warning[W0106]"), "{stderr}");
    sivi()
        .args(["check", "--deny-warnings", &deck("warn-head")])
        .assert()
        .code(DIAGNOSTIC);
}

#[test]
fn the_theme_listing_names_every_carried_theme_with_its_masters() {
    let listed = sivi().args(["theme", "list"]).assert().code(SUCCESS);
    let stdout = String::from_utf8_lossy(&listed.get_output().stdout);
    for name in ["plain", "blank", "editorial", "studio"] {
        assert!(stdout.contains(&format!("{name} ")), "{stdout}");
    }
    assert!(
        stdout.contains("masters: title, section, bullets,"),
        "{stdout}"
    );
    let json = sivi()
        .args(["theme", "list", "--format", "json"])
        .assert()
        .code(SUCCESS);
    let document = String::from_utf8_lossy(&json.get_output().stdout);
    assert!(
        document.starts_with("{\"format\":\"sivi.themes\","),
        "{document}"
    );
    assert!(document.contains("\"name\":\"studio\""), "{document}");
    assert!(document.contains("\"statement\"]"), "{document}");
}

#[test]
fn the_interface_refuses_a_terminal_it_cannot_take_and_a_directory_without_one_deck() {
    let without_terminal = sivi()
        .args(["ui", &deck("pass-basic")])
        .assert()
        .code(USAGE);
    let stderr = String::from_utf8_lossy(&without_terminal.get_output().stderr);
    assert!(
        stderr.starts_with("error: the terminal cannot be taken"),
        "{stderr}"
    );
    let no_deck = sivi().current_dir(DECKS).arg("ui").assert().code(USAGE);
    let refusal = String::from_utf8_lossy(&no_deck.get_output().stderr);
    assert!(refusal.starts_with("error: name the deck"), "{refusal}");
}

#[test]
fn a_theme_check_judges_the_built_in_theme_a_directory_and_a_broken_one() {
    for name in ["plain", "blank", "editorial", "studio"] {
        sivi().args(["theme", "check", name]).assert().code(SUCCESS);
    }
    sivi()
        .args(["theme", "check", PLAIN_THEME])
        .assert()
        .code(SUCCESS);
    let overlap = Path::new(THEMES).join("overlap");
    let refused = sivi()
        .args(["theme", "check"])
        .arg(&overlap)
        .assert()
        .code(DIAGNOSTIC);
    let stderr = String::from_utf8_lossy(&refused.get_output().stderr);
    assert!(stderr.contains("error[E0505]"), "{stderr}");
}
