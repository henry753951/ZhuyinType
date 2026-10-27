//! The reranker's candidates as one prefix tree (Swift's `RerankerTree`): candidates sharing a prefix share its
//! nodes, so the tree model computes each prefix once.

use std::collections::HashMap;

use crate::PackedTree;

/// The packed tensors and, per candidate, the nodes along its path.
#[derive(Clone, Debug, Default)]
pub struct RerankerTree {
    /// What the tree model reads.
    pub packed: PackedTree,
    /// Per candidate, the node index (into `packed.targets`) of each of its tokens.
    pub paths: Vec<Vec<usize>>,
}

impl RerankerTree {
    /// Node identity is (parent, token), never just the token: siblings cannot share attention.
    #[must_use]
    pub fn new(prefix: &[i32], candidates: &[Vec<i32>]) -> Self {
        let mut ids: Vec<i32> = prefix.to_vec();
        let mut positions: Vec<i32> = (0..prefix.len()).map(index).collect();
        let mut parents: Vec<usize> = (0..prefix.len()).map(|i| i.saturating_sub(1)).collect();
        let mut children: HashMap<(usize, i32), usize> = HashMap::new();
        let mut paths = Vec::with_capacity(candidates.len());
        for candidate in candidates {
            let mut parent = prefix.len() - 1;
            let mut path = Vec::with_capacity(candidate.len());
            for &token in candidate {
                let node = *children.entry((parent, token)).or_insert_with(|| {
                    let node = ids.len();
                    ids.push(token);
                    positions.push(positions[parent] + 1);
                    parents.push(parent);
                    node
                });
                path.push(node);
                parent = node;
            }
            paths.push(path);
        }
        // A terminal leaf needs its parent's logits, but never its own hidden state.
        let targets = ids;
        let mut state_nodes = parents.clone();
        state_nodes.sort_unstable();
        state_nodes.dedup();
        let mut state_index = vec![usize::MAX; targets.len()];
        for (index, &node) in state_nodes.iter().enumerate() {
            state_index[node] = index;
        }
        let count = state_nodes.len();
        let mut ancestors = vec![0; count * count];
        for (row, &node) in state_nodes.iter().enumerate() {
            let mut ancestor = node;
            loop {
                ancestors[row * count + state_index[ancestor]] = 1;
                if ancestor == 0 {
                    break;
                }
                ancestor = parents[ancestor];
            }
        }
        let packed = PackedTree {
            ids: state_nodes.iter().map(|&n| targets[n]).collect(),
            positions: state_nodes.iter().map(|&n| positions[n]).collect(),
            ancestors,
            parents: parents.iter().map(|&p| index(state_index[p])).collect(),
            targets,
        };
        RerankerTree { packed, paths }
    }

    /// Each candidate's log probability from the tree model's per-target values; `None` when they do not fit.
    #[must_use]
    pub fn scores(&self, logprob: &[f64], unknown_penalty: f64) -> Option<Vec<f64>> {
        if logprob.len() != self.packed.targets.len() || !logprob.iter().all(|v| v.is_finite()) {
            return None;
        }
        Some(
            self.paths
                .iter()
                .map(|path| {
                    path.iter().fold(0.0, |sum, &node| {
                        sum + logprob[node] - if self.packed.targets[node] == 2 { unknown_penalty } else { 0.0 }
                    })
                })
                .collect(),
        )
    }
}

/// A node index as a tensor value (trees have at most 32 × 64 nodes).
fn index(i: usize) -> i32 {
    i32::try_from(i).unwrap_or(i32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_prefixes_and_state_nodes() {
        // <bos> x | candidates "ab", "ac", "d".
        let tree = RerankerTree::new(&[1, 9], &[vec![3, 4], vec![3, 5], vec![6]]);
        // Nodes: 0 <bos>, 1 x, 2 a, 3 b, 4 c, 5 d; states: parents {0, 1, 2}.
        assert_eq!(tree.packed.targets, [1, 9, 3, 4, 5, 6]);
        assert_eq!(tree.paths, [vec![2, 3], vec![2, 4], vec![5]]);
        assert_eq!(tree.packed.ids, [1, 9, 3]);
        assert_eq!(tree.packed.positions, [0, 1, 2]);
        assert_eq!(tree.packed.parents, [0, 0, 1, 2, 2, 1]);
        assert_eq!(tree.packed.ancestors, [1, 0, 0, 1, 1, 0, 1, 1, 1]);
        let logprob = [0.0, 0.0, -1.0, -2.0, -3.0, -4.0];
        assert_eq!(tree.scores(&logprob, 10.0), Some(vec![-3.0, -4.0, -4.0]));
        assert_eq!(tree.scores(&logprob[..5], 10.0), None);
    }
}
