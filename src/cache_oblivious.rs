#![allow(dead_code)]

use std::ptr::null_mut;

use crate::packed_memory_array::PackedMemoryArray;

#[derive(Debug, Eq, PartialEq)]
enum NodeType<'a, K: Clone + Ord, V: Clone> {
    Branch(BranchType<'a, K, V>),
    Leaf(LeafType<K>),
}

#[derive(Debug, Eq, PartialEq)]
struct Node<'a, K: Clone + Ord, V: Clone> {
    node_type: NodeType<'a, K, V>,
    parent: *mut Node<'a, K, V>,
}

#[derive(Debug, Eq, PartialEq)]
struct LeafType<K: Clone + Ord> {
    key: Option<K>,
}

#[derive(Debug, Eq, PartialEq)]
struct BranchType<'a, K: Clone + Ord, V: Clone> {
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
            NodeType::Leaf(leaf) => leaf.key.as_ref(),
        }
    }

    #[inline]
    // Set the key value for the leaf node. Note: The key should not be changed unless
    // Returns whether the key changed.
    fn set_leave_key(&mut self, key: Option<K>) -> bool {
        let mut key_changed = true;
        match &mut self.node_type {
            NodeType::Branch(_) => panic!("Should only call this for leaf nodes."),
            NodeType::Leaf(leaf) => {
                if let Some(ref leaf_key) = leaf.key {
                    if let Some(ref key) = key {
                        key_changed = !key.eq(leaf_key);
                    }
                } else if key.is_none() {
                    // Both are None.
                    return false;
                }
                leaf.key = key;
            }
        }
        key_changed
    }

    #[inline]
    // Set the key for this node as the maximum key of the left and right children.
    // Return whether the key is changed or not.
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
            node_type: NodeType::Leaf(LeafType { key: None }),
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
pub struct BTreeMap<'a, K: Ord + Clone, V: Clone> {
    height: usize,
    nodes: Vec<Node<'a, K, V>>,
    leaves: Vec<*mut Node<'a, K, V>>,
    root: *mut Node<'a, K, V>,
    pma: PackedMemoryArray<K, V>,
    size: usize,
}

impl<'a, K, V> Default for BTreeMap<'a, K, V>
where
    K: Ord + Clone,
    V: Clone + 'a,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, K, V> BTreeMap<'a, K, V>
where
    K: Ord + Clone,
    V: Clone + 'a,
{
    pub fn new() -> Self {
        let mut nodes = Vec::with_capacity(1);
        let mut leaves = Vec::with_capacity(1);
        let root = make_tree(1, &mut leaves, &mut nodes);
        Self {
            height: 1,
            nodes,
            leaves,
            root,
            pma: PackedMemoryArray::new(),
            size: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.size == 0
    }

    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        let (old_value, changed_range) = self.pma.insert(self.find_index(&key), (key, value));
        if old_value.is_none() {
            self.size += 1;
        }
        match changed_range {
            Some((from, to)) => self.populate_changes(from, to),
            None => self.rebuild(),
        }
        old_value
    }

    pub fn remove(&mut self, key: &K) -> Option<V> {
        let index = self.find_index(key);
        if index >= self.leaves.len() {
            None
        } else {
            unsafe {
                match (*self.leaves[index]).get_key() {
                    Some(k) => {
                        if !k.eq(key) {
                            return None;
                        }
                    }
                    None => return None,
                }
            }
            let (old_value, changed_range) = self.pma.remove(index);
            if old_value.is_some() {
                self.size -= 1;
                match changed_range {
                    Some((from, to)) => self.populate_changes(from, to),
                    None => self.rebuild(),
                }
            }
            old_value
        }
    }

    pub fn get_top_k_key_values(&self, k: usize) -> Vec<(&K, &V)> {
        self.pma
            .get_key_values()
            .iter()
            .filter_map(|kv| kv.as_ref())
            .map(|kv| (&kv.0, &kv.1))
            .take(k)
            .collect()
    }

    pub fn len(&self) -> usize {
        self.size
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn keys(&self) -> Vec<&K> {
        self.pma
            .get_key_values()
            .iter()
            .filter_map(|kv| kv.as_ref())
            .map(|kv| &kv.0)
            .collect::<Vec<&K>>()
    }

    pub fn values(&self) -> Vec<&V> {
        self.pma
            .get_key_values()
            .iter()
            .filter_map(|kv| kv.as_ref())
            .map(|kv| &kv.1)
            .collect::<Vec<&V>>()
    }

    pub fn get_first_key(&self) -> Option<&K> {
        self.pma
            .get_key_values()
            .iter()
            .filter_map(|kv| kv.as_ref())
            .map(|kv| &kv.0)
            .next()
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let index = self.find_index(key);
        if index >= self.leaves.len() {
            return None;
        }
        match &self.pma.get_key_values()[index] {
            None => None,
            Some((k, v)) => {
                if key.eq(k) {
                    Some(v)
                } else {
                    None
                }
            }
        }
    }

    pub fn get_all_key_values(&self) -> Vec<(&K, &V)> {
        self.pma
            .get_key_values()
            .iter()
            .filter_map(|kv| kv.as_ref())
            .map(|kv| (&kv.0, &kv.1))
            .collect()
    }

    fn rebuild(&mut self) {
        self.nodes = Vec::with_capacity(self.pma.data_len() << 1);
        self.leaves = Vec::with_capacity(self.pma.data_len());
        self.height = (self.pma.data_len().trailing_zeros() + 1) as usize;
        self.root = make_tree(self.height, &mut self.leaves, &mut self.nodes);
        self.populate_changes(0, self.pma.data_len());
    }

    fn find_index(&self, key: &K) -> usize {
        let mut cur = self.root as *const Node<K, V>;
        let mut index = 0usize;
        unsafe {
            while let NodeType::Branch(branch) = &(*cur).node_type {
                index <<= 1;
                cur = match (*branch.left).get_key() {
                    Some(k) => {
                        if k.ge(key) {
                            branch.left
                        } else {
                            index |= 1;
                            branch.right
                        }
                    }
                    None => {
                        index |= 1;
                        branch.right
                    }
                }
            }
        }
        assert!(cur == self.leaves[index]);
        unsafe {
            if let Some(k) = (*cur).get_key() {
                if k.lt(key) {
                    // This Key is the largest.
                    index += 1;
                }
            }
        }
        index
    }

    // Populated the changed leaves to root.
    fn populate_changes(&self, from: usize, to: usize) {
        let key_values = self.pma.get_key_values();
        let mut changed_nodes = Vec::with_capacity(self.nodes.len());
        for (i, key_value) in key_values.iter().enumerate().take(to).skip(from) {
            let node = self.leaves[i];
            unsafe {
                (*node).set_leave_key(key_value.as_ref().map(|kv| kv.0.clone()));
                if !(*node).parent.is_null()
                    && (changed_nodes.is_empty()
                        || *changed_nodes.last().unwrap() as *const Node<K, V> != (*node).parent)
                {
                    changed_nodes.push((*node).parent);
                }
            }
        }

        let mut i = 0;
        while i < changed_nodes.len() {
            unsafe {
                match (*changed_nodes[i]).node_type {
                    NodeType::Branch(_) => {
                        if (*changed_nodes[i]).set_branch_key()
                            && !(*changed_nodes[i]).parent.is_null()
                            && *changed_nodes.last().unwrap() as *const Node<K, V>
                                != (*changed_nodes[i]).parent
                        {
                            changed_nodes.push((*changed_nodes[i]).parent)
                        }
                    }
                    NodeType::Leaf(_) => panic!("Should not reach here"),
                }
            }
            i += 1;
        }
    }
}

#[cfg(test)]
mod veb_tree {
    use super::{make_tree, BTreeMap, BranchType, LeafType, Node, NodeType};
    use rand::{seq::SliceRandom, thread_rng};
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
        V: Clone,
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

    // The excatly tree was shown by the paper.
    // https://ibb.co/BtmrpDz
    #[test]
    fn test_create_tree() {
        let mut nodes = Vec::with_capacity(31);
        let mut leaves = Vec::with_capacity(16);
        let root = make_tree::<usize, usize>(5, &mut leaves, &mut nodes);

        assert_eq!(leaves.len(), 16);
        assert_eq!(nodes.len(), 31);

        let mut positions = vec![Vec::new(); 5];
        let mut node_types = vec![Vec::new(); 5];
        traverse(0, root, null_mut(), &nodes, &mut positions, &mut node_types);

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
            node_types,
            [
                vec!(&NodeType::Branch(BranchType {
                    key: None,
                    left: &mut nodes[1],
                    right: &mut nodes[16],
                })),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[2],
                        right: &mut nodes[3],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[17],
                        right: &mut nodes[18],
                    })
                ),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[4],
                        right: &mut nodes[7],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[10],
                        right: &mut nodes[13],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[19],
                        right: &mut nodes[22],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[25],
                        right: &mut nodes[28],
                    })
                ),
                vec!(
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[5],
                        right: &mut nodes[6],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[8],
                        right: &mut nodes[9],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[11],
                        right: &mut nodes[12],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[14],
                        right: &mut nodes[15],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[20],
                        right: &mut nodes[21],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[23],
                        right: &mut nodes[24],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[26],
                        right: &mut nodes[27],
                    }),
                    &NodeType::Branch(BranchType {
                        key: None,
                        left: &mut nodes[29],
                        right: &mut nodes[30],
                    })
                ),
                vec!(
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                    &NodeType::Leaf(LeafType { key: None }),
                )
            ]
        );
    }

    #[test]
    fn test_opertions() {
        let mut tree = BTreeMap::<usize, usize>::new();
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.get_all_key_values(), []);

        assert_eq!(tree.insert(1, 11), None);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.get_all_key_values(), [(&1, &11)]);

        assert_eq!(tree.insert(3, 33), None);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get_all_key_values(), [(&1, &11), (&3, &33)]);

        assert_eq!(tree.insert(0, 0), None);
        assert_eq!(tree.len(), 3);
        assert_eq!(tree.get_all_key_values(), [(&0, &0), (&1, &11), (&3, &33)]);

        assert_eq!(tree.insert(5, 555), None);
        assert_eq!(tree.len(), 4);
        assert_eq!(
            tree.get_all_key_values(),
            [(&0, &0), (&1, &11), (&3, &33), (&5, &555)]
        );

        assert_eq!(tree.insert(2, 2222), None);
        assert_eq!(tree.len(), 5);
        assert_eq!(
            tree.get_all_key_values(),
            [(&0, &0), (&1, &11), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(tree.insert(1, 1000), Some(11));
        assert_eq!(tree.len(), 5);
        assert_eq!(
            tree.get_all_key_values(),
            [(&0, &0), (&1, &1000), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(tree.insert(4, 44444), None);
        assert_eq!(tree.len(), 6);
        assert_eq!(
            tree.get_all_key_values(),
            [
                (&0, &0),
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(tree.insert(0, 44444), Some(0));
        assert_eq!(tree.len(), 6);
        assert_eq!(
            tree.get_all_key_values(),
            [
                (&0, &44444),
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(tree.remove(&0), Some(44444));
        assert_eq!(tree.len(), 5);
        assert_eq!(
            tree.get_all_key_values(),
            [
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(tree.remove(&0), None);
        assert_eq!(tree.len(), 5);
        assert_eq!(
            tree.get_all_key_values(),
            [
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(tree.remove(&4), Some(44444));
        assert_eq!(tree.len(), 4);
        assert_eq!(
            tree.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(tree.remove(&4), None);
        assert_eq!(tree.len(), 4);
        assert_eq!(
            tree.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(tree.insert(4, 44), None);
        assert_eq!(tree.len(), 5);
        assert_eq!(
            tree.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&4, &44), (&5, &555)]
        );

        assert_eq!(tree.insert(5, 55), Some(555));
        assert_eq!(tree.len(), 5);
        assert_eq!(
            tree.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&4, &44), (&5, &55)]
        );

        assert_eq!(tree.remove(&1), Some(1000));
        assert_eq!(tree.len(), 4);
        assert_eq!(
            tree.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44), (&5, &55)]
        );

        assert_eq!(tree.remove(&5), Some(55));
        assert_eq!(tree.len(), 3);
        assert_eq!(
            tree.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(tree.remove(&6), None);
        assert_eq!(tree.len(), 3);
        assert_eq!(
            tree.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(tree.get(&3), Some(&33));
        assert_eq!(tree.get(&2), Some(&2222));
        assert_eq!(tree.get(&100), None);

        assert_eq!(tree.get_top_k_key_values(1), [(&2, &2222)]);
        assert_eq!(tree.get_top_k_key_values(2), [(&2, &2222), (&3, &33)]);
        assert_eq!(
            tree.get_top_k_key_values(3),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );
        assert_eq!(
            tree.get_top_k_key_values(4),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(tree.remove(&3), Some(33));
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get_all_key_values(), [(&2, &2222), (&4, &44)]);

        assert_eq!(tree.insert(3, 33), None);
        assert_eq!(tree.len(), 3);
        assert_eq!(
            tree.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(tree.insert(3, 66), Some(33));
        assert_eq!(tree.len(), 3);
        assert_eq!(
            tree.get_all_key_values(),
            [(&2, &2222), (&3, &66), (&4, &44)]
        );

        assert_eq!(tree.remove(&5), None);
        assert_eq!(tree.len(), 3);
        assert_eq!(
            tree.get_all_key_values(),
            [(&2, &2222), (&3, &66), (&4, &44)]
        );

        assert_eq!(tree.remove(&4), Some(44));
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get_all_key_values(), [(&2, &2222), (&3, &66)]);
        assert_eq!(tree.get_top_k_key_values(1), [(&2, &2222)]);

        assert_eq!(tree.insert(3, 123), Some(66));
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get_all_key_values(), [(&2, &2222), (&3, &123)]);

        assert_eq!(tree.insert(2, 321), Some(2222));
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get_all_key_values(), [(&2, &321), (&3, &123)]);
        assert_eq!(tree.get_top_k_key_values(1), [(&2, &321)]);

        assert_eq!(tree.remove(&3), Some(123));
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.get_all_key_values(), [(&2, &321)]);

        assert_eq!(tree.remove(&3), None);
        assert_eq!(tree.len(), 1);
        assert_eq!(tree.get_all_key_values(), [(&2, &321)]);
        assert_eq!(tree.get_top_k_key_values(1), [(&2, &321)]);

        assert_eq!(tree.remove(&2), Some(321));
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.get_all_key_values(), []);

        assert_eq!(tree.remove(&2), None);
        assert_eq!(tree.len(), 0);
        assert_eq!(tree.get_all_key_values(), []);
    }

    #[test]
    fn sanity_test() {
        let mut numbers: Vec<usize> = (0..10000).collect();
        numbers.shuffle(&mut thread_rng());
        let mut tree = BTreeMap::<usize, usize>::new();
        let mut s = std::collections::BTreeSet::<usize>::new();
        numbers.iter().for_each(|&v| {
            assert_eq!(tree.insert(v, v), None);
            s.insert(v);
            assert_eq!(
                tree.get_all_key_values(),
                s.iter().map(|v| (v, v)).collect::<Vec<(&usize, &usize)>>()
            );
            assert_eq!(tree.len(), s.len());
        });
        numbers.shuffle(&mut thread_rng());
        numbers.iter().for_each(|&v| {
            assert_eq!(tree.remove(&v), Some(v));
            s.remove(&v);
            assert_eq!(
                tree.get_all_key_values(),
                s.iter().map(|v| (v, v)).collect::<Vec<(&usize, &usize)>>()
            );
            assert_eq!(tree.len(), s.len());
        });
    }
}
