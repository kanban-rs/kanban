//! API-level pagination for CLI and MCP responses.
//!
//! [`PaginatedList<T>`] is the serialized response envelope returned by list
//! endpoints. It is distinct from [`super::viewport`], which manages TUI
//! viewport scroll state and is never serialized.

use crate::{CoreError, CoreResult};

/// Default page number (1-based).
pub const DEFAULT_PAGE: usize = 1;

/// Default number of items per page.
pub const DEFAULT_PAGE_SIZE: usize = 50;

/// Maximum allowed page size.
pub const MAX_PAGE_SIZE: usize = 500;

/// Resolve optional `page` and `page_size` CLI/MCP inputs to concrete values.
///
/// `None` falls back to [`DEFAULT_PAGE`] / [`DEFAULT_PAGE_SIZE`].
///
/// # Errors
///
/// Returns [`CoreError::Validation`] if the resolved `page` is 0, `page_size`
/// is 0, or `page_size` exceeds [`MAX_PAGE_SIZE`].
pub fn resolve_page_params(
    page: Option<u32>,
    page_size: Option<u32>,
) -> CoreResult<(usize, usize)> {
    let page = page.map(|p| p as usize).unwrap_or(DEFAULT_PAGE);
    let page_size = page_size.map(|p| p as usize).unwrap_or(DEFAULT_PAGE_SIZE);
    if page == 0 {
        return Err(CoreError::Validation(
            "page must be >= 1 (1-based)".to_string(),
        ));
    }
    if page_size == 0 {
        return Err(CoreError::Validation("page_size must be >= 1".to_string()));
    }
    if page_size > MAX_PAGE_SIZE {
        return Err(CoreError::Validation(format!(
            "page_size must be <= {MAX_PAGE_SIZE}"
        )));
    }
    Ok((page, page_size))
}

/// Paginated response envelope returned by CLI and MCP list endpoints.
///
/// All list commands (board, column, sprint, card) serialize to this shape:
///
/// ```json
/// { "items": [...], "total": 42, "page": 1, "page_size": 50, "total_pages": 1 }
/// ```
///
/// `total_pages` is 0 when the collection is empty.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PaginatedList<T> {
    pub items: Vec<T>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub total_pages: usize,
}

impl<T> PaginatedList<T> {
    /// Slice an already-loaded `Vec<T>` into a page window.
    ///
    /// This is an in-memory operation: the full dataset must be fetched from
    /// storage before calling this. `total` always reflects the unfiltered count.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Validation`] if `page` or `page_size` is 0.
    pub fn paginate(items: Vec<T>, page: usize, page_size: usize) -> CoreResult<Self> {
        if page == 0 {
            return Err(CoreError::Validation(
                "page must be >= 1 (1-based)".to_string(),
            ));
        }
        if page_size == 0 {
            return Err(CoreError::Validation("page_size must be >= 1".to_string()));
        }
        if page_size > MAX_PAGE_SIZE {
            return Err(CoreError::Validation(format!(
                "page_size must be <= {MAX_PAGE_SIZE}"
            )));
        }
        let total = items.len();
        let total_pages = total.div_ceil(page_size);
        let offset = (page - 1).saturating_mul(page_size);
        let items = items.into_iter().skip(offset).take(page_size).collect();
        Ok(Self {
            items,
            total,
            page,
            page_size,
            total_pages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paginate_normal() {
        let items: Vec<i32> = (1..=10).collect();
        let result = PaginatedList::paginate(items, 2, 3).unwrap();
        assert_eq!(result.items, vec![4, 5, 6]);
        assert_eq!(result.total, 10);
        assert_eq!(result.total_pages, 4);
        assert_eq!(result.page, 2);
    }

    #[test]
    fn test_paginate_empty() {
        let result = PaginatedList::<i32>::paginate(vec![], 1, 10).unwrap();
        assert_eq!(result.items, Vec::<i32>::new());
        assert_eq!(result.total, 0);
        assert_eq!(result.total_pages, 0);
        assert_eq!(result.page, 1);
    }

    #[test]
    fn test_paginate_page_zero_errors() {
        let items: Vec<i32> = (1..=5).collect();
        let err = PaginatedList::paginate(items, 0, 3).unwrap_err();
        assert!(err.to_string().contains("page must be >= 1"));
    }

    #[test]
    fn test_paginate_page_size_zero_errors() {
        let items: Vec<i32> = (1..=5).collect();
        let err = PaginatedList::paginate(items, 1, 0).unwrap_err();
        assert!(err.to_string().contains("page_size must be >= 1"));
    }

    #[test]
    fn test_paginate_out_of_bounds() {
        let items: Vec<i32> = (1..=5).collect();
        let result = PaginatedList::paginate(items, 3, 5).unwrap();
        assert_eq!(result.items, Vec::<i32>::new());
        assert_eq!(result.total, 5);
        assert_eq!(result.total_pages, 1);
    }

    #[test]
    fn test_paginate_fits_on_one_page() {
        let items: Vec<i32> = (1..=3).collect();
        let result = PaginatedList::paginate(items, 1, 10).unwrap();
        assert_eq!(result.items, vec![1, 2, 3]);
        assert_eq!(result.total_pages, 1);
    }

    #[test]
    fn test_paginate_page_size_too_large_errors() {
        let items: Vec<i32> = (1..=5).collect();
        let err = PaginatedList::paginate(items, 1, MAX_PAGE_SIZE + 1).unwrap_err();
        assert!(err.to_string().contains("page_size must be <="));
    }

    #[test]
    fn test_paginate_exact_boundary() {
        let items: Vec<i32> = (1..=9).collect();
        let result = PaginatedList::paginate(items, 3, 3).unwrap();
        assert_eq!(result.items, vec![7, 8, 9]);
        assert_eq!(result.total_pages, 3);
    }

    // resolve_page_params — base cases

    #[test]
    fn test_resolve_page_params_defaults() {
        let (page, page_size) = resolve_page_params(None, None).unwrap();
        assert_eq!(page, DEFAULT_PAGE);
        assert_eq!(page_size, DEFAULT_PAGE_SIZE);
    }

    #[test]
    fn test_resolve_page_params_explicit_values() {
        let (page, page_size) = resolve_page_params(Some(3), Some(25)).unwrap();
        assert_eq!(page, 3);
        assert_eq!(page_size, 25);
    }

    #[test]
    fn test_resolve_page_params_max_page_size() {
        let (page, page_size) = resolve_page_params(Some(1), Some(MAX_PAGE_SIZE as u32)).unwrap();
        assert_eq!(page, 1);
        assert_eq!(page_size, MAX_PAGE_SIZE);
    }

    // resolve_page_params — zero / out-of-range error cases

    #[test]
    fn test_resolve_page_params_page_zero_errors() {
        let err = resolve_page_params(Some(0), None).unwrap_err();
        assert!(err.to_string().contains("page must be >= 1"));
    }

    #[test]
    fn test_resolve_page_params_page_size_zero_errors() {
        let err = resolve_page_params(None, Some(0)).unwrap_err();
        assert!(err.to_string().contains("page_size must be >= 1"));
    }

    #[test]
    fn test_resolve_page_params_page_size_too_large_errors() {
        let err = resolve_page_params(None, Some(MAX_PAGE_SIZE as u32 + 1)).unwrap_err();
        assert!(err.to_string().contains("page_size must be <="));
    }

    // empty collection — total_pages must be 0, not 1

    #[test]
    fn test_paginate_empty_total_pages_is_zero() {
        // An empty collection has 0 pages (not 1). Callers looping
        // `for p in 1..=total_pages` correctly do zero iterations.
        let result = PaginatedList::<i32>::paginate(vec![], 1, 10).unwrap();
        assert_eq!(result.total, 0);
        assert_eq!(result.total_pages, 0);
        assert_eq!(result.items, Vec::<i32>::new());
    }
}
