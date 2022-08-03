#![allow(dead_code)]

pub(crate) struct Segment<'a, K: Clone + Ord, V: Clone> {
    data: &'a [*mut Option<(K, V)>],
    count: usize,
}

impl<'a, K, V> Segment<'a, K, V>
where
    K: Clone + Ord,
    V: Clone,
{
    #[inline]
    pub(crate) fn new(data: &'a [*mut Option<(K, V)>], count: Option<usize>) -> Segment<'a, K, V> {
        Self {
            data,
            count: unsafe {
                match count {
                    Some(c) => c,
                    None => data.iter().filter(|&&v| (*v).is_some()).count(),
                }
            },
        }
    }

    #[inline]
    pub(crate) fn get_count(&self) -> usize {
        self.count
    }

    #[inline]
    fn move_key_value(&self, src: usize, dst: usize) {
        if src == dst {
            return;
        }
        unsafe {
            *self.data[dst] = (*self.data[src]).take();
        }
    }

    #[inline]
    fn move_key_value_if_src_not_none(&self, src: usize, dst: usize) -> bool {
        unsafe {
            if (*self.data[src]).is_none() {
                return false;
            }
            if src != dst {
                *self.data[dst] = (*self.data[src]).take();
            }
        }
        true
    }

    pub(crate) fn move_all_key_values_to_front(&self) {
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
    pub(crate) fn shuffle_key_values(&self, need_to_move_to_front: bool) {
        if self.count == 0 {
            return;
        }
        if need_to_move_to_front {
            self.move_all_key_values_to_front();
        }
        let sub_len = self.data.len() / self.count;
        let remainer = self.data.len() % self.count;
        let mut j = self.data.len() - 1;
        for i in (0..self.count).rev() {
            self.move_key_value(i, j);
            if j < sub_len {
                assert!(i == 0);
                break;
            }
            j -= sub_len;
            if i < remainer && j > 0 {
                j -= 1;
            }
        }
    }

    fn set_key_value(&mut self, index: usize, key_value: (K, V)) {
        unsafe {
            assert!((*self.data[index]).is_none());
            *self.data[index] = Some(key_value);
        }
        self.count += 1;
    }

    // Try inserting a value on index.
    // If values are sorted, the position should be the index that is
    // larger than the inserted value.
    // Note: it's possible to have position == data.len() to insert
    // a value after the right-most one, in this case, exisiting values
    // may only be moved left.
    pub(crate) fn insert_key_value(&mut self, position: usize, key_value: (K, V)) {
        // Insert on index, try moving right first (possible no moving).
        for i in position..self.data.len() {
            unsafe {
                if (*self.data[i]).is_none() {
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
                if (*self.data[i]).is_none() {
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
    pub(crate) fn remove_key_value(&mut self, index: usize) {
        unsafe {
            if (*self.data[index]).is_some() {
                *self.data[index] = None;
                self.count -= 1;
            }
        }
    }
}

#[cfg(test)]
mod segment {
    use super::Segment;

    #[test]
    fn test_operations() {
        let mut v: Vec<Option<(usize, usize)>> = vec![None; 5];
        let data = v
            .iter_mut()
            .map(|v| v as *mut Option<(usize, usize)>)
            .collect::<Vec<*mut Option<(usize, usize)>>>();
        let mut s = Segment::new(&data, None);
        assert_eq!(s.get_count(), 0);

        s.insert_key_value(3, (11, 1111));
        assert_eq!(v, [None, None, None, Some((11, 1111)), None]);
        assert_eq!(s.get_count(), 1);

        s.insert_key_value(2, (8, 888));
        assert_eq!(v, [None, None, Some((8, 888)), Some((11, 1111)), None]);
        assert_eq!(s.get_count(), 2);

        s.insert_key_value(3, (10, 1010));
        assert_eq!(
            v,
            [
                None,
                None,
                Some((8, 888)),
                Some((10, 1010)),
                Some((11, 1111)),
            ]
        );
        assert_eq!(s.get_count(), 3);

        s.insert_key_value(3, (9, 999));
        assert_eq!(
            v,
            [
                None,
                Some((8, 888)),
                Some((9, 999)),
                Some((10, 1010)),
                Some((11, 1111))
            ]
        );
        assert_eq!(s.get_count(), 4);

        s.insert_key_value(5, (12, 1212));
        assert_eq!(
            v,
            [
                Some((8, 888)),
                Some((9, 999)),
                Some((10, 1010)),
                Some((11, 1111)),
                Some((12, 1212)),
            ]
        );

        s.remove_key_value(0);
        assert_eq!(
            v,
            [
                None,
                Some((9, 999)),
                Some((10, 1010)),
                Some((11, 1111)),
                Some((12, 1212)),
            ]
        );
        assert_eq!(s.get_count(), 4);

        s.remove_key_value(2);
        assert_eq!(
            v,
            [
                None,
                Some((9, 999)),
                None,
                Some((11, 1111)),
                Some((12, 1212)),
            ]
        );
        assert_eq!(s.get_count(), 3);

        s.insert_key_value(5, (15, 1515));
        assert_eq!(
            v,
            [
                None,
                Some((9, 999)),
                Some((11, 1111)),
                Some((12, 1212)),
                Some((15, 1515)),
            ]
        );
        assert_eq!(s.get_count(), 4);

        s.remove_key_value(2);
        assert_eq!(
            v,
            [
                None,
                Some((9, 999)),
                None,
                Some((12, 1212)),
                Some((15, 1515)),
            ]
        );
        assert_eq!(s.get_count(), 3);

        s.shuffle_key_values(true);
        assert_eq!(
            v,
            [
                None,
                Some((9, 999)),
                None,
                Some((12, 1212)),
                Some((15, 1515)),
            ]
        );
        assert_eq!(s.get_count(), 3);

        s.remove_key_value(4);
        assert_eq!(v, [None, Some((9, 999)), None, Some((12, 1212)), None,]);
        assert_eq!(s.get_count(), 2);

        s.shuffle_key_values(true);
        assert_eq!(v, [None, None, Some((9, 999)), None, Some((12, 1212)),]);
        assert_eq!(s.get_count(), 2);

        s.move_all_key_values_to_front();
        assert_eq!(v, [Some((9, 999)), Some((12, 1212)), None, None, None,]);
        assert_eq!(s.get_count(), 2);
    }
}
