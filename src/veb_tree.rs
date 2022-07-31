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
// Return a root pointer.
fn make_tree(height: usize, leafs: &mut Vec<*mut Node>, nodes: &mut Vec<Node>) -> *mut Node {
    if height == 1 {
        nodes.push(Node {
            parent: null_mut(),
            left: null_mut(),
            right: null_mut(),
        });
        leafs.push(nodes.last_mut().unwrap());
        return *leafs.last().unwrap();
    }
    let bottom_height = (height >> 1).next_power_of_two();
    let top_height = height - bottom_height;
    let top_num_leafs = 1usize << (top_height - 1);

    // Create the bottom trees first.
    let mut sub_roots = Vec::new();
    for i in 0..(top_num_leafs << 1) {
        sub_roots.push(make_tree(bottom_height, leafs, nodes));
    }

    // Create the top tree.
    let mut top_leafs = Vec::with_capacity(top_num_leafs);
    let root = make_tree(top_height, &mut top_leafs, nodes);

    // Connect the top tree leafs with the bottom trees.
    for (i, &node) in top_leafs.iter().enumerate() {
        let index = i << 1;
        connect_nodes(node, sub_roots[index], true);
        connect_nodes(node, top_leafs[index | 1], false);
    }
    root
}
