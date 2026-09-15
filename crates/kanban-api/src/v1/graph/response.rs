use crate::v1::enums::{RelatesKindDto, SeverityDto};
use kanban_domain::DependencyGraph;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A blocking edge incident to the requested card, carrying its severity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BlockEdgeDto {
    pub blocker: Uuid,
    pub blocked: Uuid,
    pub severity: SeverityDto,
}

/// An undirected relates edge incident to the requested card, carrying its
/// kind. `source`/`target` preserve whichever orientation created the edge;
/// callers that only care about the other endpoint should compare against
/// both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RelatedEdgeDto {
    pub source: Uuid,
    pub target: Uuid,
    pub kind: RelatesKindDto,
}

/// A card's dependency edges, scoped to that card and split by direction.
/// `parents`/`children` are the Spawns edges that spawned it / that it
/// spawned; `blocked_by`/`blocks` are the Blocks edges pointing into it /
/// out of it; `related` is its undirected Relates neighbors. Only active
/// edges are reported; archived edges are excluded. `block_edges` and
/// `related_edges` carry the same active-only, incident-only scoping as the
/// id arrays, plus each edge's metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CardGraphResponse {
    pub card_id: Uuid,
    pub parents: Vec<Uuid>,
    pub children: Vec<Uuid>,
    pub blocked_by: Vec<Uuid>,
    pub blocks: Vec<Uuid>,
    pub related: Vec<Uuid>,
    #[serde(default)]
    pub block_edges: Vec<BlockEdgeDto>,
    #[serde(default)]
    pub related_edges: Vec<RelatedEdgeDto>,
}

impl CardGraphResponse {
    pub fn from_graph(card_id: Uuid, graph: &DependencyGraph) -> Self {
        let block_edges = graph
            .blocks_edges()
            .iter()
            .filter(|e| {
                e.base.archived_at.is_none()
                    && (e.base.source == card_id || e.base.target == card_id)
            })
            .map(|e| BlockEdgeDto {
                blocker: e.base.source,
                blocked: e.base.target,
                severity: e.severity.into(),
            })
            .collect();
        let related_edges = graph
            .relates_edges()
            .iter()
            .filter(|e| {
                e.base.archived_at.is_none()
                    && (e.base.source == card_id || e.base.target == card_id)
            })
            .map(|e| RelatedEdgeDto {
                source: e.base.source,
                target: e.base.target,
                kind: e.kind.into(),
            })
            .collect();
        Self {
            card_id,
            parents: graph.parents(card_id),
            children: graph.children(card_id),
            blocked_by: graph.blockers(card_id),
            blocks: graph.blocked(card_id),
            related: graph.related(card_id),
            block_edges,
            related_edges,
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
        let blocker = Uuid::new_v4();
        let blocked = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let response = CardGraphResponse {
            card_id: Uuid::new_v4(),
            parents: vec![Uuid::new_v4()],
            children: vec![Uuid::new_v4()],
            blocked_by: vec![Uuid::new_v4()],
            blocks: vec![Uuid::new_v4()],
            related: vec![Uuid::new_v4()],
            block_edges: vec![BlockEdgeDto {
                blocker,
                blocked,
                severity: crate::v1::enums::SeverityDto::Critical,
            }],
            related_edges: vec![RelatedEdgeDto {
                source,
                target,
                kind: crate::v1::enums::RelatesKindDto::Duplicates,
            }],
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
            "block_edges",
            "related_edges",
        ] {
            assert!(obj.get(key).is_some(), "missing key {key}");
        }
        assert_eq!(
            obj.len(),
            8,
            "unexpected keys: {:?}",
            obj.keys().collect::<Vec<_>>()
        );

        let round_tripped: CardGraphResponse = serde_json::from_value(value).unwrap();
        assert_eq!(round_tripped, response);
    }

    #[test]
    fn test_card_graph_response_block_edges_carry_severity() {
        let blocker = Uuid::new_v4();
        let subject = Uuid::new_v4();

        let mut g = DependencyGraph::new();
        g.set_block_with_severity(blocker, subject, Severity::Critical)
            .unwrap();

        let r = CardGraphResponse::from_graph(subject, &g);
        assert_eq!(
            r.block_edges,
            vec![BlockEdgeDto {
                blocker,
                blocked: subject,
                severity: crate::v1::enums::SeverityDto::Critical,
            }]
        );
    }

    #[test]
    fn test_card_graph_response_related_edges_carry_kind() {
        let subject = Uuid::new_v4();
        let other = Uuid::new_v4();

        let mut g = DependencyGraph::new();
        g.relate_with_kind(subject, other, RelatesKind::Duplicates)
            .unwrap();

        let r = CardGraphResponse::from_graph(subject, &g);
        assert_eq!(r.related_edges.len(), 1);
        let edge = &r.related_edges[0];
        let endpoints: std::collections::HashSet<Uuid> =
            [edge.source, edge.target].into_iter().collect();
        assert_eq!(
            endpoints,
            [subject, other]
                .into_iter()
                .collect::<std::collections::HashSet<Uuid>>()
        );
        assert_eq!(edge.kind, crate::v1::enums::RelatesKindDto::Duplicates);
    }

    #[test]
    fn test_card_graph_response_new_edge_fields_exclude_archived_and_foreign_edges() {
        let subject = Uuid::new_v4();
        let archived_blocker = Uuid::new_v4();
        let archived_rel = Uuid::new_v4();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let incident_blocker = Uuid::new_v4();

        let mut g = DependencyGraph::new();
        g.add_archived_blocks(archived_blocker, subject, Severity::High)
            .unwrap();
        g.add_archived_relates(subject, archived_rel, RelatesKind::Duplicates)
            .unwrap();
        g.set_block_with_severity(a, b, Severity::Low).unwrap();
        g.relate_with_kind(a, b, RelatesKind::General).unwrap();

        let r = CardGraphResponse::from_graph(subject, &g);
        assert!(r.block_edges.is_empty());
        assert!(r.related_edges.is_empty());

        g.set_block_with_severity(incident_blocker, subject, Severity::Medium)
            .unwrap();
        let r = CardGraphResponse::from_graph(subject, &g);
        assert_eq!(r.block_edges.len(), 1);
        assert_eq!(r.block_edges[0].blocker, incident_blocker);
    }

    #[test]
    fn test_card_graph_response_deserializes_payload_without_edge_fields() {
        let card_id = Uuid::new_v4();
        let value = serde_json::json!({
            "card_id": card_id,
            "parents": [],
            "children": [],
            "blocked_by": [],
            "blocks": [],
            "related": [],
        });

        let response: CardGraphResponse = serde_json::from_value(value).unwrap();
        assert!(response.block_edges.is_empty());
        assert!(response.related_edges.is_empty());
    }
}
