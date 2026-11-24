//! 意見與問題: the GitHub issue pages the app opens in the browser (docs/product/DESIGN.md §4.5.2).
//!
//! The address carries ZhuyinType's version, the system's name and version, and for a removal the reason picked from
//! [`UNINSTALL_REASONS`]: nothing the user typed, no words, no app names, no paths.  The user writes the rest and sends
//! it on GitHub; the app itself sends nothing.

use crate::health::report::System;

/// Where issues are made.
pub const NEW_ISSUE_URL: &str = "https://github.com/henry753951/ZhuyinType/issues/new";

/// Why ZhuyinType is removed (the Windows uninstaller offers the same ones: installer.nsi).
pub const UNINSTALL_REASONS: [&str; 6] =
    ["轉換或選字不準", "不穩定、變慢或無法輸入", "缺少需要的功能", "操作習慣不同", "改用其他輸入法", "其他原因"];

/// What the user wants to tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Topic {
    Conversion,
    Crash,
    Other,
    Suggestion,
    /// Removed, with one of [`UNINSTALL_REASONS`].
    Uninstall(&'static str),
}

impl Topic {
    /// Its form in .github/ISSUE_TEMPLATE.
    #[must_use]
    pub fn template(self) -> &'static str {
        match self {
            Self::Conversion => "conversion.yml",
            Self::Crash => "crash.yml",
            Self::Other => "other.yml",
            Self::Suggestion => "suggestion.yml",
            Self::Uninstall(_) => "uninstall.yml",
        }
    }
}

/// The issue page for `topic`, its version and system filled in.
#[must_use]
pub fn url(topic: Topic, version: &str, system: &System) -> String {
    let mut os = system.os.clone();
    if !system.desktop.is_empty() {
        os = format!("{os}，{}", system.desktop);
    }
    let os = format!("{os}，{}", system.arch);
    let mut fields = vec![("template", topic.template()), ("version", version), ("system", os.as_str())];
    if let Topic::Uninstall(reason) = topic {
        fields.push(("reason", reason));
    }
    let query: Vec<String> = fields.into_iter().map(|(name, value)| format!("{name}={}", encoded(value))).collect();
    format!("{NEW_ISSUE_URL}?{}", query.join("&"))
}

/// `text` percent-encoded (RFC 3986's unreserved characters stay).
fn encoded(text: &str) -> String {
    use std::fmt::Write as _;
    text.bytes().fold(String::with_capacity(text.len()), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || b"-._~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _ = write!(out, "%{byte:02X}");
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../..");

    fn system() -> System {
        System { os: "Windows 11 24H2（組建 26100）".into(), desktop: String::new(), arch: "x86_64".into() }
    }

    fn decoded(text: &str) -> String {
        let bytes = text.as_bytes();
        let mut out = Vec::new();
        let mut at = 0;
        while at < bytes.len() {
            if bytes[at] == b'%' {
                out.push(u8::from_str_radix(&text[at + 1..at + 3], 16).unwrap());
                at += 3;
            } else {
                out.push(bytes[at]);
                at += 1;
            }
        }
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn the_address_carries_the_version_and_the_system_only() {
        let url = url(Topic::Crash, "0.7.6", &system());
        let (page, query) = url.split_once('?').unwrap();
        assert_eq!(page, NEW_ISSUE_URL);
        assert!(query.is_ascii());
        let fields: Vec<(String, String)> =
            query.split('&').map(|f| f.split_once('=').unwrap()).map(|(k, v)| (k.to_owned(), decoded(v))).collect();
        assert_eq!(
            fields,
            [
                ("template".to_owned(), "crash.yml".to_owned()),
                ("version".to_owned(), "0.7.6".to_owned()),
                ("system".to_owned(), "Windows 11 24H2（組建 26100），x86_64".to_owned()),
            ]
        );
    }

    #[test]
    fn reserved_characters_are_encoded() {
        assert_eq!(encoded("a b&c=d#e?中"), "a%20b%26c%3Dd%23e%3F%E4%B8%AD");
    }

    #[test]
    fn every_topic_has_its_form_with_the_fields_filled_in() {
        let topics =
            [Topic::Conversion, Topic::Crash, Topic::Other, Topic::Suggestion, Topic::Uninstall(UNINSTALL_REASONS[0])];
        for topic in topics {
            let path = format!("{ROOT}/.github/ISSUE_TEMPLATE/{}", topic.template());
            let form = std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path}: {error}"));
            assert!(form.contains("id: version") && form.contains("id: system"), "{path}");
            assert!(form.contains("私人"), "{path}: says not to post anything private");
            assert_eq!(form.contains("id: reason"), matches!(topic, Topic::Uninstall(_)), "{path}");
        }
    }

    #[test]
    fn the_windows_uninstaller_offers_the_same_reasons() {
        let path = format!("{ROOT}/platforms/desktop/packaging/windows/installer.nsi");
        let script = std::fs::read_to_string(&path).unwrap();
        for reason in UNINSTALL_REASONS {
            assert!(script.contains(&format!("\"{reason}\" \"{}\"", encoded(reason))), "{reason}: {path}");
        }
    }
}
