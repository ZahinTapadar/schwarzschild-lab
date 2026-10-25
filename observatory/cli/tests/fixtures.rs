//! Every preset renders without NaNs, within its physical limits, and matches
//! its stored image-regression fixture (see `observatory fixtures`).
#[test]
fn presets_pass_checks_and_match_fixtures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_observatory")).arg("fixtures").current_dir(&root).output().expect("run observatory fixtures");
    eprintln!("{}", String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "fixture checks failed");
}
