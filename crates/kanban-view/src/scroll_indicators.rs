/// Format an "N items above" indicator message.
pub fn above_indicator_text(count: usize, label: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("  {} {}{} above", count, label, plural)
}

/// Format an "N items below" indicator message.
pub fn below_indicator_text(count: usize, label: &str) -> String {
    let plural = if count == 1 { "" } else { "s" };
    format!("  {} {}{} below", count, label, plural)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_above_indicator_text_pluralizes_correctly() {
        assert_eq!(above_indicator_text(1, "item"), "  1 item above");
        assert_eq!(above_indicator_text(2, "item"), "  2 items above");
    }

    #[test]
    fn test_below_indicator_text_pluralizes_correctly() {
        assert_eq!(below_indicator_text(1, "item"), "  1 item below");
        assert_eq!(below_indicator_text(2, "item"), "  2 items below");
    }
}
