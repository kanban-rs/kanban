//! Card search functionality.
//!
//! Provides traits and implementations for searching cards by various criteria.
//! Used by both TUI and API for consistent search behavior.

use crate::{Board, Card, Column, Sprint};
use uuid::Uuid;

/// Trait for searching cards by various criteria.
pub trait CardSearcher {
    /// Returns true if the card matches the search criteria.
    fn matches(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> bool;
}

/// Search cards by title (case-insensitive).
pub struct TitleSearcher {
    query: String,
}

impl TitleSearcher {
    /// Create a new title searcher with the given query.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into().to_lowercase(),
        }
    }

    /// Get the search query.
    pub fn query(&self) -> &str {
        &self.query
    }
}

impl CardSearcher for TitleSearcher {
    fn matches(&self, card: &Card, _board: &Board, _sprints: &[Sprint]) -> bool {
        if self.query.is_empty() {
            return true;
        }
        card.title.to_lowercase().contains(&self.query)
    }
}

/// Search cards by generated branch name (case-insensitive).
pub struct BranchNameSearcher {
    query: String,
}

impl BranchNameSearcher {
    /// Create a new branch name searcher with the given query.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into().to_lowercase(),
        }
    }

    /// Get the search query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Generate the branch name for a card.
    fn get_branch_name(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> String {
        let sprint_prefix = card
            .sprint_id
            .and_then(|sid| sprints.iter().find(|s| s.id == sid))
            .map(|sprint| {
                format!(
                    "{}-{}/",
                    sprint.effective_prefix(board, "sprint"),
                    sprint.sprint_number
                )
            });

        let card_number = card.card_number;
        let title_slug = card
            .title
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .split('-')
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");

        let branch_prefix = board
            .sprint_prefix
            .as_deref()
            .unwrap_or_else(|| board.effective_branch_prefix("feature"));

        if let Some(prefix) = sprint_prefix {
            format!("{}{}{}-{}", prefix, branch_prefix, card_number, title_slug)
        } else {
            format!("{}{}-{}", branch_prefix, card_number, title_slug)
        }
    }
}

impl CardSearcher for BranchNameSearcher {
    fn matches(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> bool {
        if self.query.is_empty() {
            return true;
        }
        let branch_name = self.get_branch_name(card, board, sprints);
        branch_name.to_lowercase().contains(&self.query)
    }
}

/// Search cards by card identifier (e.g. "KAN-164", "164").
pub struct CardIdentifierSearcher {
    query: String,
}

impl CardIdentifierSearcher {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into().to_lowercase(),
        }
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    fn get_identifier(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> String {
        // The card's STORED prefix. Deriving it here would search on the
        // board's CURRENT prefix, so after a rename typing a card's real
        // identifier would not find it.
        //
        // Derivation remains only for a card written before the prefix was
        // stored and not yet migrated.
        let prefix = if card.prefix.is_empty() {
            let sprint_prefix = card
                .sprint_id
                .and_then(|sid| sprints.iter().find(|s| s.id == sid))
                .and_then(|s| s.card_prefix.as_deref());
            sprint_prefix
                .or(board.card_prefix.as_deref())
                .unwrap_or("task")
        } else {
            &card.prefix
        };
        format!("{}-{}", prefix, card.card_number).to_lowercase()
    }
}

impl CardSearcher for CardIdentifierSearcher {
    fn matches(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> bool {
        if self.query.is_empty() {
            return true;
        }
        self.get_identifier(card, board, sprints)
            .contains(&self.query)
    }
}

/// Enum dispatch for searching cards by a specific field.
pub enum SearchBy {
    Title(TitleSearcher),
    BranchName(BranchNameSearcher),
    CardIdentifier(CardIdentifierSearcher),
}

impl SearchBy {
    fn matches(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> bool {
        match self {
            Self::Title(s) => s.matches(card, board, sprints),
            Self::BranchName(s) => s.matches(card, board, sprints),
            Self::CardIdentifier(s) => s.matches(card, board, sprints),
        }
    }
}

/// Composite searcher that matches if any sub-searcher matches.
///
/// By default, includes both title and branch name searchers.
pub struct CompositeSearcher {
    searchers: Vec<SearchBy>,
}

impl CompositeSearcher {
    /// Create an empty composite searcher (matches all cards).
    pub fn new() -> Self {
        Self {
            searchers: Vec::new(),
        }
    }

    /// Create a composite searcher with all built-in searchers.
    pub fn all(query: impl Into<String>) -> Self {
        let query = query.into();
        Self {
            searchers: vec![
                SearchBy::Title(TitleSearcher::new(query.clone())),
                SearchBy::BranchName(BranchNameSearcher::new(query.clone())),
                SearchBy::CardIdentifier(CardIdentifierSearcher::new(query)),
            ],
        }
    }

    /// Add a searcher to the composite (builder pattern).
    pub fn with_search(mut self, searcher: SearchBy) -> Self {
        self.searchers.push(searcher);
        self
    }
}

impl Default for CompositeSearcher {
    fn default() -> Self {
        Self::new()
    }
}

/// A parsed card identifier: `"KAN-5"` or a bare `"5"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParsedIdentifier {
    PrefixAndNumber { prefix: String, number: u32 },
    NumberOnly(u32),
}

pub fn parse_identifier(identifier: &str) -> Option<ParsedIdentifier> {
    let lower = identifier.to_lowercase();
    if let Some(dash_pos) = lower.rfind('-') {
        let prefix = &lower[..dash_pos];
        if prefix.is_empty() {
            return None;
        }
        let number_str = &lower[dash_pos + 1..];
        let number = number_str.parse::<u32>().ok()?;
        Some(ParsedIdentifier::PrefixAndNumber {
            prefix: prefix.to_string(),
            number,
        })
    } else if let Ok(number) = lower.parse::<u32>() {
        Some(ParsedIdentifier::NumberOnly(number))
    } else {
        None
    }
}

/// Find all cards matching an identifier (e.g. `"KAN-5"` or `"5"`), searching across all boards.
///
/// - `"PREFIX-N"`: returns all cards whose resolved prefix equals `PREFIX` (case-insensitive)
///   and whose `card_number` equals `N`. Prefix resolution follows:
///   `sprint.card_prefix → board.card_prefix → "task"` (boards with no configured prefix
///   are addressed as "task-N", matching the display behaviour of `CardIdentifierSearcher`).
/// - `"N"` (bare number): returns all cards with `card_number == N` regardless of board.
/// - Returns an empty `Vec` if the identifier cannot be parsed or no cards match.
pub fn find_cards_by_identifier<'a>(
    identifier: &str,
    cards: &'a [Card],
    columns: &[Column],
    boards: &[Board],
    sprints: &[Sprint],
) -> Vec<&'a Card> {
    let Some(parsed) = parse_identifier(identifier) else {
        return vec![];
    };
    cards
        .iter()
        .filter(|card| match &parsed {
            ParsedIdentifier::PrefixAndNumber { prefix, number } => {
                // A card whose column resolves to no board is unaddressable by
                // prefix, as before: `resolve_card_prefix` would hand back the
                // default and match cards that never belonged to it.
                let has_board = columns
                    .iter()
                    .find(|col| col.id == card.column_id)
                    .is_some_and(|col| boards.iter().any(|b| b.id == col.board_id));
                has_board
                    && crate::prefix::Prefix::normalize(&resolve_card_prefix(
                        card, columns, boards, sprints, "task",
                    )) == *prefix
                    && card.card_number == *number
            }
            ParsedIdentifier::NumberOnly(number) => card.card_number == *number,
        })
        .collect()
}

/// The prefix a card is addressed by TODAY, resolved dynamically through
/// `sprint.card_prefix`, then `board.card_prefix`, then the default, and
/// normalised.
///
/// The board is reached through `card.column_id -> column.board_id`, NOT
/// through `card.board_id`. Those can disagree, and this deliberately follows
/// the column: it is what the identifier reader does, so it is what users see.
/// The migration that freezes prefixes onto cards calls THIS function, so the
/// frozen value cannot drift from the value it is meant to preserve.
pub fn resolve_card_prefix(
    card: &Card,
    columns: &[Column],
    boards: &[Board],
    sprints: &[Sprint],
    default_card_prefix: &str,
) -> String {
    resolve_card_prefix_by_ids(
        card.column_id,
        card.sprint_id,
        &columns
            .iter()
            .map(|c| (c.id, c.board_id))
            .collect::<Vec<_>>(),
        &boards
            .iter()
            .map(|b| (b.id, b.card_prefix.clone()))
            .collect::<Vec<_>>(),
        &sprints
            .iter()
            .map(|s| (s.id, s.card_prefix.clone()))
            .collect::<Vec<_>>(),
        default_card_prefix,
    )
}

/// The same rule over bare ids, for callers that cannot build domain structs.
///
/// Migrations are exactly that: they read files written before `Card`,
/// `Board` and `Sprint` had fields those structs now require, so they must
/// project off the raw record. Without this they would each reimplement the
/// rule, and the SQLite and JSON backfills reimplementing one rule is how they
/// silently disagreed earlier in this epic.
pub fn resolve_card_prefix_by_ids(
    column_id: Uuid,
    sprint_id: Option<Uuid>,
    columns: &[(Uuid, Uuid)],
    boards: &[(Uuid, Option<String>)],
    sprints: &[(Uuid, Option<String>)],
    default_card_prefix: &str,
) -> String {
    let from_sprint = sprint_id.and_then(|sid| {
        sprints
            .iter()
            .find(|(id, _)| *id == sid)
            .and_then(|(_, p)| p.clone())
    });
    let from_board = columns
        .iter()
        .find(|(id, _)| *id == column_id)
        .and_then(|(_, board_id)| {
            boards
                .iter()
                .find(|(id, _)| id == board_id)
                .and_then(|(_, p)| p.clone())
        });

    from_sprint
        .or(from_board)
        .unwrap_or_else(|| default_card_prefix.to_string())
}

/// Format an error message listing ambiguous card matches.
///
/// Used by both CLI and MCP when an identifier resolves to multiple cards.
pub fn format_ambiguous_matches(identifier: &str, cards: &[Card]) -> String {
    format!(
        "Ambiguous identifier '{}': {} cards match. Use a UUID to be specific.\n{}",
        identifier,
        cards.len(),
        cards
            .iter()
            .map(|c| format!("  {} — {}", c.id, c.title))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// Find all boards whose `name` equals `query` (case-insensitive).
pub fn find_boards_by_name<'a>(query: &str, boards: &'a [Board]) -> Vec<&'a Board> {
    let needle = query.to_lowercase();
    boards
        .iter()
        .filter(|b| b.name.to_lowercase() == needle)
        .collect()
}

/// Find all columns in the given slice whose `name` equals `query` (case-insensitive).
///
/// The caller is responsible for scoping `columns` (e.g. filtering by `board_id` first).
pub fn find_columns_by_name<'a>(query: &str, columns: &'a [Column]) -> Vec<&'a Column> {
    let needle = query.to_lowercase();
    columns
        .iter()
        .filter(|c| c.name.to_lowercase() == needle)
        .collect()
}

/// Find sprints matching `query` (number or name) **within a single board**.
///
/// - If `query` parses as a `u32`, match sprints whose `board_id == board.id`
///   and `sprint_number == query`.
/// - Otherwise, match sprints on this board whose resolved name (via
///   `board.sprint_names[name_index]`) equals `query` (case-insensitive).
///
/// The board-scoped contract is enforced on both branches, so callers don't
/// need to pre-filter the slice.
pub fn find_sprints_by_query_on_board<'a>(
    query: &str,
    sprints: &'a [Sprint],
    board: &Board,
) -> Vec<&'a Sprint> {
    if let Ok(number) = query.parse::<u32>() {
        return sprints
            .iter()
            .filter(|s| s.board_id == board.id && s.sprint_number == number)
            .collect();
    }
    let needle = query.to_lowercase();
    sprints
        .iter()
        .filter(|s| {
            s.board_id == board.id
                && s.get_name(board)
                    .map(|n| n.to_lowercase() == needle)
                    .unwrap_or(false)
        })
        .collect()
}

/// Find sprints matching `query` (number or name) **across all the given boards**.
///
/// - If `query` parses as a `u32`, match every sprint whose `sprint_number == query`,
///   regardless of board (caller decides what to do with multi-board ambiguity).
/// - Otherwise, match sprints whose resolved name on their own board equals
///   `query` (case-insensitive). The sprint's `board_id` is looked up in `boards`;
///   a sprint whose board is missing from the slice is silently skipped.
pub fn find_sprints_by_query_global<'a>(
    query: &str,
    sprints: &'a [Sprint],
    boards: &[Board],
) -> Vec<&'a Sprint> {
    if let Ok(number) = query.parse::<u32>() {
        return sprints
            .iter()
            .filter(|s| s.sprint_number == number)
            .collect();
    }
    let needle = query.to_lowercase();
    sprints
        .iter()
        .filter(|s| {
            let Some(board) = boards.iter().find(|b| b.id == s.board_id) else {
                return false;
            };
            s.get_name(board)
                .map(|n| n.to_lowercase() == needle)
                .unwrap_or(false)
        })
        .collect()
}

impl CardSearcher for CompositeSearcher {
    fn matches(&self, card: &Card, board: &Board, sprints: &[Sprint]) -> bool {
        if self.searchers.is_empty() {
            return true;
        }
        self.searchers
            .iter()
            .any(|searcher| searcher.matches(card, board, sprints))
    }
}

/// Trait for context-free substring search over a single entity.
pub trait Searcher<T> {
    /// Returns true if `item` matches the search criteria.
    fn matches(&self, item: &T) -> bool;
}

/// Matches a case-insensitive substring against a field extracted by a closure.
pub struct FieldSearcher<T, F: Fn(&T) -> &str> {
    query: String,
    field: F,
    _marker: std::marker::PhantomData<T>,
}

impl<T, F: Fn(&T) -> &str> FieldSearcher<T, F> {
    pub fn new(query: impl Into<String>, field: F) -> Self {
        Self {
            query: query.into().to_lowercase(),
            field,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<T, F: Fn(&T) -> &str> Searcher<T> for FieldSearcher<T, F> {
    fn matches(&self, item: &T) -> bool {
        if self.query.is_empty() {
            return true;
        }
        (self.field)(item).to_lowercase().contains(&self.query)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn create_test_card(board: &mut Board, title: &str) -> Card {
        let column = crate::Column::new(board.id, "Todo", 0);
        Card::new(board, column.id, title.to_string(), 0)
    }

    #[test]
    fn test_title_searcher_matches() {
        let mut board = Board::new("Test", None::<String>);
        let card = create_test_card(&mut board, "Fix authentication bug");

        let searcher = TitleSearcher::new("auth");
        assert!(searcher.matches(&card, &board, &[]));

        let searcher = TitleSearcher::new("AUTH"); // case insensitive
        assert!(searcher.matches(&card, &board, &[]));

        let searcher = TitleSearcher::new("database");
        assert!(!searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_title_searcher_empty_query() {
        let mut board = Board::new("Test", None::<String>);
        let card = create_test_card(&mut board, "Any card");

        let searcher = TitleSearcher::new("");
        assert!(searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_branch_name_searcher_matches() {
        let mut board = Board::new("Test", None::<String>);
        let card = create_test_card(&mut board, "Add new feature");

        let searcher = BranchNameSearcher::new("feature");
        assert!(searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_composite_searcher_any_match() {
        let mut board = Board::new("Test", None::<String>);
        let card = create_test_card(&mut board, "Fix bug");

        // Should match because title contains "bug"
        let searcher = CompositeSearcher::all("bug");
        assert!(searcher.matches(&card, &board, &[]));

        // Should match because branch name contains "feature" prefix
        let searcher = CompositeSearcher::all("feature");
        assert!(searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_composite_searcher_empty() {
        let mut board = Board::new("Test", None::<String>);
        let card = create_test_card(&mut board, "Any card");

        let searcher = CompositeSearcher::new();
        assert!(searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_card_identifier_searcher_with_prefix() {
        let mut board = Board::new("Test", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);

        let searcher = CardIdentifierSearcher::new("KAN-1");
        assert!(searcher.matches(&card, &board, &[]));

        let searcher = CardIdentifierSearcher::new("kan-1");
        assert!(searcher.matches(&card, &board, &[]));

        let searcher = CardIdentifierSearcher::new("1");
        assert!(searcher.matches(&card, &board, &[]));

        let searcher = CardIdentifierSearcher::new("MVP-1");
        assert!(!searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_card_identifier_searcher_number_only() {
        let mut board = Board::new("Test", None::<String>);
        let column = crate::Column::new(board.id, "Todo", 0);
        let _card1 = Card::new(&mut board, column.id, "First", 0);
        let card2 = Card::new(&mut board, column.id, "Second", 1);

        // card2 has card_number=2
        let searcher = CardIdentifierSearcher::new("2");
        assert!(searcher.matches(&card2, &board, &[]));

        let searcher = CardIdentifierSearcher::new("task-2");
        assert!(searcher.matches(&card2, &board, &[]));
    }

    #[test]
    fn test_find_cards_by_identifier_empty_cards_returns_empty() {
        assert!(find_cards_by_identifier("KAN-1", &[], &[], &[], &[]).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_unparseable_identifier_returns_empty() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card];
        assert!(find_cards_by_identifier("", &cards, &columns, &boards, &[]).is_empty());
        assert!(find_cards_by_identifier("---", &cards, &columns, &boards, &[]).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_single_prefix_match_returns_one() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];

        let result = find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card.id);
    }

    #[test]
    fn test_find_cards_by_identifier_ambiguous_prefix_returns_both() {
        let mut board1 = Board::new("Board One", None::<String>);
        board1.card_prefix = Some("KAN".to_string());
        let col1 = crate::Column::new(board1.id, "Todo", 0);
        let card1 = Card::new(&mut board1, col1.id, "First", 0);

        let mut board2 = Board::new("Board Two", None::<String>);
        board2.card_prefix = Some("KAN".to_string());
        let col2 = crate::Column::new(board2.id, "Todo", 0);
        let card2 = Card::new(&mut board2, col2.id, "Second", 0);

        let boards = vec![board1, board2];
        let columns = vec![col1, col2];
        let cards = vec![card1.clone(), card2.clone()];

        let result = find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_cards_by_identifier_single_bare_number_returns_one() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];

        let result = find_cards_by_identifier("1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card.id);
    }

    #[test]
    fn test_find_cards_by_identifier_ambiguous_bare_number_returns_both() {
        let mut board1 = Board::new("Board One", None::<String>);
        board1.card_prefix = Some("AAA".to_string());
        let col1 = crate::Column::new(board1.id, "Todo", 0);
        let card1 = Card::new(&mut board1, col1.id, "First", 0);

        let mut board2 = Board::new("Board Two", None::<String>);
        board2.card_prefix = Some("BBB".to_string());
        let col2 = crate::Column::new(board2.id, "Todo", 0);
        let card2 = Card::new(&mut board2, col2.id, "Second", 0);

        let boards = vec![board1, board2];
        let columns = vec![col1, col2];
        let cards = vec![card1.clone(), card2.clone()];

        let result = find_cards_by_identifier("1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_cards_by_identifier_no_match_returns_empty() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card];

        assert!(find_cards_by_identifier("KAN-99", &cards, &columns, &boards, &[]).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_ambiguous_sprint_prefix_returns_both() {
        let mut board_a = Board::new("Board A", None::<String>);
        board_a.card_prefix = Some("PROJ".to_string());
        let col_a = crate::Column::new(board_a.id, "Todo", 0);
        let card_a = Card::new(&mut board_a, col_a.id, "Card A", 0);

        let mut board_b = Board::new("Board B", None::<String>);
        let col_b = crate::Column::new(board_b.id, "Todo", 0);
        let mut sprint = crate::Sprint::new(board_b.id, 1, None, None::<String>);
        sprint.card_prefix = Some("PROJ".to_string());
        let mut card_b = Card::new(&mut board_b, col_b.id, "Card B", 0);
        card_b.sprint_id = Some(sprint.id);

        let boards = vec![board_a, board_b];
        let columns = vec![col_a, col_b];
        let cards = vec![card_a.clone(), card_b.clone()];
        let sprints = vec![sprint];

        let result = find_cards_by_identifier("PROJ-1", &cards, &columns, &boards, &sprints);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_cards_by_identifier_prefix_format() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];

        assert_eq!(
            find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &[])
                .first()
                .map(|c| c.id),
            Some(card.id)
        );
        assert_eq!(
            find_cards_by_identifier("kan-1", &cards, &columns, &boards, &[])
                .first()
                .map(|c| c.id),
            Some(card.id)
        );
    }

    #[test]
    fn test_find_cards_by_identifier_number_only() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];

        assert_eq!(
            find_cards_by_identifier("1", &cards, &columns, &boards, &[])
                .first()
                .map(|c| c.id),
            Some(card.id)
        );
    }

    #[test]
    fn test_find_cards_by_identifier_not_found() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Some task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card];

        assert!(find_cards_by_identifier("KAN-99", &cards, &columns, &boards, &[]).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_second_board() {
        let mut board1 = Board::new("Board One", None::<String>);
        board1.card_prefix = Some("AAA".to_string());
        let col1 = crate::Column::new(board1.id, "Todo", 0);
        let card1 = Card::new(&mut board1, col1.id, "First", 0);

        let mut board2 = Board::new("Board Two", None::<String>);
        board2.card_prefix = Some("BBB".to_string());
        let col2 = crate::Column::new(board2.id, "Todo", 0);
        let card2 = Card::new(&mut board2, col2.id, "Second", 0);

        let boards = vec![board1, board2];
        let columns = vec![col1, col2];
        let cards = vec![card1, card2.clone()];

        assert_eq!(
            find_cards_by_identifier("BBB-1", &cards, &columns, &boards, &[])
                .first()
                .map(|c| c.id),
            Some(card2.id)
        );
    }

    #[test]
    fn test_find_cards_by_identifier_no_prefix_collision() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let mut card1 = Card::new(&mut board, column.id, "First", 0);
        card1.card_number = 1;
        let mut card11 = Card::new(&mut board, column.id, "Eleventh", 0);
        card11.card_number = 11;
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card1.clone(), card11];

        let result = find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card1.id);
    }

    #[test]
    fn test_find_cards_by_identifier_exact_number() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let mut card11 = Card::new(&mut board, column.id, "Eleven", 0);
        card11.card_number = 11;
        let mut card111 = Card::new(&mut board, column.id, "OneHundredEleven", 0);
        card111.card_number = 111;
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card11.clone(), card111];

        let result = find_cards_by_identifier("KAN-11", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card11.id);
    }

    #[test]
    fn test_find_cards_by_identifier_sprint_prefix() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let mut sprint = crate::Sprint::new(board.id, 1, None, None::<String>);
        sprint.card_prefix = Some("SP".to_string());
        let mut card = Card::new(&mut board, column.id, "Sprint task", 0);
        card.sprint_id = Some(sprint.id);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];
        let sprints = vec![sprint];

        let result = find_cards_by_identifier("SP-1", &cards, &columns, &boards, &sprints);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card.id);
        assert!(find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &sprints).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_sprint_prefix_overrides_board() {
        let mut board = Board::new("Project", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let mut sprint = crate::Sprint::new(board.id, 1, None, None::<String>);
        sprint.card_prefix = Some("SP".to_string());
        let mut card = Card::new(&mut board, column.id, "Override task", 0);
        card.sprint_id = Some(sprint.id);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];
        let sprints = vec![sprint];

        let result = find_cards_by_identifier("SP-1", &cards, &columns, &boards, &sprints);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card.id);
        assert!(find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &sprints).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_no_prefix_no_match() {
        let mut board = Board::new("Project", None::<String>);
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "No prefix task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];

        let result = find_cards_by_identifier("task-1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, card.id);
        assert!(find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &[]).is_empty());
    }

    #[test]
    fn test_find_cards_by_identifier_uses_task_fallback_for_no_prefix_board() {
        let mut board = Board::new("Project", None::<String>);
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "task", 0);
        let boards = vec![board];
        let columns = vec![column];
        let cards = vec![card.clone()];

        let result = find_cards_by_identifier("task-1", &cards, &columns, &boards, &[]);
        assert_eq!(result.len(), 1, "no-prefix board should match 'task-N'");
        assert_eq!(result[0].id, card.id);

        assert!(find_cards_by_identifier("task-99", &cards, &columns, &boards, &[]).is_empty());
        assert!(find_cards_by_identifier("KAN-1", &cards, &columns, &boards, &[]).is_empty());
    }

    #[test]
    fn test_composite_searcher_matches_by_identifier() {
        let mut board = Board::new("Test", None::<String>);
        board.card_prefix = Some("KAN".to_string());
        let column = crate::Column::new(board.id, "Todo", 0);
        let card = Card::new(&mut board, column.id, "Unrelated title", 0);

        // Title doesn't contain "KAN-1", but identifier does
        let searcher = CompositeSearcher::all("KAN-1");
        assert!(searcher.matches(&card, &board, &[]));
    }

    #[test]
    fn test_parse_identifier_invalid_inputs() {
        let boards = vec![];
        let columns = vec![];
        let cards = vec![];
        assert!(find_cards_by_identifier("", &cards, &columns, &boards, &[]).is_empty());
        assert!(find_cards_by_identifier("KAN-", &cards, &columns, &boards, &[]).is_empty());
        assert!(find_cards_by_identifier("KAN-abc", &cards, &columns, &boards, &[]).is_empty());
        assert!(find_cards_by_identifier("-5", &cards, &columns, &boards, &[]).is_empty());
        assert!(
            find_cards_by_identifier("not-a-number", &cards, &columns, &boards, &[]).is_empty()
        );
    }

    #[test]
    fn test_find_boards_by_name_case_insensitive_match() {
        let boards = vec![
            Board::new("Kanban", None::<String>),
            Board::new("Personal", None::<String>),
        ];
        let result = find_boards_by_name("kanban", &boards);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Kanban");

        let result = find_boards_by_name("KANBAN", &boards);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Kanban");
    }

    #[test]
    fn test_find_boards_by_name_no_match_returns_empty() {
        let boards = vec![Board::new("Kanban", None::<String>)];
        assert!(find_boards_by_name("missing", &boards).is_empty());
    }

    #[test]
    fn test_find_boards_by_name_multiple_with_same_name_returns_all() {
        let boards = vec![
            Board::new("Shared", None::<String>),
            Board::new("Shared", None::<String>),
        ];
        let result = find_boards_by_name("shared", &boards);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_columns_by_name_case_insensitive_match() {
        let board_id = Uuid::new_v4();
        let columns = vec![
            Column::new(board_id, "TODO", 0),
            Column::new(board_id, "Doing", 1),
            Column::new(board_id, "Done", 2),
        ];
        let result = find_columns_by_name("todo", &columns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "TODO");

        let result = find_columns_by_name("DOING", &columns);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].name, "Doing");
    }

    #[test]
    fn test_find_columns_by_name_returns_multiple_when_same_name_across_boards() {
        let columns = vec![
            Column::new(Uuid::new_v4(), "TODO", 0),
            Column::new(Uuid::new_v4(), "TODO", 0),
        ];
        let result = find_columns_by_name("todo", &columns);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_find_columns_by_name_no_match_returns_empty() {
        let columns = vec![Column::new(Uuid::new_v4(), "TODO", 0)];
        assert!(find_columns_by_name("missing", &columns).is_empty());
    }

    #[test]
    fn test_find_sprints_by_query_matches_number() {
        let mut board = Board::new("B", None::<String>);
        board.sprint_names.push("alpha".to_string());
        let mut sprint1 = crate::Sprint::new(board.id, 1, Some(0), None::<String>);
        sprint1.sprint_number = 1;
        let mut sprint2 = crate::Sprint::new(board.id, 2, None, None::<String>);
        sprint2.sprint_number = 2;
        let sprints = vec![sprint1, sprint2];
        let boards = vec![board];

        let result = find_sprints_by_query_global("1", &sprints, &boards);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].sprint_number, 1);
    }

    #[test]
    fn test_find_sprints_by_query_matches_name_case_insensitive() {
        let mut board = Board::new("B", None::<String>);
        board.sprint_names.push("yarara-release".to_string());
        board.sprint_names.push("moonshot".to_string());
        let sprint1 = crate::Sprint::new(board.id, 1, Some(0), None::<String>);
        let sprint2 = crate::Sprint::new(board.id, 2, Some(1), None::<String>);
        let sprints = vec![sprint1.clone(), sprint2];
        let boards = vec![board];

        let result = find_sprints_by_query_global("yarara-release", &sprints, &boards);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, sprint1.id);

        let result = find_sprints_by_query_global("YARARA-RELEASE", &sprints, &boards);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, sprint1.id);
    }

    #[test]
    fn test_find_sprints_by_query_number_takes_priority_over_name() {
        let mut board = Board::new("B", None::<String>);
        board.sprint_names.push("1".to_string());
        let sprint_numbered = crate::Sprint::new(board.id, 1, None, None::<String>);
        let sprint_named = crate::Sprint::new(board.id, 99, Some(0), None::<String>);
        let sprints = vec![sprint_numbered.clone(), sprint_named];
        let boards = vec![board];

        let result = find_sprints_by_query_global("1", &sprints, &boards);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, sprint_numbered.id);
    }

    #[test]
    fn test_find_sprints_by_query_no_match_returns_empty() {
        let mut board = Board::new("B", None::<String>);
        board.sprint_names.push("alpha".to_string());
        let sprint = crate::Sprint::new(board.id, 1, Some(0), None::<String>);
        let sprints = vec![sprint];
        let boards = vec![board];

        assert!(find_sprints_by_query_global("nope", &sprints, &boards).is_empty());
        assert!(find_sprints_by_query_global("99", &sprints, &boards).is_empty());
    }

    #[test]
    fn test_find_sprints_by_query_ambiguous_number_across_boards_returns_all() {
        let board_a = Board::new("A", None::<String>);
        let board_b = Board::new("B", None::<String>);
        let sprint_a = crate::Sprint::new(board_a.id, 13, None, None::<String>);
        let sprint_b = crate::Sprint::new(board_b.id, 13, None, None::<String>);
        let sprints = vec![sprint_a, sprint_b];
        let boards = vec![board_a, board_b];

        let result = find_sprints_by_query_global("13", &sprints, &boards);
        assert_eq!(result.len(), 2);
    }

    // ---- find_sprints_by_query_on_board scoping tests (KAN-400 footgun fix) ----

    #[test]
    fn test_find_sprints_by_query_on_board_number_matches_only_on_that_board() {
        let board_a = Board::new("A", None::<String>);
        let board_b = Board::new("B", None::<String>);
        let on_a = crate::Sprint::new(board_a.id, 13, None, None::<String>);
        let on_b = crate::Sprint::new(board_b.id, 13, None, None::<String>);
        let sprints = vec![on_a.clone(), on_b];

        let result = find_sprints_by_query_on_board("13", &sprints, &board_a);
        assert_eq!(result.len(), 1, "expected exactly one match on board A");
        assert_eq!(result[0].id, on_a.id);
    }

    #[test]
    fn test_find_sprints_by_query_on_board_name_matches_only_on_that_board() {
        let mut board_a = Board::new("A", None::<String>);
        board_a.sprint_names.push("alpha".to_string());
        let mut board_b = Board::new("B", None::<String>);
        board_b.sprint_names.push("alpha".to_string());
        let on_a = crate::Sprint::new(board_a.id, 1, Some(0), None::<String>);
        let on_b = crate::Sprint::new(board_b.id, 1, Some(0), None::<String>);
        let sprints = vec![on_a.clone(), on_b];

        let result = find_sprints_by_query_on_board("alpha", &sprints, &board_a);
        assert_eq!(result.len(), 1, "expected exactly one match on board A");
        assert_eq!(result[0].id, on_a.id);
    }

    #[test]
    fn test_find_sprints_by_query_on_board_ignores_other_board_sprints_in_slice() {
        // Footgun-regression: even if a caller passes sprints from a different
        // board, the on_board variant filters them out.
        let board_a = Board::new("A", None::<String>);
        let board_b = Board::new("B", None::<String>);
        let only_on_b = crate::Sprint::new(board_b.id, 13, None, None::<String>);
        let sprints = vec![only_on_b];

        let result = find_sprints_by_query_on_board("13", &sprints, &board_a);
        assert!(result.is_empty());
    }

    // ---- FieldSearcher<T, F> tests ----

    struct Widget {
        name: String,
    }

    #[test]
    fn test_field_searcher_matches_case_insensitive_substring() {
        let widget = Widget {
            name: "In Progress".to_string(),
        };
        let searcher = FieldSearcher::new("progress", |w: &Widget| w.name.as_str());
        assert!(searcher.matches(&widget));

        let searcher = FieldSearcher::new("PROGRESS", |w: &Widget| w.name.as_str());
        assert!(searcher.matches(&widget));
    }

    #[test]
    fn test_field_searcher_empty_query_matches_everything() {
        let widget = Widget {
            name: "Anything".to_string(),
        };
        let searcher = FieldSearcher::new("", |w: &Widget| w.name.as_str());
        assert!(searcher.matches(&widget));
    }

    #[test]
    fn test_field_searcher_no_match_returns_false() {
        let widget = Widget {
            name: "Done".to_string(),
        };
        let searcher = FieldSearcher::new("todo", |w: &Widget| w.name.as_str());
        assert!(!searcher.matches(&widget));
    }

    #[test]
    fn test_field_searcher_matches_substring_anywhere_in_field() {
        let widget = Widget {
            name: "Code Review".to_string(),
        };
        let searcher = FieldSearcher::new("review", |w: &Widget| w.name.as_str());
        assert!(searcher.matches(&widget));

        let searcher = FieldSearcher::new("de rev", |w: &Widget| w.name.as_str());
        assert!(searcher.matches(&widget));
    }
}

#[cfg(test)]
mod resolve_card_prefix_tests {
    use super::*;
    use crate::{Board, Card, Column, Sprint};

    fn board(prefix: Option<&str>) -> Board {
        Board::new("b".to_string(), prefix)
    }

    fn column(board_id: uuid::Uuid) -> Column {
        Column::new(board_id, "c".to_string(), 0)
    }

    fn card(column_id: uuid::Uuid, board_id: uuid::Uuid, number: u32) -> Card {
        let mut owner = Board::new("owner".to_string(), None::<String>);
        owner.id = board_id;
        let mut c = Card::new(&mut owner, column_id, "t", 0);
        c.board_id = board_id;
        c.card_number = number;
        c
    }

    /// Search must match on the card's STORED identifier. Deriving it would
    /// search on the board's CURRENT prefix, so after a rename typing a card's
    /// real identifier would not find it -- and typing an identifier no card
    /// has would.
    #[test]
    fn test_identifier_search_matches_the_stored_prefix_not_the_boards_current_one() {
        let mut board = Board::new("b".to_string(), Some("KAN"));
        let col = Column::new(board.id, "c".to_string(), 0);
        let mut card = Card::new(&mut board, col.id, "t", 0);
        card.card_number = 5;
        card.prefix = "KAN".to_string();

        // The board is renamed after the card was minted.
        board.card_prefix = Some("DEV".to_string());

        let searcher = CardIdentifierSearcher::new("kan-5".to_string());
        assert!(
            searcher.matches(&card, &board, &[]),
            "the card still answers to the identifier it was minted with"
        );

        let searcher = CardIdentifierSearcher::new("dev-5".to_string());
        assert!(
            !searcher.matches(&card, &board, &[]),
            "and not to the board's new prefix, which no card carries"
        );
    }

    #[test]
    fn test_resolve_card_prefix_uses_the_board_prefix() {
        let b = board(Some("KAN"));
        let col = column(b.id);
        let c = card(col.id, b.id, 1);

        assert_eq!(
            resolve_card_prefix(&c, &[col], &[b], &[], "task"),
            "KAN",
            "the configured casing, not a normalised form: this feeds the value \
             stamped on a card and rendered in branch names. Comparisons \
             normalise at the point of comparison instead."
        );
    }

    #[test]
    fn test_resolve_card_prefix_falls_back_to_the_default() {
        let b = board(None);
        let col = column(b.id);
        let c = card(col.id, b.id, 1);

        assert_eq!(resolve_card_prefix(&c, &[col], &[b], &[], "task"), "task");
    }

    #[test]
    fn test_resolve_card_prefix_prefers_a_sprint_override() {
        let b = board(Some("KAN"));
        let col = column(b.id);
        let mut sprint = Sprint::new(b.id, 1, None, None::<String>);
        sprint.card_prefix = Some("AUTH".to_string());
        let mut c = card(col.id, b.id, 1);
        c.sprint_id = Some(sprint.id);

        assert_eq!(
            resolve_card_prefix(&c, &[col], &[b], &[sprint], "task"),
            "AUTH",
            "a sprint override beats its board's prefix"
        );
    }

    /// The reader walks `card.column_id -> column.board_id`, NOT `card.board_id`.
    /// The backfill must reproduce that walk exactly, or a card whose two board
    /// references disagree would be frozen under a DIFFERENT prefix than the one
    /// the user sees today -- the precise outcome freezing exists to prevent.
    #[test]
    fn test_resolve_card_prefix_follows_the_column_not_the_cards_board_id() {
        let via_column = board(Some("COL"));
        let via_field = board(Some("FIELD"));
        let col = column(via_column.id);
        let c = card(col.id, via_field.id, 1);

        assert_eq!(
            resolve_card_prefix(&c, &[col], &[via_column, via_field], &[], "task"),
            "COL",
            "resolution goes through the column's board, matching the reader"
        );
    }

    #[test]
    fn test_resolve_card_prefix_defaults_when_the_column_is_missing() {
        let b = board(Some("KAN"));
        let c = card(uuid::Uuid::new_v4(), b.id, 1);

        assert_eq!(
            resolve_card_prefix(&c, &[], &[b], &[], "task"),
            "task",
            "an unresolvable board yields the default rather than panicking"
        );
    }
}
