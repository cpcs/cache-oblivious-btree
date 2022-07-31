#![allow(dead_code)]

use std::ptr::null_mut;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum NodeType<'a, K: Clone + Ord, V: Clone> {
    Branch(BranchType<'a, K, V>),
    Leaf(LeafType<'a, K, V>),
}

pub(crate) struct Node<'a, K: Clone + Ord, V: Clone> {
    node_type: NodeType<'a, K, V>,
    parent: *mut Node<'a, K, V>,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct LeafType<'a, K: Clone + Ord, V: Clone> {
    key_value: Option<(&'a K, V)>,
    id: usize,
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct BranchType<'a, K: Clone + Ord, V: Clone> {
    key: Option<&'a K>,
    left: *mut Node<'a, K, V>,
    right: *mut Node<'a, K, V>,
}

impl<'a, K, V> Node<'a, K, V>
where
    K: Clone + Ord,
    V: Clone + 'a,
{
    #[inline]
    fn get_key(&self) -> Option<&K> {
        match &self.node_type {
            NodeType::Branch(branch) => branch.key,
            NodeType::Leaf(leaf) => match leaf.key_value {
                Some((key, _)) => Some(key),
                None => None,
            },
        }
    }

    #[inline]
    // Set the key for this node as the maximum key of the left and right children.
    // return whether the key is changed or not.
    fn set_branch_key(&mut self) -> bool {
        if let NodeType::Branch(branch) = &mut self.node_type {
            let mut input_key = unsafe { (*branch.right).get_key() };
            if input_key.is_none() {
                input_key = unsafe { (*branch.left).get_key() };
            }

            match branch.key {
                Some(k) => match input_key {
                    Some(ik) => {
                        branch.key = Some(ik);
                        !k.eq(ik)
                    }
                    None => {
                        branch.key = None;
                        true
                    }
                },

                None => match input_key {
                    Some(ik) => {
                        branch.key = Some(ik);
                        true
                    }
                    None => {
                        branch.key = None;
                        false
                    }
                },
            }
        } else {
            panic!("Should only set key for branch node.");
        }
    }
}

#[inline]
fn connect_nodes<'a, K, V>(
    parent: *mut Node<'a, K, V>,
    left: *mut Node<'a, K, V>,
    right: *mut Node<'a, K, V>,
) where
    K: Ord + Clone,
    V: Clone,
{
    unsafe {
        (*parent).node_type = NodeType::Branch(BranchType {
            key: None,
            left,
            right,
        });
        (*left).parent = parent;
        (*right).parent = parent;
    }
}

// Create a complete binary tree of the given height. Nodes are saved in the nodes.
// Return the pointer to root.
fn make_tree<'a, K, V>(
    height: usize,
    leaves: &mut Vec<*mut Node<'a, K, V>>,
    nodes: &mut Vec<Node<'a, K, V>>,
) -> *mut Node<'a, K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    if height == 1 {
        nodes.push(Node {
            parent: null_mut(),
            // Temporally set it to None which might be changed later.
            node_type: NodeType::Leaf(LeafType {
                key_value: None,
                id: 0,
            }),
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
    for i in (0..(top_num_leaves << 1)).step_by(2) {
        let left = make_tree(bottom_height, leaves, nodes);
        let right = make_tree(bottom_height, leaves, nodes);
        // Both children share the same parent with id i / 2.
        connect_nodes(top_leaves[i >> 1], left, right);
    }
    root
}

// This is to create the Van Emde Boas tree structure. The idea is in a paper.
// https://erikdemaine.org/papers/CacheObliviousBTrees_SICOMP/paper.pdf
// This is the cache oblivious version since by using this logic and if we put the tree nodes
// into an array using the specific order, we may reduce the number of memory loading.
pub(crate) struct VebTree<'a, K: Ord + Clone, V: Clone> {
    height: usize,
    nodes: Vec<Node<'a, K, V>>,
    leaves: Vec<*mut Node<'a, K, V>>,
    root: *mut Node<'a, K, V>,
}

impl<'a, K, V> VebTree<'a, K, V>
where
    K: Ord + Clone,
    V: Clone + 'a,
{
    fn new(height: usize) -> Self {
        let mut nodes = Vec::with_capacity((1 << height) - 1);
        let mut leaves = Vec::with_capacity(1 << (height - 1));
        let root = make_tree(height, &mut leaves, &mut nodes);
        // set up leaf id so that we know the index of each leaf.
        for (i, &leaf) in leaves.iter().enumerate() {
            unsafe {
                match &mut (*leaf).node_type {
                    NodeType::Branch(_) => panic!("Should never reach here"),
                    NodeType::Leaf(leaf_type) => leaf_type.id = i,
                }
            }
        }
        Self {
            height,
            nodes,
            leaves,
            root,
        }
    }

    fn update_values(&self, changed_leaves: &[*mut Node<'a, K, V>]) {
        let mut nodes = Vec::new();
        changed_leaves.iter().for_each(|&node| unsafe {
            if !(*node).parent.is_null()
                && (nodes.is_empty()
                    || *nodes.last().unwrap() as *const Node<'a, K, V> != (*node).parent)
            {
                nodes.push((*node).parent);
            }
        });

        let mut i = 0;
        while i < nodes.len() {
            unsafe {
                match (*nodes[i]).node_type {
                    NodeType::Branch(_) => {
                        if (*nodes[i]).set_branch_key()
                            && !(*nodes[i]).parent.is_null()
                            && *nodes.last().unwrap() as *const Node<K, V> != (*nodes[i]).parent
                        {
                            nodes.push((*nodes[i]).parent)
                        }
                    }
                    NodeType::Leaf(_) => panic!("Should not reach hear"),
                }
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod veb_tree {
    use super::{Node, NodeType, VebTree};
    use crate::veb_tree::{BranchType, LeafType};
    use std::ptr::null_mut;

    // Traverse the tree by layer.
    fn traverse<'a, K, V>(
        depth: usize,
        cur: *mut Node<'a, K, V>,
        parent: *mut Node<'a, K, V>,
        nodes: &Vec<Node<'a, K, V>>,
        positions: &mut Vec<Vec<usize>>,
        node_types: &mut Vec<Vec<&NodeType<'a, K, V>>>,
    ) where
        K: Ord + Clone,
        V: Clone + 'a,
    {
        if cur.is_null() {
            return;
        }

        // Save the "index" of each node.
        positions[depth].push(
            nodes
                .iter()
                .position(|node| cur as *const Node<K, V> == node as *const Node<K, V>)
                .unwrap(),
        );

        unsafe {
            node_types[depth].push(&(*cur).node_type);
            assert_eq!(parent, (*cur).parent);
            match &(*cur).node_type {
                NodeType::Branch(branch) => {
                    assert!(!branch.left.is_null());
                    assert!(!branch.right.is_null());
                    traverse(depth + 1, branch.left, cur, nodes, positions, node_types);
                    traverse(depth + 1, branch.right, cur, nodes, positions, node_types);
                }

                NodeType::Leaf(_) => {}
            };
        }
    }

    fn verify_leaf_ids<K, V>(leaves: &Vec<*mut Node<K, V>>)
    where
        K: Ord + Clone,
        V: Clone,
    {
        leaves.iter().enumerate().for_each(|(i, &leaf)| unsafe {
            match &mut (*leaf).node_type {
                NodeType::Branch(_) => panic!("Should never reach here"),
                NodeType::Leaf(leaf_type) => assert_eq!(leaf_type.id, i),
            }
        });
    }

    // The excatly tree was shown by the paper.
    // https://ibb.co/BtmrpDz
    #[test]
    fn test_create_tree_height5() {
        let tree = VebTree::<usize, usize>::new(5);
        assert_eq!(tree.height, 5);
        assert_eq!(tree.leaves.len(), 16);
        assert_eq!(tree.nodes.len(), 31);

        let mut positions = vec![Vec::new(); 5];
        let mut node_types = vec![Vec::new(); 5];
        traverse(
            0,
            tree.root,
            null_mut(),
            &tree.nodes,
            &mut positions,
            &mut node_types,
        );

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
    }

    // The tree was shown in a lecture video (https://ibb.co/wpspK71)
    #[test]
    fn test_update_height4() {
        let mut tree = VebTree::<usize, usize>::new(4);
        assert_eq!(tree.height, 4);
        assert_eq!(tree.leaves.len(), 8);
        assert_eq!(tree.nodes.len(), 15);

        let mut positions = vec![Vec::new(); 4];
        let mut node_types = vec![Vec::new(); 4];
        traverse(
            0,
            tree.root,
            null_mut(),
            &tree.nodes,
            &mut positions,
            &mut node_types,
        );

        verify_leaf_ids(&tree.leaves);

        assert_eq!(
            positions,
            [
                vec!(0),
                vec!(1, 2),
                vec!(3, 6, 9, 12),
                vec!(4, 5, 7, 8, 10, 11, 13, 14),
            ]
        );

        assert_eq!(
            node_types,
            [
                vec!(&NodeType::Branch(BranchType {
                    key: None,
                    left: &mut tree.nodes[1],
                    right: &mut tree.nodes[2],
                })),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[3],
                        right: &mut tree.nodes[6],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[9],
                        right: &mut tree.nodes[12],
                    })
                ),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[4],
                        right: &mut tree.nodes[5],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[7],
                        right: &mut tree.nodes[8],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[10],
                        right: &mut tree.nodes[11],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[13],
                        right: &mut tree.nodes[14],
                    })
                ),
                vec!(
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 0
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 1
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 2
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 3
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 4
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 5
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 6
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 7
                    }),
                )
            ]
        );

        unsafe {
            (*tree.leaves[3]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&333, 3)),
                id: 3,
            });
            (*tree.leaves[4]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&444, 4)),
                id: 4,
            });
        }

        tree.update_values(&tree.leaves[3..5]);
        assert_eq!(
            node_types,
            [
                vec!(&NodeType::Branch(BranchType {
                    key: Some(&444),
                    left: &mut tree.nodes[1],
                    right: &mut tree.nodes[2],
                })),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: Some(&333),
                        left: &mut tree.nodes[3],
                        right: &mut tree.nodes[6],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&444),
                        left: &mut tree.nodes[9],
                        right: &mut tree.nodes[12],
                    })
                ),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[4],
                        right: &mut tree.nodes[5],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&333),
                        left: &mut tree.nodes[7],
                        right: &mut tree.nodes[8],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&444),
                        left: &mut tree.nodes[10],
                        right: &mut tree.nodes[11],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[13],
                        right: &mut tree.nodes[14],
                    })
                ),
                vec!(
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 0
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 1
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 2
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&333, 3)),
                        id: 3
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&444, 4)),
                        id: 4
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 5
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 6
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 7
                    }),
                )
            ]
        );

        unsafe {
            (*tree.leaves[2]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&222, 2)),
                id: 2,
            });
            (*tree.leaves[5]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&555, 5)),
                id: 5,
            });
        }

        tree.update_values(&tree.leaves[2..6]);
        assert_eq!(
            node_types,
            [
                vec!(&NodeType::Branch(BranchType {
                    key: Some(&555),
                    left: &mut tree.nodes[1],
                    right: &mut tree.nodes[2],
                })),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: Some(&333),
                        left: &mut tree.nodes[3],
                        right: &mut tree.nodes[6],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&555),
                        left: &mut tree.nodes[9],
                        right: &mut tree.nodes[12],
                    })
                ),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[4],
                        right: &mut tree.nodes[5],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&333),
                        left: &mut tree.nodes[7],
                        right: &mut tree.nodes[8],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&555),
                        left: &mut tree.nodes[10],
                        right: &mut tree.nodes[11],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[13],
                        right: &mut tree.nodes[14],
                    })
                ),
                vec!(
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 0
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 1
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&222, 2)),
                        id: 2
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&333, 3)),
                        id: 3
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&444, 4)),
                        id: 4
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&555, 5)),
                        id: 5
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 6
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 7
                    }),
                )
            ]
        );

        unsafe {
            (*tree.leaves[0]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&0, 0)),
                id: 0,
            });
            (*tree.leaves[1]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&1, 1)),
                id: 1,
            });
            (*tree.leaves[2]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&2, 2)),
                id: 2,
            });
            (*tree.leaves[3]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&3, 3)),
                id: 3,
            });
            (*tree.leaves[4]).node_type = NodeType::Leaf(LeafType {
                key_value: None,
                id: 4,
            });
            (*tree.leaves[5]).node_type = NodeType::Leaf(LeafType {
                key_value: None,
                id: 5,
            });
            (*tree.leaves[6]).node_type = NodeType::Leaf(LeafType {
                key_value: Some((&666, 6)),
                id: 6,
            });
        }

        tree.update_values(&tree.leaves);
        assert_eq!(
            node_types,
            [
                vec!(&NodeType::Branch(BranchType {
                    key: Some(&666),
                    left: &mut tree.nodes[1],
                    right: &mut tree.nodes[2],
                })),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: Some(&3),
                        left: &mut tree.nodes[3],
                        right: &mut tree.nodes[6],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&666),
                        left: &mut tree.nodes[9],
                        right: &mut tree.nodes[12],
                    })
                ),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: Some(&1),
                        left: &mut tree.nodes[4],
                        right: &mut tree.nodes[5],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&3),
                        left: &mut tree.nodes[7],
                        right: &mut tree.nodes[8],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut tree.nodes[10],
                        right: &mut tree.nodes[11],
                    }),
                    &NodeType::Branch(BranchType {
                        key: Some(&666),
                        left: &mut tree.nodes[13],
                        right: &mut tree.nodes[14],
                    })
                ),
                vec!(
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&0, 0)),
                        id: 0
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&1, 1)),
                        id: 1
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&2, 2)),
                        id: 2
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&3, 3)),
                        id: 3
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 4
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 5
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: Some((&666, 6)),
                        id: 6
                    }),
                    &NodeType::Leaf(LeafType {
                        key_value: None,
                        id: 7
                    }),
                )
            ]
        );
    }
}
