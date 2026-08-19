use kanban_domain::DependencyGraph;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A card's dependency edges, scoped to that card and split by direction.
/// `parents`/`children` are the Spawns edges that spawned it / that it
/// spawned; `blocked_by`/`blocks` are the Blocks edges pointing into it /
/// out of it; `related` is its undirected Relates neighbors. Only active
/// edges are reported; archived edges are excluded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardGraphResponse {
    pub card_id: Uuid,
    pub parents: Vec<Uuid>,
    pub children: Vec<Uuid>,
    pub blocked_by: Vec<Uuid>,
    pub blocks: Vec<Uuid>,
    pub related: Vec<Uuid>,
}

impl CardGraphResponse {
    pub fn from_graph(card_id: Uuid, graph: &DependencyGraph) -> Self {
        Self {
            card_id,
            parents: graph.parents(card_id),
            children: graph.children(card_id),
            blocked_by: graph.blockers(card_id),
            blocks: graph.blocked(card_id),
            related: graph.related(card_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kanban_domain::{RelatesKind, Severity};

    #[test]
    fn test_card_graph_response_from_graph_maps_all_five_edge_kinds() {
        let subject = Uuid::new_v4();
        let parent = Uuid::new_v4();
        let child = Uuid::new_v4();
        let blocker = Uuid::new_v4();
        let blocked = Uuid::new_v4();
        let rel = Uuid::new_v4();

        let mut g = DependencyGraph::new();
        g.set_parent(subject, parent).unwrap();
        g.set_parent(child, subject).unwrap();
        g.set_block(blocker, subject).unwrap();
        g.set_block(subject, blocked).unwrap();
        g.relate(subject, rel).unwrap();

        let r = CardGraphResponse::from_graph(subject, &g);

        assert_eq!(r.card_id, subject);
        assert_eq!(r.parents, vec![parent]);
        assert_eq!(r.children, vec![child]);
        assert_eq!(r.blocked_by, vec![blocker]);
        assert_eq!(r.blocks, vec![blocked]);
        assert_eq!(r.related, vec![rel]);
    }

    #[test]
    fn test_card_graph_response_scoped_to_the_requested_card_excludes_foreign_edges() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let subject = Uuid::new_v4();

        let mut g = DependencyGraph::new();
        g.set_parent(b, a).unwrap();
        g.set_block(a, b).unwrap();
        g.relate(a, b).unwrap();

        let r = CardGraphResponse::from_graph(subject, &g);

        assert_eq!(r.card_id, subject);
        assert!(r.parents.is_empty());
        assert!(r.children.is_empty());
        assert!(r.blocked_by.is_empty());
        assert!(r.blocks.is_empty());
        assert!(r.related.is_empty());
        assert!(!r.parents.contains(&a) && !r.children.contains(&b) && !r.related.contains(&a));
    }

    #[test]
    fn test_card_graph_response_excludes_archived_edges() {
        let subject = Uuid::new_v4();
        let archived_parent = Uuid::new_v4();
        let archived_child = Uuid::new_v4();
        let archived_blocker = Uuid::new_v4();
        let archived_rel = Uuid::new_v4();
        let live_parent = Uuid::new_v4();

        let mut g = DependencyGraph::new();
        g.add_archived_spawns(archived_parent, subject).unwrap();
        g.add_archived_spawns(subject, archived_child).unwrap();
        g.add_archived_blocks(archived_blocker, subject, Severity::Medium)
            .unwrap();
        g.add_archived_relates(subject, archived_rel, RelatesKind::General)
            .unwrap();

        let r = CardGraphResponse::from_graph(subject, &g);
        assert!(r.parents.is_empty());
        assert!(r.children.is_empty());
        assert!(r.blocked_by.is_empty());
        assert!(r.blocks.is_empty());
        assert!(r.related.is_empty());

        g.set_parent(subject, live_parent).unwrap();
        let r = CardGraphResponse::from_graph(subject, &g);
        assert_eq!(r.parents, vec![live_parent]);
    }

    #[test]
    fn test_card_graph_response_serde_round_trip() {
        let response = CardGraphResponse {
            card_id: Uuid::new_v4(),
            parents: vec![Uuid::new_v4()],
            children: vec![Uuid::new_v4()],
            blocked_by: vec![Uuid::new_v4()],
            blocks: vec![Uuid::new_v4()],
            related: vec![Uuid::new_v4()],
        };

        let value = serde_json::to_value(&response).unwrap();
        let obj = value.as_object().expect("serializes to a JSON object");
        for key in [
            "card_id",
            "parents",
            "children",
            "blocked_by",
            "blocks",
            "related",
        ] {
            assert!(obj.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(
            obj.len(),
            6,
            "unexpected keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        let round_tripped: CardGraphResponse = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, response);
    }
}
