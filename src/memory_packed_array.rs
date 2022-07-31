// Given a sorted index in the range [1, 2^{tree_height}-1], return the VEB address
pub(crate) fn veb_index(n: usize, height: usize) -> usize {
    if height <= 1 {
        n
    } else {
        let bottom_height = (height >> 1).next_power_of_two();
        let top_height = height - bottom_height;
        let top_num = n >> bottom_height;
        let bottom_num = n & ((1 << bottom_height) - 1);
        if bottom_num == 0 {
            veb_index(top_num, top_height)
        } else {
            let top_size = (1 << top_height) - 1;
            let subtree_size = (1 << bottom_height) - 1;
            let top_address = top_num * subtree_size + top_size;
            let bot_address = veb_index(bottom_num, bottom_height);
            top_address + bot_address
        }
    }
}

#[cfg(test)]
mod test {
    use crate::memory_packed_array::veb_index;

    #[test]
    fn test_works() {
        for i in 1..16 {
            println!("{}", veb_index(i, 4));
        }
    }
}
