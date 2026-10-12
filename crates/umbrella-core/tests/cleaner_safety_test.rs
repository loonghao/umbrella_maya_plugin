//! Synthetic safety regressions. These are NOT real Maya acceptance fixtures.
use std::fs;
use tempfile::tempdir;
use umbrella_core::antivirus::cleaner::{BackupCleaner, CleanOptions, CleanStatus, Cleaner};

#[test]
fn maya_binary_is_never_rewritten_with_any_clean_options() {
    for extension in ["mb", "MB"] {
        for in_place in [true, false] {
            for create_backup in [true, false] {
                let temp = tempdir().unwrap();
                let path = temp.path().join(format!("asset.{extension}"));
                let payload = b"import vaccine\n";
                let mut original = b"FOR4".to_vec();
                original.extend_from_slice(&(payload.len() as u32).to_be_bytes());
                original.extend_from_slice(payload);
                fs::write(&path, &original).unwrap();
                let result = BackupCleaner::new()
                    .clean(
                        path.to_str().unwrap(),
                        &CleanOptions {
                            in_place,
                            create_backup,
                            aggressive: true,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(result.status, CleanStatus::Failed);
                assert!(result.message.contains("Maya standalone"));
                assert_eq!(fs::read(&path).unwrap(), original);
                assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 1);
            }
        }
    }
}

#[test]
fn renamed_binary_is_not_treated_as_text() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("asset.ma");
    let original = b"FOR4\0\0\0\x20Mayaimport vaccine";
    fs::write(&path, original).unwrap();
    let result = BackupCleaner::new()
        .clean(path.to_str().unwrap(), &Default::default())
        .unwrap();
    assert_eq!(result.status, CleanStatus::Failed);
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn non_in_place_empty_cleanup_preserves_original() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("userSetup.py");
    let original = b"import vaccine\n";
    fs::write(&path, original).unwrap();
    let result = BackupCleaner::new()
        .clean(
            path.to_str().unwrap(),
            &CleanOptions {
                in_place: false,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(result.status, CleanStatus::Success);
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(temp.path().join("userSetup.py.cleaned").exists());
}

#[test]
fn backups_are_unique_and_previous_original_is_retained() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("userSetup.py");
    let cleaner = BackupCleaner::new();
    let first = b"import vaccine\nprint('first')\n";
    fs::write(&path, first).unwrap();
    let backup1 = cleaner
        .clean(path.to_str().unwrap(), &Default::default())
        .unwrap()
        .backup_path
        .unwrap();
    let second = b"import vaccine\nprint('second')\n";
    fs::write(&path, second).unwrap();
    let backup2 = cleaner
        .clean(path.to_str().unwrap(), &Default::default())
        .unwrap()
        .backup_path
        .unwrap();
    assert_ne!(backup1, backup2);
    assert_eq!(fs::read(backup1).unwrap(), first);
    assert_eq!(fs::read(backup2).unwrap(), second);
}

#[test]
fn backup_failure_or_concurrent_lock_prevents_mutation() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("userSetup.py");
    let original = b"import vaccine\nprint('safe')\n";
    fs::write(&path, original).unwrap();
    fs::write(temp.path().join("_virus"), b"not a directory").unwrap();
    assert!(
        BackupCleaner::new()
            .clean(path.to_str().unwrap(), &Default::default())
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    fs::write(
        temp.path().join("userSetup.py.umbrella-clean.lock"),
        b"held",
    )
    .unwrap();
    assert!(
        BackupCleaner::new()
            .clean(
                path.to_str().unwrap(),
                &CleanOptions {
                    create_backup: false,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn existing_output_is_never_overwritten() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("userSetup.py");
    let output = temp.path().join("userSetup.py.cleaned");
    fs::write(&path, b"import vaccine\nprint('safe')\n").unwrap();
    fs::write(&output, b"previous output").unwrap();
    assert!(
        BackupCleaner::new()
            .clean(
                path.to_str().unwrap(),
                &CleanOptions {
                    in_place: false,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(fs::read(&output).unwrap(), b"previous output");
}

#[test]
fn residual_signatures_fail_before_any_write() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("userSetup.py");
    let original = b"import vaccine\nprint('userSetup')\n";
    fs::write(&path, original).unwrap();
    let result = BackupCleaner::new()
        .clean(path.to_str().unwrap(), &Default::default())
        .unwrap();
    assert_eq!(result.status, CleanStatus::Failed);
    assert_eq!(fs::read(&path).unwrap(), original);
}

#[test]
fn large_binary_files_are_failed_not_silently_skipped() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("large.mb");
    let file = fs::File::create(&path).unwrap();
    file.set_len(100 * 1024 * 1024 + 1).unwrap();
    drop(file);
    let result = BackupCleaner::new()
        .clean_path(path.to_str().unwrap(), &Default::default())
        .unwrap();
    assert_eq!(result.files_scanned, 1);
    assert_eq!(result.files_failed, 1);
    assert_eq!(fs::metadata(path).unwrap().len(), 100 * 1024 * 1024 + 1);
}
