//! Recovery of malware hidden inside base64-encoded payloads in Maya scenes.
//!
//! The 2024-04-30 / uifiguration virus family stores its Python/MEL payload as
//! base64 text inside a `scriptNode` string attribute (for example
//! `setAttr ".nts" -type "string" ("['<base64>']")`). `maya_umbrella` detects
//! these by reading live script-node attributes; a byte scanner that only looks
//! at the literal file contents reports such scenes as clean.
//!
//! This module recovers those decoded bytes so the existing signature patterns
//! can be applied to them. It is deliberately self-contained (no new crate
//! dependency) and conservative:
//!
//! * binary content (any NUL byte, as in `.mb` scenes or renamed binaries) is
//!   skipped — it is handled by the raw byte detector and Maya-native cleaning;
//! * only runs that decode to mostly *valid UTF-8 text* are kept, so random
//!   binary data is rejected instead of being mistaken for script source;
//! * the total decoded bytes per file are bounded by `MAX_DECODED_BYTES`; if
//!   that bound is hit the scan is reported as truncated so the caller can
//!   fail closed rather than silently declaring the scene clean.

/// Only runs at least this long are decoded. Real payloads are thousands of
/// characters; this skips ordinary identifiers, hashes and node names that
/// happen to share the base64 alphabet.
const MIN_ENCODED_LEN: usize = 64;
/// Bound the total decoded bytes for a single (possibly huge) scene so scanning
/// stays cheap. Exceeding this bound flags the scan as incomplete (fail-closed).
const MAX_DECODED_BYTES: usize = 8 * 1024 * 1024;
/// Decoded payloads must be mostly printable, well-formed UTF-8 text (script
/// source), not binary.
const MIN_PRINTABLE_RATIO: f64 = 0.85;

/// A decoded base64 payload and the byte offset where its run began in the
/// original content (used to report a useful line/offset location).
#[derive(Debug, Clone)]
pub(crate) struct EmbeddedPayload {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

/// Result of decoding embedded payloads from one file.
#[derive(Debug, Clone)]
pub(crate) struct DecodedPayloads {
    pub payloads: Vec<EmbeddedPayload>,
    /// `true` when the byte budget prevented decoding every candidate run.
    /// Callers must fail closed: a truncated scan is not proof of a clean file.
    pub truncated: bool,
}

/// Find and decode base64 payloads embedded in `content` using the default
/// work budget.
pub(crate) fn decode_embedded_payloads(content: &[u8]) -> DecodedPayloads {
    decode_with_budget(content, MAX_DECODED_BYTES)
}

/// Budgeted decode core, shared with unit tests so the truncation path can be
/// exercised without allocating a multi-megabyte scene.
fn decode_with_budget(content: &[u8], max_decoded_bytes: usize) -> DecodedPayloads {
    // Binary content (any NUL byte, as in `.mb` scenes or renamed binaries) is
    // ignored: those are handled by the raw byte detector and by Maya-native
    // cleaning, and decoding arbitrary binary would only produce noise.
    if content.contains(&0) {
        return DecodedPayloads {
            payloads: Vec::new(),
            truncated: false,
        };
    }

    let mut payloads = Vec::new();
    let mut decoded_budget = max_decoded_bytes;
    let mut truncated = false;

    // There is no cap on the *number* of runs: scriptNode payloads usually sit
    // near the end of a `.ma` file, so a count limit could hide the real threat
    // behind a wall of benign base64. Work is bounded by the decoded-byte
    // budget, and hitting that budget flags the scan as incomplete instead of
    // silently dropping the remaining candidates.
    for (start, end) in find_base64_runs(content) {
        let run = &content[start..end];
        let estimated_decoded = run.len() / 4 * 3;
        if estimated_decoded > decoded_budget {
            truncated = true;
            continue;
        }
        let Some(bytes) = decode_base64(run) else {
            continue;
        };
        if !is_mostly_printable(&bytes) {
            continue;
        }
        decoded_budget = decoded_budget.saturating_sub(bytes.len());
        payloads.push(EmbeddedPayload {
            offset: start,
            bytes,
        });
    }

    DecodedPayloads {
        payloads,
        truncated,
    }
}

/// Locate maximal runs of standard base64 alphabet characters.
fn find_base64_runs(content: &[u8]) -> Vec<(usize, usize)> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;

    for (index, &byte) in content.iter().enumerate() {
        let is_base64 = byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'/';
        if is_base64 {
            if start.is_none() {
                start = Some(index);
            }
        } else if let Some(run_start) = start.take()
            && index - run_start >= MIN_ENCODED_LEN
        {
            runs.push((run_start, index));
        }
    }
    if let Some(run_start) = start
        && content.len() - run_start >= MIN_ENCODED_LEN
    {
        runs.push((run_start, content.len()));
    }

    runs
}

/// Streaming standard-base64 decode. Padding is not part of a run (the run
/// finder stops at `=`), so this also accepts unpadded input: it emits every
/// complete byte and drops any trailing partial sextet.
fn decode_base64(run: &[u8]) -> Option<Vec<u8>> {
    fn sextet(byte: u8) -> Option<u32> {
        match byte {
            b'A'..=b'Z' => Some((byte - b'A') as u32),
            b'a'..=b'z' => Some((byte - b'a' + 26) as u32),
            b'0'..=b'9' => Some((byte - b'0' + 52) as u32),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }

    let mut out = Vec::with_capacity(run.len() * 3 / 4 + 3);
    let mut accum = 0u32;
    let mut bits = 0u32;
    for &byte in run {
        accum = (accum << 6) | sextet(byte)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((accum >> bits) as u8);
            accum &= (1u32 << bits) - 1;
        }
    }
    Some(out)
}

/// A decoded payload must be mostly printable, well-formed UTF-8 text.
///
/// High bytes are only counted when they form a valid UTF-8 scalar value
/// (via `Utf8Chunks`). Counting every byte `>= 0x80` as printable made the
/// filter ineffective: random binary then scored ~88% printable and passed
/// the 0.85 threshold. Malformed UTF-8 bytes and control characters are
/// treated as non-text and drag the ratio down.
fn is_mostly_printable(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let mut printable = 0usize;
    for chunk in bytes.utf8_chunks() {
        for ch in chunk.valid().chars() {
            // Tabs/newlines/carriage returns and any non-control scalar are
            // text; NUL and other control characters are not. `invalid()`
            // bytes are malformed UTF-8 and are deliberately not counted.
            if ch == '\t' || ch == '\n' || ch == '\r' || !ch.is_control() {
                printable += ch.len_utf8();
            }
        }
    }
    (printable as f64 / bytes.len() as f64) >= MIN_PRINTABLE_RATIO
}

#[cfg(test)]
mod tests {
    use super::*;

    const EVIL_PLAIN: &[u8] = b"import vaccine\ncmds.evalDeferred('leukocyte = vaccine.phage()')\n";

    /// Standard-base64 encoder (test-only), so fixtures are computed at runtime
    /// rather than embedding opaque base64 literals in the source tree.
    fn b64(data: &[u8]) -> Vec<u8> {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = Vec::new();
        let mut accum = 0u32;
        let mut bits = 0u32;
        for &byte in data {
            accum = (accum << 8) | byte as u32;
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                out.push(TABLE[((accum >> bits) & 0x3f) as usize]);
            }
        }
        if bits > 0 {
            out.push(TABLE[((accum << (6 - bits)) & 0x3f) as usize]);
        }
        while !out.len().is_multiple_of(4) {
            out.push(b'=');
        }
        out
    }

    #[test]
    fn decodes_unpadded_run_to_original_script() {
        // The run finder stops at `=`, so decode only sees the alphabet run.
        let encoded = b64(EVIL_PLAIN);
        let alphabet_len = encoded.iter().take_while(|&&b| b != b'=').count();
        let run = &encoded[..alphabet_len];
        assert_eq!(decode_base64(run).unwrap(), EVIL_PLAIN);
    }

    #[test]
    fn finds_single_long_run_and_skips_short_tokens() {
        let mut content = b"createNode script -n \"shortToken123\";\n".to_vec();
        let prefix_len = content.len();
        content.extend_from_slice(&b64(EVIL_PLAIN));
        let result = decode_embedded_payloads(&content);
        assert_eq!(result.payloads.len(), 1);
        assert_eq!(result.payloads[0].bytes, EVIL_PLAIN);
        assert!(result.payloads[0].offset >= prefix_len);
        assert!(!result.truncated);
    }

    #[test]
    fn binary_content_is_ignored() {
        let mut content = b"FOR4\0\0".to_vec();
        content.extend_from_slice(&b64(EVIL_PLAIN));
        let result = decode_embedded_payloads(&content);
        assert!(result.payloads.is_empty());
        assert!(
            !result.truncated,
            "binary scenes are handled by the raw detector, not a truncated decode"
        );
    }

    #[test]
    fn non_text_decodes_are_rejected() {
        // `'A' * 128` decodes to NUL/control bytes, which are not printable
        // text. Assert the extractor rejects it instead of returning a payload.
        let nul_run = vec![b'A'; 128];
        let mut content = b"setAttr \".nts\" -type \"string\" (\"".to_vec();
        content.extend_from_slice(&nul_run);
        content.extend_from_slice(b"\");");
        let result = decode_embedded_payloads(&content);
        assert!(
            result.payloads.is_empty(),
            "a run decoding to NUL/control bytes must be rejected, got {:?}",
            result.payloads
        );

        // A deterministic high-entropy byte buffer decodes to mostly invalid
        // UTF-8 and must also be rejected (the old filter counted every high
        // byte as printable and wrongly kept it).
        let mut state = 0x9e37_79b9u32;
        let random: Vec<u8> = (0..512)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
            .collect();
        let mut content = b"setAttr \".nts\" -type \"string\" (\"".to_vec();
        content.extend_from_slice(&b64(&random));
        content.extend_from_slice(b"\");");
        let result = decode_embedded_payloads(&content);
        assert!(
            result.payloads.is_empty(),
            "a run decoding to random binary must be rejected"
        );
    }

    #[test]
    fn printable_filter_requires_valid_utf8_text() {
        assert!(is_mostly_printable(b"import vaccine\nprint('ok')\n"));
        assert!(is_mostly_printable(
            "caf\u{e9} \u{4e2d}\u{6587} script\n".as_bytes()
        ));
        assert!(!is_mostly_printable(&[0u8; 96])); // NUL control bytes
        assert!(!is_mostly_printable(&[0x80, 0x81, 0xfe, 0xff].repeat(32))); // invalid UTF-8

        // High-entropy bytes: ~50% are >0x7f but mostly invalid UTF-8, so the
        // printable ratio stays far below 0.85.
        let mut state = 0x1234_5678u32;
        let random: Vec<u8> = (0..4096)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                (state >> 24) as u8
            })
            .collect();
        assert!(!is_mostly_printable(&random));
    }

    #[test]
    fn budget_exhaustion_is_reported_as_truncated() {
        // Two printable payloads separated by a space (two distinct runs).
        let mut content = b64(EVIL_PLAIN);
        content.push(b' ');
        content.extend_from_slice(&b64(EVIL_PLAIN));

        // A generous budget decodes both runs.
        let full = decode_with_budget(&content, MAX_DECODED_BYTES);
        assert_eq!(full.payloads.len(), 2);
        assert!(!full.truncated);

        // A tiny budget cannot fit either run; exhaustion must be reported
        // rather than silently returning a clean/empty result.
        let tiny = decode_with_budget(&content, 8);
        assert!(
            tiny.truncated,
            "exceeding the decode budget must be reported, not silent"
        );
    }
}
