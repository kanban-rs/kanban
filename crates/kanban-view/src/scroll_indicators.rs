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
