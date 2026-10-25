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
    pub(crate) allocations:   usize,
    pub(crate) reallocations: usize,
}

impl Allocations {
    pub(crate) fn total(self) -> usize {
        self.allocations + self.reallocations
    }
}

thread_local! {
    static OBSERVING: Cell<bool> = const { Cell::new(false) };
    static SEEN: Cell<Allocations> = const {
        Cell::new(Allocations {
            allocations:   0,
            reallocations: 0,
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
        note(|seen| seen.allocations += 1);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        note(|seen| seen.allocations += 1);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        note(|seen| seen.reallocations += 1);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

/// Runs `operation` and returns its result with the allocations it made on
/// this thread. The result is dropped by the caller, outside the observation.
pub(crate) fn observe<T>(operation: impl FnOnce() -> T) -> (T, Allocations) {
    /// Stops observing when the operation ends, including by unwinding.
    struct StopObserving;

    impl Drop for StopObserving {
        fn drop(&mut self) {
            let _ = OBSERVING.try_with(|observing| observing.set(false));
        }
    }

    SEEN.with(|seen| seen.set(Allocations::default()));
    OBSERVING.with(|observing| observing.set(true));
    let result = {
        let _stop = StopObserving;
        operation()
    };
    (result, SEEN.with(Cell::get))
}
