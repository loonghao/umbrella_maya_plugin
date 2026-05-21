//! Virus signatures ported from `maya_umbrella`.

use serde::Serialize;

/// Signature source category from the original Python project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SignatureKind {
    /// Signatures used for scriptJob/scriptNode detection.
    JobScript,
    /// Signatures used for file content detection and offline cleaning.
    File,
    /// Signatures for maya_secure_system file payloads.
    MayaSecureSystem,
    /// Signatures for maya_secure_system script nodes.
    MayaSecureSystemScriptNode,
}

/// A known Maya virus signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VirusSignature {
    pub name: &'static str,
    pub pattern: &'static str,
    pub kind: SignatureKind,
}

pub const VIRUS20240430_SIG1: &str = r"python(.*);.+exec.+(pyCode).+;";
pub const VIRUS20240430_SIG2: &str = r"^\['.+']";
pub const MAYA_SECURE_SYSTEM_SIG1: &str = "import maya_secure_system";
pub const MAYA_SECURE_SYSTEM_SIG2: &str = r"maya_secure_system\.MayaSecureSystem\(\)\.startup\(\)";
pub const MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG1: &str = "maya_secure_system_scriptNode";
pub const MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG2: &str = "Maya Secure System Stager";
pub const MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG3: &str = "codeExtractor";
pub const MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG4: &str = "codeChunk";

/// `JOB_SCRIPTS_VIRUS_SIGNATURES` from upstream `maya_umbrella.signatures`.
pub const JOB_SCRIPT_SIGNATURES: &[VirusSignature] = &[
    VirusSignature {
        name: "petri_dish_path",
        pattern: r"petri_dish_path.+cmds.internalVar.+",
        kind: SignatureKind::JobScript,
    },
    VirusSignature {
        name: "userSetup",
        pattern: "userSetup",
        kind: SignatureKind::JobScript,
    },
    VirusSignature {
        name: "fuckVirus",
        pattern: "fuckVirus",
        kind: SignatureKind::JobScript,
    },
    VirusSignature {
        name: "virus20240430",
        pattern: VIRUS20240430_SIG1,
        kind: SignatureKind::JobScript,
    },
    VirusSignature {
        name: "virus20240430",
        pattern: VIRUS20240430_SIG2,
        kind: SignatureKind::JobScript,
    },
    VirusSignature {
        name: "maya_secure_system",
        pattern: MAYA_SECURE_SYSTEM_SIG1,
        kind: SignatureKind::JobScript,
    },
    VirusSignature {
        name: "maya_secure_system",
        pattern: MAYA_SECURE_SYSTEM_SIG2,
        kind: SignatureKind::JobScript,
    },
];

/// `FILE_VIRUS_SIGNATURES` from upstream `maya_umbrella.signatures`.
pub const FILE_SIGNATURES: &[VirusSignature] = &[
    VirusSignature {
        name: "vaccine",
        pattern: "import vaccine",
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "leukocyte_eval_deferred",
        pattern: r"cmds.evalDeferred.*leukocyte.+",
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "virus20240430",
        pattern: VIRUS20240430_SIG1,
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "maya_secure_system",
        pattern: MAYA_SECURE_SYSTEM_SIG1,
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "maya_secure_system",
        pattern: MAYA_SECURE_SYSTEM_SIG2,
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG1,
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG2,
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG3,
        kind: SignatureKind::File,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG4,
        kind: SignatureKind::File,
    },
];

pub const MAYA_SECURE_SYSTEM_SIGNATURES: &[VirusSignature] = &[
    VirusSignature {
        name: "maya_secure_system",
        pattern: MAYA_SECURE_SYSTEM_SIG1,
        kind: SignatureKind::MayaSecureSystem,
    },
    VirusSignature {
        name: "maya_secure_system",
        pattern: MAYA_SECURE_SYSTEM_SIG2,
        kind: SignatureKind::MayaSecureSystem,
    },
];

pub const MAYA_SECURE_SYSTEM_SCRIPTNODE_SIGNATURES: &[VirusSignature] = &[
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG1,
        kind: SignatureKind::MayaSecureSystemScriptNode,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG2,
        kind: SignatureKind::MayaSecureSystemScriptNode,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG3,
        kind: SignatureKind::MayaSecureSystemScriptNode,
    },
    VirusSignature {
        name: "maya_secure_system_scriptNode",
        pattern: MAYA_SECURE_SYSTEM_SCRIPTNODE_SIG4,
        kind: SignatureKind::MayaSecureSystemScriptNode,
    },
];

/// Signatures used by the portable scanner. This mirrors the upstream scanner,
/// which combines job-script and file signatures before calling ripgrep.
pub fn scanner_signatures() -> Vec<VirusSignature> {
    let mut signatures = Vec::new();
    for signature in JOB_SCRIPT_SIGNATURES
        .iter()
        .chain(FILE_SIGNATURES.iter())
        .copied()
    {
        if !signatures
            .iter()
            .any(|existing: &VirusSignature| existing.pattern == signature.pattern)
        {
            signatures.push(signature);
        }
    }
    signatures
}

/// Signatures that are safe to remove from file contents by default.
pub fn default_clean_signatures() -> Vec<VirusSignature> {
    FILE_SIGNATURES.to_vec()
}

/// Aggressive offline cleaning also removes job-script signatures.
pub fn aggressive_clean_signatures() -> Vec<VirusSignature> {
    scanner_signatures()
}
