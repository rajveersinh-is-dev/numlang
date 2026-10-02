//! Scoped Arena Allocator Runtime for NumLang.
//!
//! Provides deterministic, bounded memory management with O(1) reset capability.
//! Supports both explicit arena handles (__nl_arena_*) and a thread-local default arena
//! used by native code generation.

use std::alloc::{alloc, dealloc, Layout};
use std::cell::RefCell;
use std::ptr;

pub const DEFAULT_CHUNK_CAPACITY: usize = 64 * 1024; // 64 KiB default chunk size

#[repr(C)]
pub struct ArenaChunk {
    pub data: *mut u8,
    pub capacity: usize,
    pub offset: usize,
    pub next: *mut ArenaChunk,
}

#[repr(C)]
pub struct Arena {
    pub head: *mut ArenaChunk,
    pub current: *mut ArenaChunk,
    pub default_chunk_size: usize,
    pub total_allocated: usize,
    pub peak_usage: usize,
}

impl Arena {
    pub fn new(capacity: usize) -> Self {
        let cap = if capacity == 0 { DEFAULT_CHUNK_CAPACITY } else { capacity };
        let chunk = Self::alloc_chunk(cap);
        Arena {
            head: chunk,
            current: chunk,
            default_chunk_size: cap,
            total_allocated: 0,
            peak_usage: 0,
        }
    }

    fn alloc_chunk(capacity: usize) -> *mut ArenaChunk {
        unsafe {
            let data_layout = Layout::from_size_align_unchecked(capacity, 16);
            let data = alloc(data_layout);
            assert!(!data.is_null(), "Arena: out of memory allocating chunk data");

            let chunk_layout = Layout::new::<ArenaChunk>();
            let chunk_ptr = alloc(chunk_layout) as *mut ArenaChunk;
            assert!(!chunk_ptr.is_null(), "Arena: out of memory allocating chunk header");

            ptr::write(chunk_ptr, ArenaChunk {
                data,
                capacity,
                offset: 0,
                next: ptr::null_mut(),
            });
            chunk_ptr
        }
    }

    pub fn alloc(&mut self, size: usize) -> *mut u8 {
        if size == 0 {
            return ptr::null_mut();
        }
        let aligned_size = (size + 7) & !7; // 8-byte alignment

        unsafe {
            let curr = self.current;
            if (*curr).offset + aligned_size <= (*curr).capacity {
                let res = (*curr).data.add((*curr).offset);
                (*curr).offset += aligned_size;
                self.total_allocated += aligned_size;
                if self.total_allocated > self.peak_usage {
                    self.peak_usage = self.total_allocated;
                }
                return res;
            }

            // Need next chunk or allocate a new one
            if !(*curr).next.is_null() && (*(*curr).next).capacity >= aligned_size {
                self.current = (*curr).next;
                let c = self.current;
                (*c).offset = aligned_size;
                self.total_allocated += aligned_size;
                if self.total_allocated > self.peak_usage {
                    self.peak_usage = self.total_allocated;
                }
                return (*c).data;
            }

            let new_cap = self.default_chunk_size.max(aligned_size * 2);
            let new_chunk = Self::alloc_chunk(new_cap);
            (*curr).next = new_chunk;
            self.current = new_chunk;
            (*new_chunk).offset = aligned_size;
            self.total_allocated += aligned_size;
            if self.total_allocated > self.peak_usage {
                self.peak_usage = self.total_allocated;
            }
            (*new_chunk).data
        }
    }

    pub fn reset(&mut self) {
        unsafe {
            let mut curr = self.head;
            while !curr.is_null() {
                (*curr).offset = 0;
                curr = (*curr).next;
            }
            self.current = self.head;
            self.total_allocated = 0;
        }
    }

    pub fn destroy(&mut self) {
        unsafe {
            let mut curr = self.head;
            while !curr.is_null() {
                let next = (*curr).next;
                let data_layout = Layout::from_size_align_unchecked((*curr).capacity, 16);
                dealloc((*curr).data, data_layout);
                let chunk_layout = Layout::new::<ArenaChunk>();
                dealloc(curr as *mut u8, chunk_layout);
                curr = next;
            }
            self.head = ptr::null_mut();
            self.current = ptr::null_mut();
            self.total_allocated = 0;
        }
    }
}

impl Drop for Arena {
    fn drop(&mut self) {
        self.destroy();
    }
}

thread_local! {
    static DEFAULT_ARENA: RefCell<Arena> = RefCell::new(Arena::new(DEFAULT_CHUNK_CAPACITY));
}

#[no_mangle]
pub extern "C" fn __nl_arena_create(capacity: usize) -> *mut Arena {
    let arena = Box::new(Arena::new(capacity));
    Box::into_raw(arena)
}

/// Allocate memory from the specified arena (or default arena if null).
///
/// # Safety
/// If `arena` is non-null, it must point to a valid initialized `Arena`.
#[no_mangle]
pub unsafe extern "C" fn __nl_arena_alloc(arena: *mut Arena, size: usize) -> *mut u8 {
    if arena.is_null() {
        return __nl_arena_alloc_default(size);
    }
    unsafe { (*arena).alloc(size) }
}

/// Reset the specified arena (or default arena if null).
///
/// # Safety
/// If `arena` is non-null, it must point to a valid initialized `Arena`.
#[no_mangle]
pub unsafe extern "C" fn __nl_arena_reset(arena: *mut Arena) {
    if arena.is_null() {
        __nl_loop_reset();
    } else {
        unsafe { (*arena).reset() }
    }
}

/// Destroy an arena previously created by `__nl_arena_create`.
///
/// # Safety
/// If `arena` is non-null, it must point to an `Arena` allocated via `Box::into_raw` and must not be used after.
#[no_mangle]
pub unsafe extern "C" fn __nl_arena_destroy(arena: *mut Arena) {
    if !arena.is_null() {
        unsafe {
            drop(Box::from_raw(arena));
        }
    }
}

#[no_mangle]
pub extern "C" fn __nl_arena_alloc_default(size: usize) -> *mut u8 {
    DEFAULT_ARENA.with(|a| a.borrow_mut().alloc(size))
}

#[no_mangle]
pub extern "C" fn __nl_loop_reset() {
    DEFAULT_ARENA.with(|a| a.borrow_mut().reset());
}

#[no_mangle]
pub extern "C" fn __nl_arena_get_allocated_bytes() -> usize {
    DEFAULT_ARENA.with(|a| a.borrow().total_allocated)
}

#[no_mangle]
pub extern "C" fn __nl_arena_get_peak_bytes() -> usize {
    DEFAULT_ARENA.with(|a| a.borrow().peak_usage)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_basic_lifecycle() {
        let arena_ptr = __nl_arena_create(1024);
        assert!(!arena_ptr.is_null());

        unsafe {
            let p1 = __nl_arena_alloc(arena_ptr, 64);
            assert!(!p1.is_null());

            let p2 = __nl_arena_alloc(arena_ptr, 128);
            assert!(!p2.is_null());
            assert_ne!(p1, p2);

            assert_eq!((*arena_ptr).total_allocated, 192);

            __nl_arena_reset(arena_ptr);
            assert_eq!((*arena_ptr).total_allocated, 0);

            let p3 = __nl_arena_alloc(arena_ptr, 64);
            assert_eq!(p1, p3, "Resetting arena reuses the exact same initial address");

            __nl_arena_destroy(arena_ptr);
        }
    }

    #[test]
    fn test_default_loop_arena() {
        __nl_loop_reset();
        assert_eq!(__nl_arena_get_allocated_bytes(), 0);

        let p1 = __nl_arena_alloc_default(256);
        assert!(!p1.is_null());
        assert_eq!(__nl_arena_get_allocated_bytes(), 256);

        __nl_loop_reset();
        assert_eq!(__nl_arena_get_allocated_bytes(), 0);

        let p2 = __nl_arena_alloc_default(256);
        assert_eq!(p1, p2, "Default loop arena reuses head chunk on reset");
    }
}
