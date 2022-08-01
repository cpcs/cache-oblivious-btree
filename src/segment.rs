#![allow(dead_code)]

use crate::veb_tree::Node;

pub(crate) struct Segment<'a, K: Clone + Ord, V: Clone> {
    data: &'a [*mut Node<'a, K, V>],
    count: usize,
}

impl<'a, K, V> Segment<'a, K, V>
where
    K: Clone + Ord,
    V: Clone,
{
    #[inline]
    fn new(data: &'a [*mut Node<'a, K, V>], count: Option<usize>) -> Segment<'a, K, V> {
        Self {
            data,
            count: match count {
                Some(c) => c,
                None => data
                    .iter()
                    .filter(|&&v| unsafe { (*v).get_leaf_key_value_ref().is_some() })
                    .count(),
            },
        }
    }

    #[inline]
    fn move_key_value(&self, src: usize, dst: usize) {
        if src == dst {
            return;
        }
        unsafe {
            let src_key_value = (*self.data[src]).get_leaf_key_value_mut_ref();
            *(*self.data[dst]).get_leaf_key_value_mut_ref() = src_key_value.take();
        }
    }

    #[inline]
    fn move_key_value_if_src_not_none(&self, src: usize, dst: usize) -> bool {
        unsafe {
            let src_key_value = (*self.data[src]).get_leaf_key_value_mut_ref();
            if src_key_value.is_none() {
                return false;
            }
            if src != dst {
                *(*self.data[dst]).get_leaf_key_value_mut_ref() = src_key_value.take();
            }
        }
        true
    }

    fn move_all_key_values_to_front(&self) {
        if self.count == 0 {
            return;
        }
        let mut num = 0;
        for i in 0..self.data.len() {
            if self.move_key_value_if_src_not_none(i, num) {
                num += 1;
                if num == self.count {
                    break;
                }
            }
        }
    }

    // Evenly distribut the data.
    fn shuffle_key_values(&self) {
        if self.count == 0 {
            return;
        }
        self.move_all_key_values_to_front();
        let sub_len = self.data.len() / self.count;
        let remainer = self.data.len() % self.count;
        let mut j = self.data.len() - 1;
        for i in (0..self.count).rev() {
            self.move_key_value(i, j);
            j -= sub_len;
            if i < remainer && j > 0 {
                j -= 1;
            }
        }
    }

    fn set_key_value(&mut self, index: usize, key_value: (&'a K, V)) {
        unsafe {
            let kv = (*self.data[index]).get_leaf_key_value_mut_ref();
            assert!(kv.is_none());
            *kv = Some(key_value);
            self.count += 1;
        }
    }

    // Try inserting a value on index.
    // If values are sorted, the position should be the index that is
    // larger than the inserted value.
    // Note: it's possible to have position == data.len() to insert
    // a value after the right-most one, in this case, exisiting values
    // may only be moved left.
    fn insert_key_value(&mut self, position: usize, key_value: (&'a K, V)) {
        // Insert on index, try moving right first (possible no moving).
        for i in position..self.data.len() {
            unsafe {
                if (*self.data[i]).get_leaf_key_value_ref().is_none() {
                    for j in (position..i).rev() {
                        self.move_key_value(j, j + 1);
                    }
                    self.set_key_value(position, key_value);
                    return;
                }
            }
        }
        // Try inserting on position - 1, move other values to left.
        for i in (0..position).rev() {
            unsafe {
                if (*self.data[i]).get_leaf_key_value_ref().is_none() {
                    for j in i + 1..position {
                        self.move_key_value(j, j - 1);
                    }
                    self.set_key_value(position - 1, key_value);
                    return;
                }
            }
        }
        panic!("No space to insert");
    }

    #[inline]
    fn remove_key_value(&mut self, index: usize) {
        unsafe {
            let key_value = (*self.data[index]).get_leaf_key_value_mut_ref().take();
            if key_value.is_some() {
                self.count -= 1;
            }
        }
    }
}

#[cfg(test)]
mod segment {
    use super::Segment;
    use crate::veb_tree::{LeafType, Node, NodeType};
    use std::ptr::null_mut;

    #[test]
    fn test_operations() {
        let mut nodes: Vec<Node<usize, usize>> = vec![
            Node {
                parent: null_mut(),
                node_type: NodeType::Leaf(LeafType { key_value: None }),
            };
            5
        ];

        let data = nodes
            .iter_mut()
            .map(|node| node as *mut Node<usize, usize>)
            .collect::<Vec<*mut Node<usize, usize>>>();
        let mut s = Segment::new(&data, None);
        assert_eq!(s.count, 0);

        s.insert_key_value(3, (&11, 1111));
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
            ]
        );
        assert_eq!(s.count, 1);

        s.insert_key_value(2, (&8, 888));
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&8, 888))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
            ]
        );
        assert_eq!(s.count, 2);

        s.insert_key_value(3, (&10, 1010));
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&8, 888))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&10, 1010))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 3);

        s.insert_key_value(3, (&9, 999));
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&8, 888))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&10, 1010))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 4);

        s.insert_key_value(5, (&12, 1212));
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&8, 888))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&10, 1010))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 5);

        s.remove_key_value(0);
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&10, 1010))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 4);

        s.remove_key_value(2);
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 3);

        s.insert_key_value(5, (&15, 1515));
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&11, 1111))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&15, 1515))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 4);

        s.remove_key_value(2);
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&15, 1515))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 3);

        s.shuffle_key_values();
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&15, 1515))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 3);

        s.remove_key_value(4);
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
            ]
        );
        assert_eq!(s.count, 2);

        s.shuffle_key_values();
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
            ]
        );
        assert_eq!(s.count, 2);

        s.move_all_key_values_to_front();
        assert_eq!(
            nodes,
            [
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&9, 999))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType {
                        key_value: Some((&12, 1212))
                    }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
                Node {
                    parent: null_mut(),
                    node_type: NodeType::Leaf(LeafType { key_value: None }),
                },
            ]
        );
        assert_eq!(s.count, 2);
    }
}
