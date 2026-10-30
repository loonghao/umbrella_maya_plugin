//! Regression coverage for the legacy "encoded payload" capability gap.
//!
//! The upstream `maya_umbrella` detects the 2024-04-30 / uifiguration virus
//! family by inspecting live `scriptNode` attributes, where the malicious
//! Python/MEL is stored as base64 text. The Rust rewrite scanned raw file bytes,
//! so those payloads were reported clean.
//!
//! These tests pin the behaviour that scanning a Maya ASCII scene must recover
//! and flag signatures hidden inside base64-encoded payloads, while leaving
//! benign encoded data and binary scenes alone.

use std::fs;
use std::path::PathBuf;

use tempfile::tempdir;
use umbrella_core::antivirus::detector::{Detector, PatternDetector, SCAN_INCOMPLETE};
use umbrella_core::antivirus::scanner::{FileSystemScanner, ScanOptions};

/// A malicious script body carrying two known file-content signatures.
const EVIL_SCRIPT: &[u8] = b"import vaccine\ncmds.evalDeferred('leukocyte = vaccine.phage()')\n";

/// An ordinary, harmless Maya UI helper with no virus signature at all.
const BENIGN_SCRIPT: &[u8] = b"// benign Maya UI helper\n\
import maya.cmds as cmds\n\
def build_ui():\n\
    cmds.window(title='Render')\n\
    cmds.showWindow()\n";

fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut accum = 0u32;
    let mut bits = 0u32;
    for &byte in data {
        accum = (accum << 8) | byte as u32;
        bits += 8;
        while bits >= 6 {
            bits -= 6;
            out.push(TABLE[((accum >> bits) & 0x3f) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(TABLE[((accum << (6 - bits)) & 0x3f) as usize] as char);
    }
    while !out.len().is_multiple_of(4) {
        out.push('=');
    }
    out
}

fn scene_with_payload(script: &[u8]) -> Vec<u8> {
    let b64 = base64_encode(script);
    format!(
        "//Maya ASCII 2024 scene\n\
         requires maya \"2024\";\n\
         createNode script -n \"uifiguration\";\n\
         \tsetAttr \".nts\" -type \"string\" (\"{b64}\");\n\
         createNode transform -n \"safe_geometry\";\n"
    )
    .into_bytes()
}

fn workspace_virus_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("virus")
}

#[test]
fn red_detects_base64_encoded_scriptnode_payload() {
    let content = scene_with_payload(EVIL_SCRIPT);
    let result = PatternDetector::new().detect_bytes("scene.ma", &content);

    assert!(
        result.is_infected(),
        "an encoded virus payload inside a scriptNode attribute must be detected"
    );
    assert!(
        result
            .matches
            .iter()
            .any(|m| m.name.starts_with("encoded:")),
        "the report must state the match came from an encoded payload, got {:?}",
        result.matches.iter().map(|m| &m.name).collect::<Vec<_>>()
    );
}

#[test]
fn scan_report_flags_real_legacy_ma_scenes() {
    let dir = workspace_virus_dir();
    // Skip rather than fail when the checkout omits the (large) sample scenes.
    if !dir.join("2024-4-30.ma").is_file() {
        return;
    }
    let report = FileSystemScanner::new()
        .scan_with_detector(
            dir.to_str().unwrap(),
            &ScanOptions::maya_scene_files(),
            &PatternDetector::new(),
        )
        .unwrap();

    let infected: Vec<String> = report
        .infected_files
        .iter()
        .map(|d| {
            std::path::Path::new(&d.file_path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    for fixture in ["2024-4-30.ma", "uifiguration.ma"] {
        assert!(
            infected.iter().any(|f| f == fixture),
            "{fixture} should be reported infected, got {infected:?}"
        );
    }
}

#[test]
fn benign_encoded_content_is_not_a_false_positive() {
    let content = scene_with_payload(BENIGN_SCRIPT);
    let result = PatternDetector::new().detect_bytes("scene.ma", &content);
    assert!(
        !result.is_infected(),
        "a benign base64 blob must not be flagged, got {:?}",
        result.matches.iter().map(|m| &m.name).collect::<Vec<_>>()
    );
}

#[test]
fn binary_scene_with_nul_bytes_is_not_payload_decoded() {
    // A real .mb begins with FOR4/FOR8 and contains NUL bytes; payload
    // decoding must be skipped for binary content (no garbage matches).
    let mut content = b"FOR4".to_vec();
    content.extend_from_slice(&[0u8; 64]);
    content.extend_from_slice(base64_encode(EVIL_SCRIPT).as_bytes());
    let result = PatternDetector::new().detect_bytes("scene.mb", &content);
    assert!(
        result
            .matches
            .iter()
            .all(|m| !m.name.starts_with("encoded:")),
        "binary content must not produce encoded-payload matches"
    );
}

#[test]
fn detector_reads_file_and_detects_encoded_payload() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("infected.ma");
    fs::write(&path, scene_with_payload(EVIL_SCRIPT)).unwrap();

    let result = PatternDetector::new()
        .detect(path.to_str().unwrap())
        .unwrap();
    assert!(
        result.is_infected(),
        "the file-based Detector::detect path must also recover encoded payloads"
    );
}

#[test]
fn padded_scene_fails_closed_instead_of_reporting_clean() {
    // F2 regression: a scene is padded with a huge benign base64 run placed
    // before the real encoded scriptNode payload. The trailing payload sits
    // near the end of a `.ma` file, exactly where a bounded decoder could skip
    // it. Hitting the byte budget must fail closed, never silently pass clean.
    let mut content = b"//Maya ASCII 2024 scene\n".to_vec();
    let pad_len = content.len() + 12 * 1024 * 1024;
    content.resize(pad_len, b'A');
    content.push(b'\n');
    content.extend_from_slice(&scene_with_payload(EVIL_SCRIPT));

    let result = PatternDetector::new().detect_bytes("padded.ma", &content);
    assert!(
        result.is_infected(),
        "a padded/truncated scene must fail closed, not be reported clean"
    );
    assert!(
        result.description.contains("incomplete"),
        "the report must state the scan was incomplete, got {:?}",
        result.description
    );
    assert!(
        result.matches.iter().any(|m| m.name == SCAN_INCOMPLETE),
        "truncation must surface the scan-incomplete marker, got {:?}",
        result.matches.iter().map(|m| &m.name).collect::<Vec<_>>()
    );
}
