#![allow(dead_code)]
use crate::packed_memory_array::PackedMemoryArray;

#[derive(Clone, Eq, PartialEq)]
enum Node<K: Clone + Ord> {
    Branch(BranchType<K>),
    Leaf(LeafType<K>),
}

#[derive(Clone, Eq, PartialEq)]
struct LeafType<K: Clone + Ord> {
    key: Option<K>,
}

#[derive(Clone, Eq, PartialEq)]
struct BranchType<K: Clone + Ord> {
    key: Option<K>,
}

fn compute_node_id_internal(n: usize, d: usize, height: usize) -> usize {
    if height < 3 {
        n
    } else {
        let h2 = ((height + 1) >> 1).next_power_of_two();
        let h1 = height - h2;
        if d <= h1 {
            compute_node_id_internal(n, d, h1)
        } else {
            let d2 = d - h1;
            let d1 = d2 - 1;
            (1 << h1) - 1
                + ((1 << h2) - 1) * ((n >> d1) - (1 << h1))
                + compute_node_id_internal((1 << d1) | (n & ((1 << d1) - 1)), d2, h2)
        }
    }
}

fn compute_node_id(n: usize, height: usize) -> usize {
    if height < 3 {
        n
    } else {
        let mut log2 = 1usize;
        while (n >> log2) > 0 {
            log2 += 1;
        }
        compute_node_id_internal(n, log2, height)
    }
}

impl<K> Node<K>
where
    K: Clone + Ord,
{
    #[inline]
    fn get_key(&self) -> Option<&K> {
        match self {
            Node::Branch(branch) => branch.key.as_ref(),
            Node::Leaf(leaf) => leaf.key.as_ref(),
        }
    }

    #[inline]
    // Set the key for the leaf node.
    // Returns whether the key changed.
    fn set_leaf_key(&mut self, input_key: Option<K>) -> bool {
        let mut key_changed = true;
        match self {
            Node::Branch(_) => panic!("Should only call this for leaf nodes."),
            Node::Leaf(leaf) => {
                if let Some(ref leaf_key) = leaf.key {
                    if let Some(ref key) = input_key {
                        key_changed = !key.eq(leaf_key);
                    }
                } else if input_key.is_none() {
                    // Both are None.
                    return false;
                }
                *self = Node::Leaf(LeafType { key: input_key });
            }
        }
        key_changed
    }
}

// This is to create the Van Emde Boas tree structure. The idea is in a paper.
// https://erikdemaine.org/papers/CacheObliviousBTrees_SICOMP/paper.pdf
// This is the cache oblivious version since by using this logic and if we put the tree nodes
// into an array using the specific order, we may reduce the number of memory loading.
pub struct BTreeMap<K: Ord + Clone, V: Clone> {
    height: usize,
    nodes: Vec<Node<K>>,
    pma: PackedMemoryArray<K, V>,
    size: usize,
}

impl<K, V> Default for BTreeMap<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> BTreeMap<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self {
            height: 1,
            nodes: vec![Node::Leaf(LeafType { key: None })],
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
        if index >= self.pma.data_len() {
            None
        } else {
            let first_leaf_id = 1usize << (self.height - 1);
            match self.nodes[self.compute_node_index(first_leaf_id + index)].get_key() {
                Some(k) => {
                    if !k.eq(key) {
                        return None;
                    }
                }
                None => return None,
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

    pub fn key_vec(&self) -> Vec<&K> {
        self.pma
            .get_key_values()
            .iter()
            .filter_map(|kv| kv.as_ref())
            .map(|kv| &kv.0)
            .collect::<Vec<&K>>()
    }

    pub fn value_vec(&self) -> Vec<&V> {
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
        if index >= self.pma.data_len() {
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
        self.nodes.resize(
            self.pma.data_len() << 1,
            Node::Branch(BranchType { key: None }),
        );
        self.height = (self.pma.data_len().trailing_zeros() + 1) as usize;
        let first_leaf_id = 1usize << (self.height - 1);
        for i in 1usize..(1 << self.height) {
            let index = self.compute_node_index(i);
            self.nodes[index] = if i < first_leaf_id {
                Node::Branch(BranchType { key: None })
            } else {
                Node::Leaf(LeafType { key: None })
            };
        }
        self.populate_changes(0, self.pma.data_len());
    }

    fn find_index(&self, key: &K) -> usize {
        let mut node_id = 1usize;
        let mut node_index = self.compute_node_index(node_id);
        let mut leaf_index = 0usize;
        while let Node::Branch(_) = &self.nodes[node_index] {
            leaf_index <<= 1;
            node_id <<= 1;
            node_index = self.compute_node_index(node_id);
            match self.nodes[node_index].get_key() {
                Some(k) => {
                    if k.lt(key) {
                        leaf_index |= 1;
                        node_id |= 1;
                    }
                }
                None => {
                    leaf_index |= 1;
                    node_id |= 1;
                }
            };
        }
        node_index = self.compute_node_index(node_id);
        if let Some(k) = self.nodes[node_index].get_key() {
            if k.lt(key) {
                leaf_index += 1;
            }
        }
        leaf_index
    }

    // Populated the changed leaves to root.
    fn populate_changes(&mut self, from: usize, to: usize) {
        let first_leaf_id = 1usize << (self.height - 1);
        let key_values = self.pma.get_key_values();
        let mut changed_nodes = Vec::with_capacity(self.nodes.len());
        for (i, key_value) in key_values.iter().enumerate().take(to).skip(from) {
            let leaf_id = first_leaf_id + i;
            let leaf_index = self.compute_node_index(leaf_id);
            let leaf = &mut self.nodes[leaf_index];
            if leaf.set_leaf_key(key_value.as_ref().map(|kv| kv.0.to_owned()))
                && leaf_id > 1
                && (changed_nodes.is_empty() || changed_nodes.last().unwrap() != &(leaf_id >> 1))
            {
                changed_nodes.push(leaf_id >> 1);
            }
        }

        let mut i = 0;
        while i < changed_nodes.len() {
            let changed_node_id = changed_nodes[i];
            let changed_node_index = self.compute_node_index(changed_node_id);
            let changed_node = &mut self.nodes[changed_node_index];
            match changed_node {
                Node::Branch(_) => {
                    if self.set_branch_key(
                        changed_node_index,
                        self.compute_node_index(changed_node_id << 1),
                        self.compute_node_index((changed_node_id << 1) | 1),
                    ) && changed_node_id > 1
                        && changed_nodes.last().unwrap() != &(changed_node_id >> 1)
                    {
                        changed_nodes.push(changed_node_id >> 1);
                    }
                }
                Node::Leaf(_) => panic!("Should not reach here"),
            }
            i += 1;
        }
    }

    fn compute_node_index(&self, x: usize) -> usize {
        compute_node_id(x, self.height) - 1
    }

    #[inline]
    // Set the key for this node as the maximum key of the left and right children.
    // Return whether the key is changed or not.
    fn set_branch_key(&mut self, node_index: usize, left_index: usize, right_index: usize) -> bool {
        if let Node::Branch(branch) = &self.nodes[node_index] {
            let right_key = self.nodes[right_index].get_key();
            let input_key = if right_key.is_none() {
                self.nodes[left_index].get_key()
            } else {
                right_key
            };

            match &branch.key {
                Some(k) => match input_key {
                    Some(ik) => {
                        let changed = !k.eq(ik);
                        self.nodes[node_index] = Node::Branch(BranchType {
                            key: Some(ik.to_owned()),
                        });
                        changed
                    }
                    None => {
                        self.nodes[node_index] = Node::Branch(BranchType { key: None });
                        true
                    }
                },

                None => match input_key {
                    Some(ik) => {
                        self.nodes[node_index] = Node::Branch(BranchType {
                            key: Some(ik.to_owned()),
                        });
                        true
                    }
                    None => false,
                },
            }
        } else {
            panic!("Should only set key for branch node.");
        }
    }
}

#[cfg(test)]
mod btree_map {
    use crate::cache_oblivious::{compute_node_id_internal, BTreeMapgi};
    use float_ord::FloatOrd;
    use rand::{seq::SliceRandom, thread_rng};

    // The excatly tree was shown by the paper.
    // https://ibb.co/BtmrpDz
    #[test]
    fn test_node_ids() {
        let answer = vec![
            1usize, 2, 17, 3, 4, 18, 19, 5, 8, 11, 14, 20, 23, 26, 29, 6, 7, 9, 10, 12, 13, 15, 16,
            21, 22, 24, 25, 27, 28, 30, 31,
        ];
        for (n, &id) in answer.iter().enumerate() {
            let mut log2 = 1usize;
            let d = n + 1;
            while (d >> log2) > 0 {
                log2 += 1;
            }
            assert_eq!(compute_node_id_internal(d, log2, 5), id);
        }
    }

    #[test]
    fn test_other_keys() {
        let mut map = BTreeMap::<FloatOrd<f32>, usize>::new();
        for i in 1000..1128 {
            assert_eq!(map.insert(FloatOrd(i as f32), i), None);
            assert_eq!(map.len(), i - 999);
        }

        for i in 1000..1128 {
            assert_eq!(map.remove(&FloatOrd(i as f32)), Some(i));
            assert_eq!(map.len(), 127);
            assert_eq!(map.insert(FloatOrd((i + 128) as f32), i + 128), None);
            assert_eq!(map.len(), 128);
        }
    }

    #[test]
    fn test_opertions() {
        let mut map = BTreeMap::<usize, usize>::new();
        assert_eq!(map.len(), 0);
        assert_eq!(map.get_all_key_values(), []);

        assert_eq!(map.insert(1, 11), None);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get_all_key_values(), [(&1, &11)]);

        assert_eq!(map.insert(3, 33), None);
        assert_eq!(map.len(), 2);
        assert_eq!(map.get_all_key_values(), [(&1, &11), (&3, &33)]);

        assert_eq!(map.insert(0, 0), None);
        assert_eq!(map.len(), 3);
        assert_eq!(map.get_all_key_values(), [(&0, &0), (&1, &11), (&3, &33)]);

        assert_eq!(map.insert(5, 555), None);
        assert_eq!(map.len(), 4);
        assert_eq!(
            map.get_all_key_values(),
            [(&0, &0), (&1, &11), (&3, &33), (&5, &555)]
        );

        assert_eq!(map.insert(2, 2222), None);
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.get_all_key_values(),
            [(&0, &0), (&1, &11), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(map.insert(1, 1000), Some(11));
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.get_all_key_values(),
            [(&0, &0), (&1, &1000), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(map.insert(4, 44444), None);
        assert_eq!(map.len(), 6);
        assert_eq!(
            map.get_all_key_values(),
            [
                (&0, &0),
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(map.insert(0, 44444), Some(0));
        assert_eq!(map.len(), 6);
        assert_eq!(
            map.get_all_key_values(),
            [
                (&0, &44444),
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(map.remove(&0), Some(44444));
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.get_all_key_values(),
            [
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(map.remove(&0), None);
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.get_all_key_values(),
            [
                (&1, &1000),
                (&2, &2222),
                (&3, &33),
                (&4, &44444),
                (&5, &555)
            ]
        );

        assert_eq!(map.remove(&4), Some(44444));
        assert_eq!(map.len(), 4);
        assert_eq!(
            map.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(map.remove(&4), None);
        assert_eq!(map.len(), 4);
        assert_eq!(
            map.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&5, &555)]
        );

        assert_eq!(map.insert(4, 44), None);
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&4, &44), (&5, &555)]
        );

        assert_eq!(map.insert(5, 55), Some(555));
        assert_eq!(map.len(), 5);
        assert_eq!(
            map.get_all_key_values(),
            [(&1, &1000), (&2, &2222), (&3, &33), (&4, &44), (&5, &55)]
        );

        assert_eq!(map.remove(&1), Some(1000));
        assert_eq!(map.len(), 4);
        assert_eq!(
            map.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44), (&5, &55)]
        );

        assert_eq!(map.remove(&5), Some(55));
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(map.remove(&6), None);
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(map.get(&3), Some(&33));
        assert_eq!(map.get(&2), Some(&2222));
        assert_eq!(map.get(&100), None);

        assert_eq!(map.get_top_k_key_values(1), [(&2, &2222)]);
        assert_eq!(map.get_top_k_key_values(2), [(&2, &2222), (&3, &33)]);
        assert_eq!(
            map.get_top_k_key_values(3),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );
        assert_eq!(
            map.get_top_k_key_values(4),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(map.remove(&3), Some(33));
        assert_eq!(map.len(), 2);
        assert_eq!(map.get_all_key_values(), [(&2, &2222), (&4, &44)]);

        assert_eq!(map.insert(3, 33), None);
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get_all_key_values(),
            [(&2, &2222), (&3, &33), (&4, &44)]
        );

        assert_eq!(map.insert(3, 66), Some(33));
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get_all_key_values(),
            [(&2, &2222), (&3, &66), (&4, &44)]
        );

        assert_eq!(map.remove(&5), None);
        assert_eq!(map.len(), 3);
        assert_eq!(
            map.get_all_key_values(),
            [(&2, &2222), (&3, &66), (&4, &44)]
        );

        assert_eq!(map.remove(&4), Some(44));
        assert_eq!(map.len(), 2);
        assert_eq!(map.get_all_key_values(), [(&2, &2222), (&3, &66)]);
        assert_eq!(map.get_top_k_key_values(1), [(&2, &2222)]);

        assert_eq!(map.insert(3, 123), Some(66));
        assert_eq!(map.len(), 2);
        assert_eq!(map.get_all_key_values(), [(&2, &2222), (&3, &123)]);

        assert_eq!(map.insert(2, 321), Some(2222));
        assert_eq!(map.len(), 2);
        assert_eq!(map.get_all_key_values(), [(&2, &321), (&3, &123)]);
        assert_eq!(map.get_top_k_key_values(1), [(&2, &321)]);

        assert_eq!(map.remove(&3), Some(123));
        assert_eq!(map.len(), 1);
        assert_eq!(map.get_all_key_values(), [(&2, &321)]);

        assert_eq!(map.remove(&3), None);
        assert_eq!(map.len(), 1);
        assert_eq!(map.get_all_key_values(), [(&2, &321)]);
        assert_eq!(map.get_top_k_key_values(1), [(&2, &321)]);

        assert_eq!(map.remove(&2), Some(321));
        assert_eq!(map.len(), 0);
        assert_eq!(map.get_all_key_values(), []);

        assert_eq!(map.remove(&2), None);
        assert_eq!(map.len(), 0);
        assert_eq!(map.get_all_key_values(), []);
    }

    #[test]
    fn sanity_test() {
        let mut numbers: Vec<usize> = (0..10000).collect();
        numbers.shuffle(&mut thread_rng());
        let mut map = BTreeMap::<usize, usize>::new();
        let mut s = std::collections::BTreeSet::<usize>::new();
        numbers.iter().for_each(|&v| {
            assert_eq!(map.insert(v, v), None);
            s.insert(v);
            assert_eq!(
                map.get_all_key_values(),
                s.iter().map(|v| (v, v)).collect::<Vec<(&usize, &usize)>>()
            );
            assert_eq!(map.len(), s.len());
        });
        numbers.shuffle(&mut thread_rng());
        numbers.iter().for_each(|&v| {
            assert_eq!(map.remove(&v), Some(v));
            s.remove(&v);
            assert_eq!(
                map.get_all_key_values(),
                s.iter().map(|v| (v, v)).collect::<Vec<(&usize, &usize)>>()
            );
            assert_eq!(map.len(), s.len());
        });
    }
}
