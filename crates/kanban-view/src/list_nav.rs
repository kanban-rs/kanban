/// Find the next selectable index after `cur`, where "selectable" is defined
/// by the predicate. Skips non-selectable entries. Clamps at the last
/// selectable index; returns `None` only when no selectable index exists.
pub fn next_selectable_index<F>(cur: Option<usize>, count: usize, is_selectable: F) -> Option<usize>
where
    F: Fn(usize) -> bool,
{
    let start = cur.map(|i| i + 1).unwrap_or(0);
    (start..count)
        .find(|&i| is_selectable(i))
        .or_else(|| cur_if_selectable_index(cur, count, &is_selectable))
        .or_else(|| last_selectable_index(count, &is_selectable))
}

/// Find the previous selectable index before `cur`. Skips non-selectable
/// entries. Clamps at the first selectable index; returns `None` only when
/// no selectable index exists.
pub fn prev_selectable_index<F>(cur: Option<usize>, count: usize, is_selectable: F) -> Option<usize>
where
    F: Fn(usize) -> bool,
{
    let end = cur.unwrap_or(count);
    (0..end)
        .rev()
        .find(|&i| is_selectable(i))
        .or_else(|| cur_if_selectable_index(cur, count, &is_selectable))
        .or_else(|| first_selectable_index(count, &is_selectable))
}

pub fn first_selectable_index<F>(count: usize, is_selectable: F) -> Option<usize>
where
    F: Fn(usize) -> bool,
{
    (0..count).find(|&i| is_selectable(i))
}

pub fn last_selectable_index<F>(count: usize, is_selectable: F) -> Option<usize>
where
    F: Fn(usize) -> bool,
{
    (0..count).rev().find(|&i| is_selectable(i))
}

fn cur_if_selectable_index<F>(cur: Option<usize>, count: usize, is_selectable: &F) -> Option<usize>
where
    F: Fn(usize) -> bool,
{
    cur.filter(|&c| c < count && is_selectable(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Predicate: every other index selectable, starting from index 1.
    // Layout: [no, YES, no, YES, no]
    fn alternating(i: usize) -> bool {
        i % 2 == 1
    }

    #[test]
    fn test_next_selectable_index_advances_to_first_selectable_after_cur() {
        assert_eq!(next_selectable_index(Some(1), 5, alternating), Some(3));
    }

    #[test]
    fn test_next_selectable_index_starting_from_none_returns_first_selectable() {
        assert_eq!(next_selectable_index(None, 5, alternating), Some(1));
    }

    #[test]
    fn test_next_selectable_index_clamps_at_last_selectable_when_no_following_exists() {
        // From index 3 (last selectable), next stays at 3.
        assert_eq!(next_selectable_index(Some(3), 5, alternating), Some(3));
    }

    #[test]
    fn test_next_selectable_index_returns_none_when_no_selectable_exists() {
        assert_eq!(next_selectable_index(Some(0), 5, |_| false), None);
    }

    #[test]
    fn test_prev_selectable_index_retreats_to_last_selectable_before_cur() {
        assert_eq!(prev_selectable_index(Some(3), 5, alternating), Some(1));
    }

    #[test]
    fn test_prev_selectable_index_clamps_at_first_selectable() {
        assert_eq!(prev_selectable_index(Some(1), 5, alternating), Some(1));
    }

    #[test]
    fn test_prev_selectable_index_returns_none_when_no_selectable_exists() {
        assert_eq!(prev_selectable_index(Some(4), 5, |_| false), None);
    }

    #[test]
    fn test_first_selectable_index_finds_first_match() {
        assert_eq!(first_selectable_index(5, alternating), Some(1));
    }

    #[test]
    fn test_first_selectable_index_returns_none_when_no_match() {
        assert_eq!(first_selectable_index(5, |_| false), None);
    }

    #[test]
    fn test_last_selectable_index_finds_last_match() {
        assert_eq!(last_selectable_index(5, alternating), Some(3));
    }

    #[test]
    fn test_empty_count_returns_none() {
        assert_eq!(next_selectable_index(None, 0, |_| true), None);
        assert_eq!(prev_selectable_index(None, 0, |_| true), None);
        assert_eq!(first_selectable_index(0, |_| true), None);
        assert_eq!(last_selectable_index(0, |_| true), None);
    }
}
