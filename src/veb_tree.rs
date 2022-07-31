#![allow(dead_code)]

use std::ptr::null_mut;

pub(crate) struct Node {
    parent: *mut Node,
    left: *mut Node,
    right: *mut Node,
}

#[inline]
fn connect_nodes(parent: *mut Node, child: *mut Node, is_left: bool) {
    unsafe {
        if is_left {
            (*parent).left = child;
        } else {
            (*parent).right = child;
        }
        (*child).parent = parent;
    }
}

// Create a complete binary tree of the given height. Nodes are saved in the nodes.
// Return the pointer to root.
fn make_tree(height: usize, leaves: &mut Vec<*mut Node>, nodes: &mut Vec<Node>) -> *mut Node {
    if height == 1 {
        nodes.push(Node {
            parent: null_mut(),
            left: null_mut(),
            right: null_mut(),
        });
        leaves.push(nodes.last_mut().unwrap());
        return *leaves.last().unwrap();
    }

    let bottom_height = ((height + 1) >> 1).next_power_of_two();
    let top_height = height - bottom_height;
    let top_num_leaves = 1usize << (top_height - 1);

    // Create the top tree first.
    let mut top_leaves = Vec::with_capacity(top_num_leaves);
    let root = make_tree(top_height, &mut top_leaves, nodes);

    // Create the bottom trees and connect each one to the top tree.
    for i in 0..(top_num_leaves << 1) {
        let sub_root = make_tree(bottom_height, leaves, nodes);
        // The sub_root id is i, the parent is i / 2 and if i is odd, it's parent's left child.
        connect_nodes(top_leaves[i >> 1], sub_root, (i & 1) == 0);
    }
    root
}

// This is to create the Van Emde Boas tree structure. The idea is in a paper.
// https://erikdemaine.org/papers/CacheObliviousBTrees_SICOMP/paper.pdf
// This is the cache oblivious version since by using this logic and if we put the tree nodes
// into an array using the specific order, we may reduce the number of memory loading.
pub(crate) struct VebTree {
    height: usize,
    nodes: Vec<Node>,
    leaves: Vec<*mut Node>,
    root: *mut Node,
}

impl VebTree {
    fn new(height: usize) -> Self {
        let mut nodes = Vec::with_capacity((1 << height) - 1);
        let mut leaves = Vec::with_capacity(1 << (height - 1));
        let root = make_tree(height, &mut leaves, &mut nodes);
        Self {
            height,
            nodes,
            leaves,
            root,
        }
    }
}

#[cfg(test)]
mod veb_tree {
    use super::{Node, VebTree};
    use std::ptr::null_mut;

    // Traverse the tree by layer.
    fn traverse(
        depth: usize,
        cur: *mut Node,
        parent: *mut Node,
        nodes: &Vec<Node>,
        positions: &mut Vec<Vec<usize>>,
    ) {
        if cur.is_null() {
            return;
        }

        // Save the "index" of each node.
        positions[depth].push(
            nodes
                .iter()
                .position(|node| cur as *const Node == node as *const Node)
                .unwrap(),
        );

        unsafe {
            assert_eq!(parent, (*cur).parent);
            traverse(depth + 1, (*cur).left, cur, nodes, positions);
            traverse(depth + 1, (*cur).right, cur, nodes, positions);
        }
    }

    fn get_leaf_positions(tree: &VebTree) -> Vec<usize> {
        tree.leaves
            .iter()
            .map(|&leaf| {
                tree.nodes
                    .iter()
                    .position(|node| leaf as *const Node == node as *const Node)
                    .unwrap()
            })
            .collect::<Vec<usize>>()
    }

    // The excatly tree was shown by the paper.
    // https://ibb.co/BtmrpDz
    #[test]
    fn test_create_tree_height5() {
        let tree = VebTree::new(5);
        assert_eq!(tree.height, 5);
        assert_eq!(tree.leaves.len(), 16);
        assert_eq!(tree.nodes.len(), 31);

        let mut positions = vec![Vec::new(); 5];
        traverse(0, tree.root, null_mut(), &tree.nodes, &mut positions);

        assert_eq!(
            positions,
            [
                vec!(0),
                vec!(1, 16),
                vec!(2, 3, 17, 18),
                vec!(4, 7, 10, 13, 19, 22, 25, 28),
                vec!(5, 6, 8, 9, 11, 12, 14, 15, 20, 21, 23, 24, 26, 27, 29, 30),
            ]
        );

        assert_eq!(
            get_leaf_positions(&tree),
            [5, 6, 8, 9, 11, 12, 14, 15, 20, 21, 23, 24, 26, 27, 29, 30]
        );
    }

    // The tree was shown in a lecture video (https://ibb.co/wpspK71)
    #[test]
    fn test_create_tree_height4() {
        let tree = VebTree::new(4);
        assert_eq!(tree.height, 4);
        assert_eq!(tree.leaves.len(), 8);
        assert_eq!(tree.nodes.len(), 15);

        let mut positions = vec![Vec::new(); 4];
        traverse(0, tree.root, null_mut(), &tree.nodes, &mut positions);

        assert_eq!(
            positions,
            [
                vec!(0),
                vec!(1, 2),
                vec!(3, 6, 9, 12),
                vec!(4, 5, 7, 8, 10, 11, 13, 14),
            ]
        );
        assert_eq!(get_leaf_positions(&tree), [4, 5, 7, 8, 10, 11, 13, 14]);
    }
}
