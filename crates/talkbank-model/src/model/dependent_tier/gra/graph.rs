//! Cycle evidence over every authored dependency edge, without choosing a parent.

use std::collections::HashMap;

use super::GrammaticalRelation;
use crate::alignment::indices::{GraHeadRef, SemanticWordIndex1};

/// A dense node identity minted only when its adjacency slot is allocated.
#[derive(Clone, Copy)]
struct NodeId(usize);

/// Construction owns both index admission and allocation of matching slots.
#[derive(Default)]
struct GraphBuilder {
    words: HashMap<SemanticWordIndex1, NodeId>,
    parents: Vec<Vec<NodeId>>,
}

impl GraphBuilder {
    fn node(&mut self, word: SemanticWordIndex1) -> NodeId {
        *self.words.entry(word).or_insert_with(|| {
            let node = NodeId(self.parents.len());
            self.parents.push(Vec::new());
            node
        })
    }

    fn connect(&mut self, word: SemanticWordIndex1, head: SemanticWordIndex1) {
        let from = self.node(word);
        let to = self.node(head);
        self.parents[from.0].push(to);
    }

    fn finish(self) -> ObservedGraGraph {
        ObservedGraGraph {
            parents: self.parents,
        }
    }
}

/// Resolved observed edges, not proof of a valid single-parent dependency tree.
/// Repeated indices retain all their edges. Missing heads have an allocated
/// node with no outgoing edges; index/alignment validation remains separate.
pub(super) struct ObservedGraGraph {
    parents: Vec<Vec<NodeId>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CycleEvidence {
    Absent,
    Present,
}

impl ObservedGraGraph {
    pub(super) fn from_relations(relations: &[GrammaticalRelation]) -> Self {
        let mut builder = GraphBuilder::default();
        for relation in relations {
            let Ok(word) = relation.index_as_semantic() else {
                // Head zero denotes the virtual root, never dependent zero.
                // No edge can enter this invalid dependent, so it cannot be
                // part of a cycle. The original relation remains in the model.
                continue;
            };
            match relation.head_ref() {
                GraHeadRef::Root => {}
                GraHeadRef::Word(head)
                    if head == word && relation.relation.as_str().eq_ignore_ascii_case("ROOT") => {}
                GraHeadRef::Word(head) => builder.connect(word, head),
            }
        }
        builder.finish()
    }

    /// Iterative directed-graph DFS. Each node is entered once and each edge
    /// scheduled once: O(V + E) work after expected-linear hash-map admission.
    /// The private node identities already prove every lookup is in bounds.
    pub(super) fn classify(self) -> CycleEvidence {
        #[derive(Clone, Copy)]
        enum Visit {
            Unseen,
            Visiting,
            Finished,
        }
        enum Frame {
            Enter(NodeId),
            Leave(NodeId),
        }

        let mut visits = vec![Visit::Unseen; self.parents.len()];
        let mut stack = Vec::new();
        for index in 0..self.parents.len() {
            if matches!(visits[index], Visit::Finished) {
                continue;
            }
            stack.push(Frame::Enter(NodeId(index)));
            while let Some(frame) = stack.pop() {
                match frame {
                    Frame::Leave(node) => visits[node.0] = Visit::Finished,
                    Frame::Enter(node) => match visits[node.0] {
                        Visit::Finished => {}
                        Visit::Visiting => return CycleEvidence::Present,
                        Visit::Unseen => {
                            visits[node.0] = Visit::Visiting;
                            stack.push(Frame::Leave(node));
                            stack.extend(
                                self.parents[node.0].iter().rev().copied().map(Frame::Enter),
                            );
                        }
                    },
                }
            }
        }
        CycleEvidence::Absent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructor-level scale control: no recursive call stack and no vector
    /// sized by an authored index. CHAT parsing is exercised by the spec suite.
    #[test]
    fn long_chains_and_sparse_indices_preserve_cycle_evidence() {
        let mut chain = (1..=25_000)
            .map(|index| GrammaticalRelation::new(index, index + 1, "DEP".try_into().unwrap()))
            .collect::<Vec<_>>();
        chain.push(GrammaticalRelation::new(
            25_001,
            usize::MAX,
            "DEP".try_into().unwrap(),
        ));
        chain.push(GrammaticalRelation::new(
            usize::MAX,
            0,
            "ROOT".try_into().unwrap(),
        ));
        assert_eq!(
            ObservedGraGraph::from_relations(&chain).classify(),
            CycleEvidence::Absent
        );
        chain.push(GrammaticalRelation::new(
            usize::MAX,
            1,
            "DEP".try_into().unwrap(),
        ));
        assert_eq!(
            ObservedGraGraph::from_relations(&chain).classify(),
            CycleEvidence::Present
        );
    }

    #[test]
    fn repeated_acyclic_edges_do_not_become_back_edges() {
        let relations = [
            GrammaticalRelation::new(1, 2, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(1, 3, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(1, 2, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(2, 3, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(3, 0, "ROOT".try_into().unwrap()),
        ];
        assert_eq!(
            ObservedGraGraph::from_relations(&relations).classify(),
            CycleEvidence::Absent
        );
    }

    /// Raw construction/import may contain dependent zero, but head zero is
    /// always a virtual root, never an incoming edge to that invalid dependent.
    #[test]
    fn dependent_zero_does_not_conflate_the_virtual_root_with_a_word() {
        let acyclic = [
            GrammaticalRelation::new(0, 1, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(1, 0, "DEP".try_into().unwrap()),
        ];
        assert_eq!(
            ObservedGraGraph::from_relations(&acyclic).classify(),
            CycleEvidence::Absent
        );
        let cyclic = [
            GrammaticalRelation::new(0, 1, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(1, 2, "DEP".try_into().unwrap()),
            GrammaticalRelation::new(2, 1, "DEP".try_into().unwrap()),
        ];
        assert_eq!(
            ObservedGraGraph::from_relations(&cyclic).classify(),
            CycleEvidence::Present
        );
    }
}
