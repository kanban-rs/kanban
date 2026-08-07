use serde::{Deserialize, Serialize};

/// Which end of the viewport the off-screen items lie beyond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScrollDirection {
    Above,
    Below,
}

/// How many list items lie beyond one end of the viewport. Carries the count
/// and the direction only; the noun, pluralization and any padding are the
/// renderer's concern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrollIndicator {
    pub count: usize,
    pub direction: ScrollDirection,
}

impl ScrollIndicator {
    pub fn above(count: usize) -> Self {
        Self {
            count,
            direction: ScrollDirection::Above,
        }
    }

    pub fn below(count: usize) -> Self {
        Self {
            count,
            direction: ScrollDirection::Below,
        }
    }

    /// Whether a renderer should use the plural noun: true for every count
    /// except exactly one.
    pub fn is_plural(&self) -> bool {
        self.count != 1
    }
}

/// The indicator for `direction`, or `None` when the list is not scrolled
/// that way.
pub fn scroll_indicator(
    show: bool,
    count: usize,
    direction: ScrollDirection,
) -> Option<ScrollIndicator> {
    show.then_some(ScrollIndicator { count, direction })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scroll_indicator_above_carries_count_and_direction() {
        let indicator = ScrollIndicator::above(2);
        assert_eq!(indicator.count, 2);
        assert_eq!(indicator.direction, ScrollDirection::Above);
    }

    #[test]
    fn test_scroll_indicator_below_carries_count_and_direction() {
        let indicator = ScrollIndicator::below(7);
        assert_eq!(indicator.count, 7);
        assert_eq!(indicator.direction, ScrollDirection::Below);
    }

    #[test]
    fn test_is_plural_is_false_only_for_a_single_item() {
        assert!(ScrollIndicator::above(0).is_plural());
        assert!(!ScrollIndicator::above(1).is_plural());
        assert!(ScrollIndicator::above(2).is_plural());
    }

    #[test]
    fn test_scroll_indicator_hidden_returns_none() {
        assert_eq!(scroll_indicator(false, 5, ScrollDirection::Above), None);
        assert_eq!(scroll_indicator(false, 5, ScrollDirection::Below), None);
    }

    #[test]
    fn test_scroll_indicator_shown_returns_indicator_for_direction() {
        assert_eq!(
            scroll_indicator(true, 5, ScrollDirection::Above),
            Some(ScrollIndicator::above(5))
        );
        assert_eq!(
            scroll_indicator(true, 5, ScrollDirection::Below),
            Some(ScrollIndicator::below(5))
        );
    }
}
