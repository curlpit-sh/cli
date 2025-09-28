use assert_cmd::prelude::*;
use assert_fs::prelude::*;
use httpmock::prelude::*;
use predicates::prelude::*;
use std::process::Command;

fn cargo_bin() -> Command {
    Command::cargo_bin("curlpit").expect("binary exists")
}

#[test]
fn displays_help() {
    let mut cmd = cargo_bin();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("File-first HTTP runner"));
}

#[test]
fn displays_version() {
    let mut cmd = cargo_bin();
    cmd.arg("--version");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn errors_when_request_missing() {
    let mut cmd = cargo_bin();
    cmd.arg("missing.curl");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("missing.curl"));
}

#[test]
fn executes_simple_request() {
    let temp = assert_fs::TempDir::new().unwrap();
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.method(GET).path("/ping");
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"ok\":true}");
    });

    let request = temp.child("sample.curl");
    request
        .write_str(&format!("GET {}\n", server.url("/ping")))
        .unwrap();

    let mut cmd = cargo_bin();
    cmd.current_dir(temp.path());
    cmd.arg("sample.curl").arg("--preview").arg("16");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("GET"))
        .stdout(predicate::str::contains("Status:"));

    mock.assert();
}
