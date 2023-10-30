use core::ops::{
    Bound::{Excluded, Included, Unbounded},
    RangeBounds,
};
use core::ptr;

// REVIEW: rename to RangedRetain?
//         and rename methods to ranged_retain accordingly?
pub trait RetainRange<T> {
    fn retain_range<R, F>(&mut self, range: R, f: F)
    where
        R: RangeBounds<usize>,
        F: FnMut(&T) -> bool;

    fn retain_range_mut<R, F>(&mut self, range: R, f: F)
    where
        R: RangeBounds<usize>,
        F: FnMut(&mut T) -> bool;
}

impl<T> RetainRange<T> for Vec<T> {
    // modified from the original source for Vec::retain
    /// Extension of `Vec::retain` to operate only on part of the vector defined by a range.
    fn retain_range<R, F>(&mut self, range: R, mut f: F)
    where
        R: RangeBounds<usize>,
        F: FnMut(&T) -> bool,
    {
        self.retain_range_mut(range, |elem| f(elem));
    }

    // modified from the original source for Vec::retain_mut
    /// Extension of `Vec::retain_mut` to operate only on part of the vector defined by a range.
    fn retain_range_mut<R, F>(&mut self, range: R, mut f: F)
    where
        R: RangeBounds<usize>,
        F: FnMut(&mut T) -> bool,
    {
        let range_start_index = match range.start_bound() {
            Unbounded => 0,
            Included(&i) => i,
            Excluded(&i) => i + 1,
        };
        let range_end_index = match range.end_bound() {
            Unbounded => 0,
            Included(&i) => i + 1,
            Excluded(&i) => i,
        };

        let original_len = self.len();
        // Avoid double drop if the drop guard is not executed,
        // since we may make some holes during the process.
        unsafe { self.set_len(0) };

        // Vec: [Kept, Kept, Hole, Hole, Hole, Hole, Unchecked, Unchecked]
        //      |<-              processed len   ->| ^- next to check
        //                  |<-  deleted cnt     ->|
        //      |<-              original_len                          ->|
        // Kept: Elements which predicate returns true on.
        // Hole: Moved or dropped element slot.
        // Unchecked: Unchecked valid elements.
        //
        // This drop guard will be invoked when predicate or `drop` of element panicked.
        // It shifts unchecked elements to cover holes and `set_len` to the correct length.
        // In cases when predicate and `drop` never panick, it will be optimized out.
        struct BackshiftOnDrop<'a, T> {
            v: &'a mut Vec<T>,
            processed_len: usize,
            deleted_cnt: usize,
            original_len: usize,
        }

        impl<T> Drop for BackshiftOnDrop<'_, T> {
            fn drop(&mut self) {
                if self.deleted_cnt > 0 {
                    // SAFETY: Trailing unchecked items must be valid since we never touch them.
                    unsafe {
                        ptr::copy(
                            self.v.as_ptr().add(self.processed_len),
                            self.v
                                .as_mut_ptr()
                                .add(self.processed_len - self.deleted_cnt),
                            self.original_len - self.processed_len,
                        );
                    }
                }
                // SAFETY: After filling holes, all items are in contiguous memory.
                unsafe {
                    self.v.set_len(self.original_len - self.deleted_cnt);
                }
            }
        }

        let mut g = BackshiftOnDrop {
            v: self,
            processed_len: range_start_index,
            deleted_cnt: 0,
            original_len,
        };

        fn process_loop<F, T, const DELETED: bool>(
            end_index: usize,
            f: &mut F,
            g: &mut BackshiftOnDrop<'_, T>,
        ) where
            F: FnMut(&mut T) -> bool,
        {
            while g.processed_len != end_index {
                // SAFETY: Unchecked element must be valid.
                let cur = unsafe { &mut *g.v.as_mut_ptr().add(g.processed_len) };
                if !f(cur) {
                    // Advance early to avoid double drop if `drop_in_place` panicked.
                    g.processed_len += 1;
                    g.deleted_cnt += 1;
                    // SAFETY: We never touch this element again after dropped.
                    unsafe { ptr::drop_in_place(cur) };
                    // We already advanced the counter.
                    if DELETED {
                        continue;
                    } else {
                        break;
                    }
                }
                if DELETED {
                    // SAFETY: `deleted_cnt` > 0, so the hole slot must not overlap with current element.
                    // We use copy for move, and never touch this element again.
                    unsafe {
                        let hole_slot = g.v.as_mut_ptr().add(g.processed_len - g.deleted_cnt);
                        ptr::copy_nonoverlapping(cur, hole_slot, 1);
                    }
                }
                g.processed_len += 1;
            }
        }

        // Stage 1: Nothing was deleted.
        process_loop::<F, T, false>(range_end_index, &mut f, &mut g);

        // Stage 2: Some elements were deleted.
        process_loop::<F, T, true>(range_end_index, &mut f, &mut g);

        // All item are processed. This can be optimized to `set_len` by LLVM.
        drop(g);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut vec = vec![1, 2, 3, 4, 5];
        vec.retain_range(1..=3, |&x| x <= 2);
        assert_eq!(vec, [1, 2, 5]);
    }
}
