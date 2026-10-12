use std::fs;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn offline_binary_failure_has_nonzero_exit_and_preserves_source() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("asset.mb");
    let original = b"FOR4\0\0\0\x20Mayaimport vaccine";
    fs::write(&path, original).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_umbrella-maya"))
        .arg("--path")
        .arg(&path)
        .arg("--clean")
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("verified Maya standalone"));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn maya_version_without_clean_only_scans() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("asset.mb");
    let original = b"FOR4\0\0\0\x20Mayaimport vaccine";
    fs::write(&path, original).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_umbrella-maya"))
        .arg("--path")
        .arg(&path)
        .args(["--maya-version", "2024"])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn native_clean_without_environment_profile_fails_before_launch() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("asset.mb");
    let original = b"FOR4\0\0\0\x20Mayaimport vaccine";
    fs::write(&path, original).unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_umbrella-maya"))
        .arg("--path")
        .arg(&path)
        .args(["--clean", "--maya-version", "2024"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("--environment-profile"));
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn unsupported_scene_input_is_not_reported_clean() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("userSetup.py");
    fs::write(&path, b"import vaccine").unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_umbrella-maya"))
        .arg("--path")
        .arg(&path)
        .args(["--clean", "--maya-version", "2024"])
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("No supported files were scanned"));
    assert!(!String::from_utf8_lossy(&result.stdout).contains("No infected"));
    assert_eq!(fs::read(&path).unwrap(), b"import vaccine");
}
