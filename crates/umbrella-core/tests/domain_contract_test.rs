use std::fs;

use tempfile::tempdir;
use umbrella_core::antivirus::cleaner::{BackupCleaner, CleanStatus, Cleaner};
use umbrella_core::antivirus::detector::{Detector, PatternDetector};
use umbrella_core::antivirus::scanner::{FileSystemScanner, ScanOptions};

#[test]
fn detector_reports_known_upstream_signature_contract() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("userSetup.py");
    fs::write(&file, "import vaccine\nprint('safe line')\n").unwrap();

    let detector = PatternDetector::new();
    let result = detector.detect(file.to_str().unwrap()).unwrap();

    assert!(result.is_infected());
    assert_eq!(result.file_path, file.to_string_lossy());
    assert!(result.threat_type.contains("vaccine"));
    assert_eq!(result.line_numbers, vec![1]);
}

#[test]
fn scanner_ignores_backup_folder_contract() {
    let temp = tempdir().unwrap();
    let infected = temp.path().join("scene.ma");
    let backup_dir = temp.path().join("_virus");
    fs::create_dir_all(&backup_dir).unwrap();
    fs::write(&infected, "cmds.evalDeferred('leukocyte')").unwrap();
    fs::write(
        backup_dir.join("backup.ma"),
        "cmds.evalDeferred('leukocyte')",
    )
    .unwrap();

    let scanner = FileSystemScanner::new();
    let detector = PatternDetector::new();
    let report = scanner
        .scan_with_detector(
            temp.path().to_str().unwrap(),
            &ScanOptions::default(),
            &detector,
        )
        .unwrap();

    assert_eq!(report.files_scanned, 1);
    assert_eq!(report.infected_files.len(), 1);
    assert_eq!(
        report.infected_files[0].file_path,
        infected.to_string_lossy()
    );
}

#[test]
fn cleaner_creates_backup_and_removes_file_signature_contract() {
    let temp = tempdir().unwrap();
    let file = temp.path().join("userSetup.py");
    fs::write(&file, "import vaccine\nprint('safe line')\n").unwrap();

    let cleaner = BackupCleaner::new();
    let result = cleaner
        .clean(file.to_str().unwrap(), &Default::default())
        .unwrap();

    assert_eq!(result.status, CleanStatus::Success);
    assert_eq!(result.threats_removed, 1);
    assert!(result.backup_path.as_deref().unwrap().contains("_virus"));
    assert!(
        !fs::read_to_string(&file)
            .unwrap()
            .contains("import vaccine")
    );
    assert!(fs::read_to_string(&file).unwrap().contains("safe line"));
}
