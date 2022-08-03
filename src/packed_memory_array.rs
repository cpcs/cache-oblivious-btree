#![allow(dead_code)]

use crate::segment::Segment;
use num_rational::Ratio;

pub(crate) struct PackedMemoryArray<K: Clone + Ord, V: Clone> {
    v: Vec<Option<(K, V)>>,
    data: Vec<*mut Option<(K, V)>>,
    height: usize,
    segment_size_log2: usize,
    segment_size: usize,
}

impl<K, V> PackedMemoryArray<K, V>
where
    K: Clone + Ord,
    V: Clone,
{
    #[inline]
    pub(crate) fn new() -> Self {
        let mut v = vec![None];
        Self {
            data: vec![&mut v[0]],
            v,
            height: 1,
            segment_size_log2: 0,
            segment_size: 1,
        }
    }

    #[inline]
    fn insert_density_ok(&self, depth: usize, count: usize, size: usize) -> bool {
        // (1 / 4) + 3 * (d / height) * 4 = (height * 3 + d) / (height * 4)
        Ratio::new_raw(count, size) <= Ratio::new_raw(self.height * 3 + depth, self.height << 2)
    }

    #[inline]
    fn remove_density_ok(&self, depth: usize, count: usize, size: usize) -> bool {
        // (1 / 2) - (d / height) / 4 = (height * 2 - d) / (height * 4)
        Ratio::new_raw(count, size) >= Ratio::new_raw((self.height << 1) - depth, self.height << 2)
    }

    // 0 <= index <= data.len(), Note: index == num is special.
    pub(crate) fn insert(&mut self, index: usize, key_value: (K, V)) -> Option<(usize, usize)> {
        let segment_id =
            (index >> self.segment_size_log2) - if self.data.len() == index { 1 } else { 0 };
        let mut segment_pos = if self.data.len() == index {
            self.segment_size
        } else {
            index & (self.segment_size - 1)
        };
        let mut from = segment_id << self.segment_size_log2;
        let mut to = from + self.segment_size;
        let mut size = self.segment_size;
        let mut count = Segment::new(&self.data[from..to], None).get_count();
        let mut found_segment = false;
        let mut density_ok = false;
        if count < size {
            found_segment = true;
            count += 1;
            density_ok = self.insert_density_ok(self.height - 1, count, size);
        }
        // Complicated logic to avoid key_value to be moved.
        // Note: If we found a segment, even if from and to changed, the final segment is still feasible
        // to insert.
        if !found_segment || !density_ok {
            for depth in (0..self.height - 1).rev() {
                if ((from / size) & 1) > 0 {
                    // Previous is the right child, need to add the left child.
                    count += Segment::new(&self.data[(from - size)..from], None).get_count();
                    segment_pos += size;
                    from -= size;
                } else {
                    // Previous is the left child, need to add the right child.
                    count += Segment::new(&self.data[to..(to + size)], None).get_count();
                    to += size;
                }
                size <<= 1;
                if !found_segment && count < size {
                    count += 1;
                    found_segment = true;
                }
                if found_segment && self.insert_density_ok(depth, count, size) {
                    density_ok = true;
                    break;
                }
            }
        }
        assert!(found_segment);
        if density_ok {
            let mut segment = Segment::new(&self.data[from..to], Some(count - 1));
            segment.insert_key_value(segment_pos, key_value);
            segment.shuffle_key_values(true);
            return Some((from, to));
        }
        self.v.resize(size << 1, None);
        self.data = self
            .v
            .iter_mut()
            .map(|v| v as *mut Option<(K, V)>)
            .collect();
        if self.height - 1 == self.segment_size_log2 {
            self.height += 1;
        } else {
            self.segment_size_log2 += 1;
            self.segment_size <<= 1;
        }
        let mut segment = Segment::new(&self.data, Some(count - 1));
        segment.insert_key_value(segment_pos, key_value);
        segment.shuffle_key_values(true);
        None
    }

    // 0 <= index < data.len().
    pub(crate) fn remove(&mut self, index: usize) -> Option<(usize, usize)> {
        let segment_id = index >> self.segment_size_log2;
        let segment_pos = index & (self.segment_size - 1);
        let mut from = self.segment_size * segment_id;
        let mut to = from + self.segment_size;
        let mut segment = Segment::new(&self.data[from..to], None);
        segment.remove_key_value(segment_pos);
        let mut count = segment.get_count();
        let mut size = self.segment_size;
        if self.remove_density_ok(self.height - 1, count, size) {
            Segment::new(&self.data[from..to], Some(count)).shuffle_key_values(true);
            return Some((from, to));
        }
        for depth in (0..self.height - 1).rev() {
            if ((from / size) & 1) > 0 {
                // Current is the right child, need to add the left child.
                count += Segment::new(&self.data[(from - size)..from], None).get_count();
                from -= size;
            } else {
                // Current is the left child, need to add the right child.
                count += Segment::new(&self.data[to..(to + size)], None).get_count();
                to += size;
            }
            size <<= 1;
            if self.remove_density_ok(depth, count, size) {
                Segment::new(&self.data[from..to], Some(count)).shuffle_key_values(true);
                return Some((from, to));
            }
        }
        assert!(self.data.len() == size);
        if count == 0 {
            *self = Self::new();
            return None;
        }
        Segment::new(&self.data, Some(count)).move_all_key_values_to_front();
        self.v.resize(size >> 1, None);
        self.data = self
            .v
            .iter_mut()
            .map(|v| v as *mut Option<(K, V)>)
            .collect();
        Segment::new(&self.data, Some(count)).shuffle_key_values(false);
        if self.height - 1 == self.segment_size_log2 {
            self.segment_size_log2 -= 1;
            self.segment_size >>= 1;
        } else {
            self.height -= 1;
        }
        None
    }
}

#[cfg(test)]
mod packed_memory_array {
    use crate::packed_memory_array::PackedMemoryArray;

    #[test]
    fn test_operations() {
        let mut pma = PackedMemoryArray::<usize, usize>::new();
        assert_eq!(pma.v, [None]);
        assert_eq!(pma.height, 1);
        assert_eq!(pma.segment_size, 1);
        assert_eq!(pma.segment_size_log2, 0);

        assert_eq!(pma.insert(1, (100, 10)), None);
        assert_eq!(pma.v, [None, Some((100, 10))]);
        assert_eq!(pma.height, 2);
        assert_eq!(pma.segment_size, 1);
        assert_eq!(pma.segment_size_log2, 0);

        assert_eq!(pma.insert(2, (200, 22)), None);
        assert_eq!(pma.v, [None, Some((100, 10)), None, Some((200, 22))]);
        assert_eq!(pma.height, 2);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.insert(3, (150, 11)), Some((0, 4)));
        assert_eq!(
            pma.v,
            [None, Some((100, 10)), Some((150, 11)), Some((200, 22))]
        );
        assert_eq!(pma.height, 2);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.insert(0, (88, 8)), None);
        assert_eq!(
            pma.v,
            [
                None,
                Some((88, 8)),
                None,
                Some((100, 10)),
                None,
                Some((150, 11)),
                None,
                Some((200, 22))
            ],
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.insert(2, (99, 9)), Some((0, 4)));
        assert_eq!(
            pma.v,
            [
                None,
                Some((88, 8)),
                Some((99, 9)),
                Some((100, 10)),
                None,
                Some((150, 11)),
                None,
                Some((200, 22))
            ],
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.insert(8, (250, 25)), Some((4, 8)));
        assert_eq!(
            pma.v,
            [
                None,
                Some((88, 8)),
                Some((99, 9)),
                Some((100, 10)),
                None,
                Some((150, 11)),
                Some((200, 22)),
                Some((250, 25))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.insert(6, (166, 66)), None);
        assert_eq!(
            pma.v,
            [
                None,
                None,
                Some((88, 8)),
                None,
                None,
                Some((99, 9)),
                None,
                Some((100, 10)),
                None,
                Some((150, 11)),
                None,
                Some((166, 66)),
                None,
                Some((200, 22)),
                None,
                Some((250, 25))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 4);
        assert_eq!(pma.segment_size_log2, 2);

        assert_eq!(pma.insert(13, (199, 19)), Some((12, 16)));
        assert_eq!(
            pma.v,
            [
                None,
                None,
                Some((88, 8)),
                None,
                None,
                Some((99, 9)),
                None,
                Some((100, 10)),
                None,
                Some((150, 11)),
                None,
                Some((166, 66)),
                None,
                Some((199, 19)),
                Some((200, 22)),
                Some((250, 25))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 4);
        assert_eq!(pma.segment_size_log2, 2);

        assert_eq!(pma.remove(13), Some((12, 16)));
        assert_eq!(
            pma.v,
            [
                None,
                None,
                Some((88, 8)),
                None,
                None,
                Some((99, 9)),
                None,
                Some((100, 10)),
                None,
                Some((150, 11)),
                None,
                Some((166, 66)),
                None,
                Some((200, 22)),
                None,
                Some((250, 25))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 4);
        assert_eq!(pma.segment_size_log2, 2);

        assert_eq!(pma.remove(11), None);
        assert_eq!(
            pma.v,
            [
                None,
                Some((88, 8)),
                None,
                Some((99, 9)),
                Some((100, 10)),
                Some((150, 11)),
                Some((200, 22)),
                Some((250, 25))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.remove(7), Some((6, 8)));
        assert_eq!(
            pma.v,
            [
                None,
                Some((88, 8)),
                None,
                Some((99, 9)),
                Some((100, 10)),
                Some((150, 11)),
                None,
                Some((200, 22))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.remove(4), Some((4, 6)));
        assert_eq!(
            pma.v,
            [
                None,
                Some((88, 8)),
                None,
                Some((99, 9)),
                None,
                Some((150, 11)),
                None,
                Some((200, 22))
            ]
        );
        assert_eq!(pma.height, 3);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.remove(1), None);
        assert_eq!(
            pma.v,
            [None, Some((99, 9)), Some((150, 11)), Some((200, 22))]
        );
        assert_eq!(pma.height, 2);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.remove(1), Some((0, 4)));
        assert_eq!(pma.v, [None, Some((150, 11)), None, Some((200, 22))]);
        assert_eq!(pma.height, 2);
        assert_eq!(pma.segment_size, 2);
        assert_eq!(pma.segment_size_log2, 1);

        assert_eq!(pma.remove(3), None);
        assert_eq!(pma.v, [None, Some((150, 11))]);
        assert_eq!(pma.height, 2);
        assert_eq!(pma.segment_size, 1);
        assert_eq!(pma.segment_size_log2, 0);

        assert_eq!(pma.remove(1), None);
        assert_eq!(pma.v, [None]);
        assert_eq!(pma.height, 1);
        assert_eq!(pma.segment_size, 1);
        assert_eq!(pma.segment_size_log2, 0);
    }

    #[test]
    fn test_restrictions() {
        let mut pma = PackedMemoryArray::<usize, usize>::new();
        for i in 0usize..10000usize {
            pma.insert(pma.v.len(), (i, i));
            assert!(pma.height - 1 >= pma.segment_size_log2);
            assert!(pma.height - 1 - pma.segment_size_log2 <= 1);
            assert!(pma.segment_size == (1 << pma.segment_size_log2));
            assert!(pma.v.len() == pma.data.len());
            assert!(pma.v.len() == pma.segment_size * (1 << (pma.height - 1)));
            let v = pma
                .v
                .iter()
                .filter_map(|&v| v)
                .collect::<Vec<(usize, usize)>>();
            assert_eq!(v.len(), (i + 1));
            v.iter()
                .enumerate()
                .for_each(|(i, &v)| assert_eq!(v, (i, i)));
        }
        assert_eq!(pma.v.len(), 16384);
        for i in 0usize..10000usize {
            pma.remove(pma.v.iter().position(|v| v.is_some()).unwrap());
            assert!(pma.height - 1 >= pma.segment_size_log2);
            assert!(pma.height - 1 - pma.segment_size_log2 <= 1);
            assert!(pma.segment_size == (1 << pma.segment_size_log2));
            assert!(pma.v.len() == pma.data.len());
            assert!(pma.v.len() == pma.segment_size * (1 << (pma.height - 1)));
            let v = pma
                .v
                .iter()
                .filter_map(|&v| v)
                .collect::<Vec<(usize, usize)>>();
            assert_eq!(v.len(), 9999 - i);
            for j in 1..v.len() {
                assert!(v[j - 1].1 < v[j].1);
            }
        }
        assert_eq!(pma.v.len(), 1);
    }
}
