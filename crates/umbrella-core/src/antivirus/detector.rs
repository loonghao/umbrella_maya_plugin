//! Signature-based threat detection for Maya files and scripts.

use crate::antivirus::signatures::{VirusSignature, scanner_signatures};
use crate::error::{Result, UmbrellaError};
use regex::bytes::Regex;
use serde::Serialize;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

/// Threat level classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ThreatLevel {
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for ThreatLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreatLevel::None => write!(f, "None"),
            ThreatLevel::Low => write!(f, "Low"),
            ThreatLevel::Medium => write!(f, "Medium"),
            ThreatLevel::High => write!(f, "High"),
            ThreatLevel::Critical => write!(f, "Critical"),
        }
    }
}

/// One matched signature in a file.
#[derive(Debug, Clone, Serialize)]
pub struct ThreatMatch {
    pub name: String,
    pub pattern: String,
    pub line: usize,
    pub offset: usize,
}

/// Result of a threat detection operation.
#[derive(Debug, Clone, Serialize)]
pub struct DetectionResult {
    pub file_path: String,
    pub threat_level: ThreatLevel,
    pub threat_type: String,
    pub description: String,
    pub line_numbers: Vec<usize>,
    pub confidence: f32,
    pub matches: Vec<ThreatMatch>,
}

impl DetectionResult {
    pub fn clean(file_path: &str) -> Self {
        DetectionResult {
            file_path: file_path.to_string(),
            threat_level: ThreatLevel::None,
            threat_type: "None".to_string(),
            description: "No threats detected".to_string(),
            line_numbers: Vec::new(),
            confidence: 1.0,
            matches: Vec::new(),
        }
    }

    pub fn is_infected(&self) -> bool {
        !self.matches.is_empty()
    }

    fn from_matches(file_path: &str, matches: Vec<ThreatMatch>) -> Self {
        if matches.is_empty() {
            return Self::clean(file_path);
        }

        let mut line_numbers = BTreeSet::new();
        let mut threat_names = BTreeSet::new();
        for hit in &matches {
            line_numbers.insert(hit.line);
            threat_names.insert(hit.name.clone());
        }

        let threat_type = threat_names.into_iter().collect::<Vec<_>>().join(", ");
        DetectionResult {
            file_path: file_path.to_string(),
            threat_level: ThreatLevel::Critical,
            description: format!("Matched {} known Maya virus signature(s)", matches.len()),
            line_numbers: line_numbers.into_iter().collect(),
            confidence: 0.95,
            matches,
            threat_type,
        }
    }
}

/// Trait for implementing threat detectors.
pub trait Detector {
    fn detect(&self, file_path: &str) -> Result<DetectionResult>;
    fn name(&self) -> &str;

    fn version(&self) -> &str {
        "1.0.0"
    }
}

/// Regex pattern detector using upstream maya_umbrella signatures.
pub struct PatternDetector {
    name: String,
    patterns: Vec<CompiledThreatPattern>,
}

#[derive(Debug, Clone)]
pub struct ThreatPattern {
    pub name: String,
    pub pattern: String,
    pub threat_level: ThreatLevel,
    pub description: String,
}

struct CompiledThreatPattern {
    signature: VirusSignature,
    regex: Regex,
}

impl PatternDetector {
    pub fn new() -> Self {
        Self::from_signatures(scanner_signatures()).expect("bundled virus signatures must compile")
    }

    pub fn from_signatures(signatures: Vec<VirusSignature>) -> Result<Self> {
        let mut patterns = Vec::with_capacity(signatures.len());
        for signature in signatures {
            let regex = Regex::new(signature.pattern).map_err(|err| {
                UmbrellaError::Antivirus(format!(
                    "Invalid signature regex '{}': {}",
                    signature.pattern, err
                ))
            })?;
            patterns.push(CompiledThreatPattern { signature, regex });
        }

        Ok(Self {
            name: "PatternDetector".to_string(),
            patterns,
        })
    }

    pub fn detect_bytes(&self, file_path: &str, content: &[u8]) -> DetectionResult {
        let mut matches = Vec::new();

        for pattern in &self.patterns {
            for hit in pattern.regex.find_iter(content) {
                matches.push(ThreatMatch {
                    name: pattern.signature.name.to_string(),
                    pattern: pattern.signature.pattern.to_string(),
                    line: line_number(content, hit.start()),
                    offset: hit.start(),
                });
            }
        }

        DetectionResult::from_matches(file_path, matches)
    }

    pub fn patterns(&self) -> Vec<ThreatPattern> {
        self.patterns
            .iter()
            .map(|compiled| ThreatPattern {
                name: compiled.signature.name.to_string(),
                pattern: compiled.signature.pattern.to_string(),
                threat_level: ThreatLevel::Critical,
                description: format!("Known maya_umbrella signature: {}", compiled.signature.name),
            })
            .collect()
    }
}

impl Default for PatternDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl Detector for PatternDetector {
    fn detect(&self, file_path: &str) -> Result<DetectionResult> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(UmbrellaError::Antivirus(format!(
                "File does not exist: {}",
                file_path
            )));
        }

        let content = fs::read(path).map_err(|err| {
            UmbrellaError::Antivirus(format!("Failed to read file {}: {}", file_path, err))
        })?;

        Ok(self.detect_bytes(file_path, &content))
    }

    fn name(&self) -> &str {
        &self.name
    }
}

fn line_number(content: &[u8], offset: usize) -> usize {
    content[..offset.min(content.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::antivirus::signatures::FILE_SIGNATURES;

    #[test]
    fn test_threat_level_display() {
        assert_eq!(ThreatLevel::None.to_string(), "None");
        assert_eq!(ThreatLevel::Low.to_string(), "Low");
        assert_eq!(ThreatLevel::Medium.to_string(), "Medium");
        assert_eq!(ThreatLevel::High.to_string(), "High");
        assert_eq!(ThreatLevel::Critical.to_string(), "Critical");
    }

    #[test]
    fn test_detection_result_clean() {
        let result = DetectionResult::clean("test.py");
        assert_eq!(result.file_path, "test.py");
        assert_eq!(result.threat_level, ThreatLevel::None);
        assert_eq!(result.confidence, 1.0);
    }

    #[test]
    fn test_pattern_detector_detects_upstream_signature() {
        let detector = PatternDetector::new();
        let result = detector.detect_bytes("userSetup.py", b"import vaccine\nprint('x')\n");
        assert!(result.is_infected());
        assert_eq!(result.line_numbers, vec![1]);
        assert!(result.threat_type.contains("vaccine"));
    }

    #[test]
    fn test_pattern_detector_creation() {
        let detector = PatternDetector::new();
        assert_eq!(detector.name(), "PatternDetector");
        assert!(detector.patterns().len() >= FILE_SIGNATURES.len());
    }
}
