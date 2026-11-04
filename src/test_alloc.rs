//! Counts the allocations a closure makes on the calling thread. Only the
//! test build links this allocator.
#![allow(unsafe_code)]

use std::{
    alloc::{
        GlobalAlloc,
        Layout,
        System,
    },
    cell::Cell,
};

/// Allocations seen while observing: fresh allocations and reallocations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Allocations {
    pub(crate) allocations:        usize,
    pub(crate) reallocations:      usize,
    pub(crate) allocated_bytes:    usize,
    pub(crate) deallocated_bytes:  usize,
    pub(crate) realloc_from_bytes: usize,
    pub(crate) realloc_to_bytes:   usize,
    pub(crate) live_bytes:         isize,
    pub(crate) peak_bytes:         usize,
}

impl Allocations {
    pub(crate) fn total(self) -> usize {
        self.allocations + self.reallocations
    }

    /// All successful allocation requests, including the new realloc sizes.
    pub(crate) fn total_bytes(self) -> usize {
        self.allocated_bytes + self.realloc_to_bytes
    }

    fn change_live(&mut self, change: isize) {
        self.live_bytes += change;
        self.peak_bytes = self.peak_bytes.max(self.live_bytes.max(0) as usize);
    }
}

thread_local! {
    static OBSERVING: Cell<bool> = const { Cell::new(false) };
    static SEEN: Cell<Allocations> = const {
        Cell::new(Allocations {
            allocations:   0,
            reallocations: 0,
            allocated_bytes: 0,
            deallocated_bytes: 0,
            realloc_from_bytes: 0,
            realloc_to_bytes: 0,
            live_bytes: 0,
            peak_bytes: 0,
        })
    };
}

fn note(update: impl FnOnce(&mut Allocations)) {
    let _ = OBSERVING.try_with(|observing| {
        if observing.get() {
            let _ = SEEN.try_with(|seen| {
                let mut value = seen.get();
                update(&mut value);
                seen.set(value);
            });
        }
    });
}

struct CountingAllocator;

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            note(|seen| {
                seen.allocations += 1;
                seen.allocated_bytes += layout.size();
                seen.change_live(layout.size() as isize);
            });
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            note(|seen| {
                seen.allocations += 1;
                seen.allocated_bytes += layout.size();
                seen.change_live(layout.size() as isize);
            });
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        note(|seen| {
            seen.deallocated_bytes += layout.size();
            seen.change_live(-(layout.size() as isize));
        });
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_ptr = unsafe { System.realloc(ptr, layout, new_size) };
        if !new_ptr.is_null() {
            note(|seen| {
                seen.reallocations += 1;
                seen.realloc_from_bytes += layout.size();
                seen.realloc_to_bytes += new_size;
                seen.change_live(new_size as isize - layout.size() as isize);
            });
        }
        new_ptr
    }
}

/// Runs `operation` and returns its result with the allocations it made on
/// this thread. The result is dropped by the caller, outside the observation.
pub(crate) fn observe<T>(operation: impl FnOnce() -> T) -> (T, Allocations) {
    observe_with_live_bytes(0, operation)
}

/// Starts a scope with bytes transferred from an existing owner. Live bytes
/// are relative to this baseline; unrelated pre-scope drops can be negative.
pub(crate) fn observe_with_live_bytes<T>(
    initial_live_bytes: usize,
    operation: impl FnOnce() -> T,
) -> (T, Allocations) {
    /// Stops observing when the operation ends, including by unwinding.
    struct StopObserving;

    impl Drop for StopObserving {
        fn drop(&mut self) {
            let _ = OBSERVING.try_with(|observing| observing.set(false));
        }
    }

    OBSERVING.with(|observing| assert!(!observing.get(), "nested allocation observation"));
    SEEN.with(|seen| {
        seen.set(Allocations {
            live_bytes: initial_live_bytes as isize,
            peak_bytes: initial_live_bytes,
            ..Allocations::default()
        });
    });
    OBSERVING.with(|observing| observing.set(true));
    let result = {
        let _stop = StopObserving;
        operation()
    };
    (result, SEEN.with(Cell::get))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Exact raw layouts distinguish request bytes from retained bytes and
    // exercise both directions of realloc, zeroing and deallocation.
    #[test]
    fn allocation_bytes_follow_growth_shrink_and_release() {
        let ((), seen) = observe(|| unsafe {
            let small = Layout::from_size_align(13, 1).unwrap();
            let ptr = std::alloc::alloc(small);
            assert!(!ptr.is_null());
            let zeroed = Layout::from_size_align(7, 1).unwrap();
            let other = std::alloc::alloc_zeroed(zeroed);
            assert!(!other.is_null());
            assert_eq!(*other, 0);
            let ptr = std::alloc::realloc(ptr, small, 29);
            assert!(!ptr.is_null());
            let grown = Layout::from_size_align(29, 1).unwrap();
            let ptr = std::alloc::realloc(ptr, grown, 5);
            assert!(!ptr.is_null());
            std::alloc::dealloc(ptr, Layout::from_size_align(5, 1).unwrap());
            std::alloc::dealloc(other, zeroed);
        });
        assert_eq!(seen.allocations, 2);
        assert_eq!(seen.reallocations, 2);
        assert_eq!(seen.allocated_bytes, 13 + 7);
        assert_eq!(seen.realloc_from_bytes, 13 + 29);
        assert_eq!(seen.realloc_to_bytes, 29 + 5);
        assert_eq!(seen.total_bytes(), 13 + 7 + 29 + 5);
        assert_eq!(seen.deallocated_bytes, 5 + 7);
        assert_eq!(seen.live_bytes, 0);
        assert_eq!(seen.peak_bytes, 29 + 7);
    }

    // Moved storage is charged at scope entry, rather than vanishing from a
    // structure's byte measurement because its allocation preceded the scope.
    #[test]
    fn transferred_storage_is_included_in_scoped_live_bytes() {
        let text = String::from("transferred");
        assert_eq!(text.capacity(), 11);
        let ((), seen) = observe_with_live_bytes(11, || drop(text));
        assert_eq!(seen.total(), 0);
        assert_eq!(seen.deallocated_bytes, 11);
        assert_eq!(seen.live_bytes, 0);
        assert_eq!(seen.peak_bytes, 11);
    }

    // A returned allocation remains live at the boundary; a later scope can
    // see a pre-existing drop without unsigned byte-accounting underflow.
    #[test]
    fn retained_and_preexisting_bytes_keep_signed_scope_balance() {
        let (text, seen) = observe(|| String::from("kept"));
        assert_eq!((seen.total(), seen.total_bytes()), (1, 4));
        assert_eq!((seen.live_bytes, seen.peak_bytes), (4, 4));
        let ((), released) = observe(|| drop(text));
        assert_eq!(released.deallocated_bytes, 4);
        assert_eq!((released.live_bytes, released.peak_bytes), (-4, 0));
    }
}
