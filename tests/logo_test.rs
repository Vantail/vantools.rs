//! Integration tests for the embedded ASCII logo.

use vantools::tui::logo::LOGO;
use vantools::tui::logo::formatted_version;
use vantools::tui::logo::logo_max_width;

#[test]
fn logo_is_not_empty() {
    assert!(!LOGO.is_empty(), "embedded logo is empty");
    return;
}

#[test]
fn logo_contains_underscore() {
    assert!(LOGO.contains('_'), "embedded logo is missing '_'");
    return;
}

#[test]
fn version_line_ends_at_logo_edge() {
    let line = formatted_version();
    let plain = line.replace("\x1b[3m", "").replace("\x1b[0m", "");
    let mut plain_len: usize = 0;
    for _ in plain.chars() {
        plain_len += 1;
    }
    assert_eq!(plain_len, logo_max_width());
    assert!(line.contains(env!("CARGO_PKG_VERSION")));
    return;
}
