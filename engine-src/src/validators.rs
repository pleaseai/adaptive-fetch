//! Response validation (RFC 0001 §4.2).
//!
//! The full 4-layer battery (status semantics → hard challenge markers → size
//! fingerprint → JSON awareness → caller CSS proof → no-proof heuristics) lands
//! with the generic grid (M2). The M3 Phase 0 slice needs only enough to tell a
//! real RSS/Atom feed from a block, a challenge, or an error — [`validate_feed`].

use crate::Verdict;

/// Hard WAF/challenge interstitial markers (RFC 0001 §4.2, layer 2).
const CHALLENGE_MARKERS: [&str; 5] = [
    "just a moment",
    "cf-challenge",
    "sec-if-cpt-container",
    "/cdn-cgi/challenge-platform",
    "attention required",
];

/// Classify a Phase 0 feed response. Returns the [`Verdict`] and the reasons that
/// produced it (recorded in the trace).
pub fn validate_feed(status: u16, body: &str) -> (Verdict, Vec<String>) {
    // Layer 1 — status semantics.
    match status {
        429 => return (Verdict::RateLimited, vec!["http 429".to_string()]),
        401 | 407 => return (Verdict::AuthRequired, vec![format!("http {status}")]),
        404 | 410 => return (Verdict::NotFound, vec![format!("http {status}")]),
        s if !(200..300).contains(&s) => return (Verdict::Blocked, vec![format!("http {s}")]),
        _ => {}
    }

    // Inspect a bounded, UTF-8-safe prefix (feed/challenge markers live near the top).
    let head: String = body.chars().take(4096).collect();
    let lower = head.to_ascii_lowercase();

    // Layer 2 — hard challenge markers.
    if let Some(marker) = CHALLENGE_MARKERS.iter().find(|m| lower.contains(**m)) {
        return (
            Verdict::Challenge,
            vec![format!("challenge marker: {marker}")],
        );
    }

    // Feed shape — a real RSS/Atom document is the success signal for `.rss`.
    if is_feed(&lower) {
        return (Verdict::WeakOk, vec!["rss/atom feed".to_string()]);
    }

    // 2xx but not a feed at a `.rss` endpoint — we did not get what we asked for.
    (
        Verdict::Challenge,
        vec!["2xx but body is not an rss/atom feed".to_string()],
    )
}

/// Whether the (lowercased) head is a real RSS/Atom/RDF feed — decided by the
/// **root element**, not a substring, so an HTML page that merely contains
/// `<feedback>`, a stray `<feed>` in its body, or an embedded XML example is not
/// mistaken for a feed.
fn is_feed(lower_head: &str) -> bool {
    root_element(lower_head).is_some_and(|root| {
        // Compare the LOCAL name so a namespace-qualified root still matches:
        // `atom:feed` → `feed`, and RSS 1.0's `<rdf:RDF>` → `rdf`.
        let local = root.rsplit(':').next().unwrap_or(root);
        matches!(local, "rss" | "feed" | "rdf")
    })
}

/// The name of the first XML element in `lower_head`, skipping a leading BOM,
/// whitespace, an `<?xml …?>` declaration, comments, and a doctype. `None` if the
/// document does not start with an element.
fn root_element(lower_head: &str) -> Option<&str> {
    let mut rest = lower_head.strip_prefix('\u{feff}').unwrap_or(lower_head);
    loop {
        rest = rest.trim_start();
        rest = if let Some(after) = rest.strip_prefix("<?") {
            &after[after.find("?>")? + 2..] // xml declaration / processing instruction
        } else if let Some(after) = rest.strip_prefix("<!--") {
            &after[after.find("-->")? + 3..] // comment
        } else if let Some(after) = rest.strip_prefix("<!") {
            &after[after.find('>')? + 1..] // doctype / declaration
        } else if let Some(after) = rest.strip_prefix('<') {
            // First real element — its name runs up to whitespace, '>', or '/'.
            let end = after
                .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
                .unwrap_or(after.len());
            return Some(&after[..end]);
        } else {
            return None;
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_rss_and_atom_feeds() {
        let (v, _) = validate_feed(200, "<?xml version=\"1.0\"?><rss version=\"2.0\">…</rss>");
        assert_eq!(v, Verdict::WeakOk);
        let (v, _) = validate_feed(200, "<?xml version=\"1.0\"?><feed xmlns=\"…\">…</feed>");
        assert_eq!(v, Verdict::WeakOk);
    }

    #[test]
    fn flags_challenge_interstitial_even_on_200() {
        let (v, reasons) = validate_feed(200, "<html><title>Just a moment...</title></html>");
        assert_eq!(v, Verdict::Challenge);
        assert!(reasons[0].contains("just a moment"));
    }

    #[test]
    fn two_hundred_non_feed_is_not_success() {
        let (v, _) = validate_feed(200, "<html><body>totally normal page</body></html>");
        assert_eq!(v, Verdict::Challenge);
    }

    #[test]
    fn html_containing_feed_substrings_is_not_a_feed() {
        // The check is on the ROOT element — an HTML page with `<feedback>` or an
        // embedded `<feed>`/`<rss>` example must not be mistaken for a feed.
        let (v, _) = validate_feed(
            200,
            "<!doctype html><html><body><feedback>hi</feedback><feed>x</feed>\
             <pre>&lt;rss&gt;example&lt;/rss&gt;</pre></body></html>",
        );
        assert_eq!(v, Verdict::Challenge);
    }

    #[test]
    fn accepts_feed_with_bom_declaration_and_comments() {
        let (v, _) = validate_feed(
            200,
            "\u{feff}<?xml version=\"1.0\"?>\n<!-- generated --><feed xmlns=\"…\">…</feed>",
        );
        assert_eq!(v, Verdict::WeakOk);
    }

    #[test]
    fn accepts_namespace_qualified_roots() {
        // A namespace-prefixed root must match on its local name.
        let (v, _) = validate_feed(
            200,
            "<?xml version=\"1.0\"?><atom:feed xmlns:atom=\"http://www.w3.org/2005/Atom\"></atom:feed>",
        );
        assert_eq!(v, Verdict::WeakOk);
        // RSS 1.0's `<rdf:RDF>` root.
        let (v, _) = validate_feed(
            200,
            "<?xml version=\"1.0\"?><rdf:RDF xmlns=\"http://purl.org/rss/1.0/\"></rdf:RDF>",
        );
        assert_eq!(v, Verdict::WeakOk);
    }

    #[test]
    fn maps_status_semantics() {
        assert_eq!(validate_feed(429, "").0, Verdict::RateLimited);
        assert_eq!(validate_feed(403, "").0, Verdict::Blocked);
        assert_eq!(validate_feed(404, "").0, Verdict::NotFound);
        assert_eq!(validate_feed(401, "").0, Verdict::AuthRequired);
    }

    #[test]
    fn feed_detection_is_utf8_safe_on_long_multibyte_bodies() {
        // A body longer than the 4096-char inspection window, full of multibyte
        // characters, must not panic on the prefix slice.
        let body = "가".repeat(5000);
        let (v, _) = validate_feed(200, &body);
        assert_eq!(v, Verdict::Challenge);
    }
}
