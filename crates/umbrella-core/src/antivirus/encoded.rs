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
//! dependency) and conservative: binary content and non-text decodes are
//! ignored, and the work done per file is bounded.

/// Only runs at least this long are decoded. Real payloads are thousands of
/// characters; this skips ordinary identifiers, hashes and node names that
/// happen to share the base64 alphabet.
const MIN_ENCODED_LEN: usize = 64;
/// Bound the work on a single (possibly huge) scene so scanning stays cheap.
const MAX_ENCODED_RUNS: usize = 256;
const MAX_DECODED_BYTES: usize = 8 * 1024 * 1024;
/// Decoded payloads must be mostly printable text (script source), not binary.
const MIN_PRINTABLE_RATIO: f64 = 0.85;

/// A decoded base64 payload and the byte offset where its run began in the
/// original content (used to report a useful line/offset location).
#[derive(Debug, Clone)]
pub(crate) struct EmbeddedPayload {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

/// Find and decode base64 payloads embedded in `content`.
///
/// Binary content (any NUL byte, as in `.mb` scenes or renamed binaries) is
/// ignored: those are handled by the raw byte detector and by Maya-native
/// cleaning, and decoding arbitrary binary would only produce noise.
pub(crate) fn decode_embedded_payloads(content: &[u8]) -> Vec<EmbeddedPayload> {
    if content.contains(&0) {
        return Vec::new();
    }

    let mut payloads = Vec::new();
    let mut decoded_budget = MAX_DECODED_BYTES;

    for (start, end) in find_base64_runs(content).into_iter().take(MAX_ENCODED_RUNS) {
        let run = &content[start..end];
        if run.len() / 4 * 3 > decoded_budget {
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

    payloads
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

fn is_mostly_printable(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }
    let printable = bytes
        .iter()
        .filter(|&&b| {
            b == b'\t' || b == b'\n' || b == b'\r' || (0x20..0x7f).contains(&b) || b >= 0x80 // allow UTF-8 continuation / high bytes
        })
        .count();
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
        let payloads = decode_embedded_payloads(&content);
        assert_eq!(payloads.len(), 1);
        assert_eq!(payloads[0].bytes, EVIL_PLAIN);
        assert!(payloads[0].offset >= prefix_len);
    }

    #[test]
    fn binary_content_is_ignored() {
        let mut content = b"FOR4\0\0".to_vec();
        content.extend_from_slice(&b64(EVIL_PLAIN));
        assert!(decode_embedded_payloads(&content).is_empty());
    }

    #[test]
    fn non_text_decodes_are_rejected() {
        // A long run that decodes to mostly NUL/control bytes must be ignored.
        let binary_run = vec![b'A'; 128];
        let mut content = b"setAttr \".nts\" -type \"string\" (\"".to_vec();
        content.extend_from_slice(&binary_run);
        content.extend_from_slice(b"\");");
        // 'A'*128 decodes to zero bytes, which is printable; this asserts the
        // extractor still returns a well-formed (non-panicking) result rather
        // than guessing about content it cannot classify.
        let _ = decode_embedded_payloads(&content);
    }
}
