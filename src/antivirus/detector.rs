//! Threat detection functionality
//! 
//! This module provides threat detection capabilities for analyzing
//! Maya files and scripts for malicious code patterns.

use crate::error::{Result, UmbrellaError};
use std::fs;
use std::path::Path;

/// Threat level classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreatLevel {
    /// No threat detected
    None,
    /// Low-level threat (suspicious but not necessarily malicious)
    Low,
    /// Medium-level threat (likely malicious)
    Medium,
    /// High-level threat (definitely malicious)
    High,
    /// Critical threat (extremely dangerous)
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

/// Result of a threat detection operation
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Path to the analyzed file
    pub file_path: String,
    /// Detected threat level
    pub threat_level: ThreatLevel,
    /// Type of threat detected
    pub threat_type: String,
    /// Detailed description of the threat
    pub description: String,
    /// Line numbers where threats were found
    pub line_numbers: Vec<usize>,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,
}

impl DetectionResult {
    /// Create a new detection result with no threat
    pub fn clean(file_path: &str) -> Self {
        DetectionResult {
            file_path: file_path.to_string(),
            threat_level: ThreatLevel::None,
            threat_type: "None".to_string(),
            description: "No threats detected".to_string(),
            line_numbers: Vec::new(),
            confidence: 1.0,
        }
    }
    
    /// Create a new detection result with a threat
    pub fn threat(
        file_path: &str,
        threat_level: ThreatLevel,
        threat_type: &str,
        description: &str,
        line_numbers: Vec<usize>,
        confidence: f32,
    ) -> Self {
        DetectionResult {
            file_path: file_path.to_string(),
            threat_level,
            threat_type: threat_type.to_string(),
            description: description.to_string(),
            line_numbers,
            confidence,
        }
    }
}

/// Trait for implementing threat detectors
pub trait Detector {
    /// Detect threats in the specified file
    fn detect(&self, file_path: &str) -> Result<DetectionResult>;
    
    /// Get the detector name
    fn name(&self) -> &str;
    
    /// Get the detector version
    fn version(&self) -> &str {
        "1.0.0"
    }
}

/// Pattern-based threat detector
pub struct PatternDetector {
    name: String,
    patterns: Vec<ThreatPattern>,
}

/// A threat pattern definition
#[derive(Debug, Clone)]
pub struct ThreatPattern {
    /// Pattern name
    pub name: String,
    /// Regular expression pattern
    pub pattern: String,
    /// Threat level for this pattern
    pub threat_level: ThreatLevel,
    /// Description of what this pattern detects
    pub description: String,
}

impl PatternDetector {
    /// Create a new pattern detector with default patterns
    pub fn new() -> Self {
        let mut detector = PatternDetector {
            name: "PatternDetector".to_string(),
            patterns: Vec::new(),
        };
        
        detector.load_default_patterns();
        detector
    }
    
    /// Load default threat patterns
    fn load_default_patterns(&mut self) {
        // Common malicious patterns in Maya scripts
        self.patterns.extend(vec![
            ThreatPattern {
                name: "Suspicious Import".to_string(),
                pattern: r"import\s+(os|subprocess|sys|socket)".to_string(),
                threat_level: ThreatLevel::Low,
                description: "Potentially suspicious import statement".to_string(),
            },
            ThreatPattern {
                name: "File System Access".to_string(),
                pattern: r"(os\.system|subprocess\.call|subprocess\.run)".to_string(),
                threat_level: ThreatLevel::Medium,
                description: "Direct system command execution".to_string(),
            },
            ThreatPattern {
                name: "Network Activity".to_string(),
                pattern: r"(socket\.|urllib|requests\.|http)".to_string(),
                threat_level: ThreatLevel::Medium,
                description: "Network communication detected".to_string(),
            },
            ThreatPattern {
                name: "Eval/Exec Usage".to_string(),
                pattern: r"(eval\s*\(|exec\s*\()".to_string(),
                threat_level: ThreatLevel::High,
                description: "Dynamic code execution detected".to_string(),
            },
            ThreatPattern {
                name: "File Deletion".to_string(),
                pattern: r"(os\.remove|os\.unlink|shutil\.rmtree)".to_string(),
                threat_level: ThreatLevel::High,
                description: "File deletion operations detected".to_string(),
            },
            ThreatPattern {
                name: "Registry Access".to_string(),
                pattern: r"(_winreg|winreg)".to_string(),
                threat_level: ThreatLevel::Critical,
                description: "Windows registry access detected".to_string(),
            },
        ]);
    }
    
    /// Add a custom pattern
    pub fn add_pattern(&mut self, pattern: ThreatPattern) {
        self.patterns.push(pattern);
    }
    
    /// Get all patterns
    pub fn patterns(&self) -> &[ThreatPattern] {
        &self.patterns
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
            return Err(UmbrellaError::Antivirus(format!("File does not exist: {}", file_path)));
        }
        
        let content = fs::read_to_string(path)
            .map_err(|e| UmbrellaError::Antivirus(format!("Failed to read file {}: {}", file_path, e)))?;
        
        let mut highest_threat = ThreatLevel::None;
        let mut detected_threats = Vec::new();
        let mut all_line_numbers = Vec::new();
        let mut max_confidence = 0.0f32;
        
        // Analyze each line for patterns
        for (line_num, line) in content.lines().enumerate() {
            for pattern in &self.patterns {
                // Simple pattern matching (in a real implementation, you'd use regex)
                if line.to_lowercase().contains(&pattern.pattern.to_lowercase()) {
                    detected_threats.push(pattern.clone());
                    all_line_numbers.push(line_num + 1);
                    
                    // Update highest threat level
                    if self.threat_level_priority(&pattern.threat_level) > self.threat_level_priority(&highest_threat) {
                        highest_threat = pattern.threat_level.clone();
                    }
                    
                    // Update confidence (simplified calculation)
                    max_confidence = max_confidence.max(0.8);
                }
            }
        }
        
        if detected_threats.is_empty() {
            Ok(DetectionResult::clean(file_path))
        } else {
            let threat_types: Vec<String> = detected_threats.iter().map(|p| p.name.clone()).collect();
            let descriptions: Vec<String> = detected_threats.iter().map(|p| p.description.clone()).collect();
            
            Ok(DetectionResult::threat(
                file_path,
                highest_threat,
                &threat_types.join(", "),
                &descriptions.join("; "),
                all_line_numbers,
                max_confidence,
            ))
        }
    }
    
    fn name(&self) -> &str {
        &self.name
    }
}

impl PatternDetector {
    fn threat_level_priority(&self, level: &ThreatLevel) -> u8 {
        match level {
            ThreatLevel::None => 0,
            ThreatLevel::Low => 1,
            ThreatLevel::Medium => 2,
            ThreatLevel::High => 3,
            ThreatLevel::Critical => 4,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_pattern_detector_creation() {
        let detector = PatternDetector::new();
        assert_eq!(detector.name(), "PatternDetector");
        assert!(!detector.patterns().is_empty());
    }

    #[test]
    fn test_threat_level_priority() {
        let detector = PatternDetector::new();
        assert!(detector.threat_level_priority(&ThreatLevel::Critical) > detector.threat_level_priority(&ThreatLevel::High));
        assert!(detector.threat_level_priority(&ThreatLevel::High) > detector.threat_level_priority(&ThreatLevel::Medium));
        assert!(detector.threat_level_priority(&ThreatLevel::Medium) > detector.threat_level_priority(&ThreatLevel::Low));
        assert!(detector.threat_level_priority(&ThreatLevel::Low) > detector.threat_level_priority(&ThreatLevel::None));
    }
}
