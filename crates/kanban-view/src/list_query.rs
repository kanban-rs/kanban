pub fn search_and_sort<T>(
    items: Vec<T>,
    matches: impl Fn(&T) -> bool,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) -> Vec<T> {
    let mut filtered: Vec<T> = items.into_iter().filter(matches).collect();
    filtered.sort_by(compare);
    filtered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq)]
    struct Item {
        name: &'static str,
    }

    fn item(name: &'static str) -> Item {
        Item { name }
    }

    #[test]
    fn test_search_and_sort_filters_non_matching_items() {
        let items = vec![item("apple"), item("banana"), item("apricot")];

        let result = search_and_sort(
            items,
            |i| i.name.starts_with('a'),
            |a, b| a.name.cmp(b.name),
        );

        assert_eq!(result, vec![item("apple"), item("apricot")]);
    }

    #[test]
    fn test_search_and_sort_orders_by_comparator() {
        let items = vec![item("banana"), item("apple"), item("cherry")];

        let result = search_and_sort(items, |_| true, |a, b| a.name.cmp(b.name));

        assert_eq!(result, vec![item("apple"), item("banana"), item("cherry")]);
    }

    #[test]
    fn test_search_and_sort_empty_input_returns_empty_output() {
        let items: Vec<Item> = vec![];

        let result = search_and_sort(items, |_| true, |a, b| a.name.cmp(b.name));

        assert!(result.is_empty());
    }
}
