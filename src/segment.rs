use std::cell::{Cell, RefCell};

pub(crate) struct Segment<'a, T> {
    data: Vec<Cell<&'a mut Option<T>>>,
    count: Option<usize>,
}

impl<'a, T> Segment<'a, T>
where
    T: Clone,
{
    #[inline]
    fn new(data: &'a [Option<T>]) -> Segment<T> {
        let mut cpcs = Vec::new();
        for x in data.iter() {
            cpcs.push(Cell::new(x));
        }
        Self {
            data: cpcs,
            //data: data.into_iter().map(|&'a x| Cell::from(x).get_mut()).collect(),
            count: None,
        }
    }

    fn get_count(&self) -> usize {
        if let Some(num) = self.count {
            num
        } else {
            let num = self.data.iter().filter(|&v| v.is_some()).count();
            *RefCell::new(self.count).get_mut() = Some(num);
            num
        }
    }

    fn move_all_to_front(&mut self) {
        let mut num = 0;
        for i in 0..self.data.len() {
            if self.data[i].is_some() {
                *RefCell::new(self.data[num].as_ref()).get_mut() =
                    RefCell::new(self.data[i].as_ref()).get_mut().take();
                num += 1;
            }
        }
        self.count = Some(num);
    }

    // Evenly distributed the items. Assuming all elements are at the beginning.
    fn shuffle(&mut self) {
        let num = self.get_count();
        if num == 0 {
            return;
        }
        let sub_len = self.data.len() / num;
        let remainder = self.data.len() % num;
        let mut j = self.data.len() - 1;
        for i in (0..num).rev() {
            *self.data[j] = self.data[i].take();
            j -= sub_len;
            if i < remainder && j > 0 {
                j -= 1;
            }
        }
    }

    fn insert(&mut self, index: usize, value: T) {
        if let Some(num) = self.count {
            self.count = Some(num + 1);
        }
        if self.data[index].is_some() {
            for i in index + 1..self.data.len() {
                if self.data[i].is_none() {
                    for j in (index..i).rev() {
                        *self.data[j + 1] = self.data[j].take();
                    }
                    *self.data[index] = Some(value);
                    return;
                }
            }
            for i in (0..index).rev() {
                if self.data[i].is_none() {
                    for j in i + 1..index {
                        *self.data[j - 1] = self.data[j].take();
                    }
                    *self.data[index - 1] = Some(value);
                    return;
                }
            }
            panic!("No space to insert");
        } else {
            *self.data[index] = Some(value);
            println!("inserted");
        }
    }

    #[inline]
    fn remove(&mut self, index: usize) {
        if self.data[index].is_some() {
            *self.data[index] = None;
            if let Some(num) = self.count {
                self.count = Some(num - 1);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::Segment;

    #[test]
    fn test_insert() {
        let mut v = vec![None; 5];
        let mut s = Segment::new(&mut v);
        assert_eq!(s.get_count(), 0);
        s.insert(0, 1);
        assert_eq!(v, [Some(1), None, None, None, None]);
        s.insert(1, 3);
        assert_eq!(v, [Some(1), Some(3), None, None, None]);
    }
}
