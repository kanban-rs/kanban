use serde::{Deserialize, Serialize};

/// Paginated list envelope returned by every collection `GET`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: u32,
    pub page_size: u32,
    pub total_pages: u32,
}

/// Pagination query parameters (`?page=&page_size=`), consumed by list handlers.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct PageParams {
    #[serde(default)]
    pub page: Option<u32>,
    #[serde(default)]
    pub page_size: Option<u32>,
}

impl<T> From<kanban_core::paginated_list::PaginatedList<T>> for Page<T> {
    fn from(list: kanban_core::paginated_list::PaginatedList<T>) -> Self {
        let kanban_core::paginated_list::PaginatedList {
            items,
            total,
            page,
            page_size,
            total_pages,
        } = list;
        Self {
            items,
            total,
            page: page as u32,
            page_size: page_size as u32,
            total_pages: total_pages as u32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_serde_round_trip() {
        let page = Page {
            items: vec!["a".to_string(), "b".to_string()],
            total: 2,
            page: 1,
            page_size: 50,
            total_pages: 1,
        };
        let json = serde_json::to_string(&page).unwrap();
        let back: Page<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(back, page);
    }

    #[test]
    fn test_page_params_defaults_to_none() {
        let params: PageParams = serde_json::from_str("{}").unwrap();
        assert_eq!(params, PageParams::default());
        assert_eq!(params.page, None);
        assert_eq!(params.page_size, None);
    }

    #[test]
    fn test_page_params_parses_both_fields() {
        let params: PageParams = serde_json::from_str(r#"{"page":2,"page_size":25}"#).unwrap();
        assert_eq!(params.page, Some(2));
        assert_eq!(params.page_size, Some(25));
    }

    #[test]
    fn test_paginated_list_converts_to_page_preserving_every_field() {
        let list = kanban_core::paginated_list::PaginatedList {
            items: vec!["a".to_string(), "b".to_string()],
            total: 7,
            page: 2,
            page_size: 3,
            total_pages: 3,
        };

        let page = Page::from(list);

        assert_eq!(page.items, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(page.total, 7);
        assert_eq!(page.page, 2u32);
        assert_eq!(page.page_size, 3u32);
        assert_eq!(page.total_pages, 3u32);
    }
}
