// Mog runtime — hash map operations.
//
// Layout matches the C ABI that compiled Mog programs expect:
//
//   MapEntry (32 bytes):  key(*u8):0  key_len(u64):8  value(u64):16  next(*Entry):24
//   Map      (24 bytes):  capacity(u64):0  size(u64):8  buckets(**Entry):16

use core::ptr;
use core::slice;

unsafe extern "C" {
    fn gc_alloc_kind(size: usize, kind: i32) -> *mut u8;
    fn gc_alloc(size: usize) -> *mut u8;
    fn gc_add_root(slot: *mut *mut u8);
    fn gc_remove_root(slot: *mut *mut u8);
}

const OBJ_MAP: i32 = 2;
const OBJ_ENTRY: i32 = 4;

// ---------------------------------------------------------------------------
// Struct helpers — typed views over the raw GC-allocated memory.
// ---------------------------------------------------------------------------

#[repr(C)]
struct Map {
    capacity: u64,
    size: u64,
    buckets: *mut *mut MapEntry,
}

#[repr(C)]
struct MapEntry {
    key: *mut u8,
    key_len: u64,
    value: u64,
    next: *mut MapEntry,
}

// ---------------------------------------------------------------------------
// FNV-1a hash
// ---------------------------------------------------------------------------

const FNV_OFFSET: u64 = 14695981039346656037;
const FNV_PRIME: u64 = 1099511628211;

fn fnv1a(key: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET;
    for &b in key {
        hash ^= b as u64;
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find an entry whose key equals `key` in the bucket chain starting at `head`.
unsafe fn find_entry(mut entry: *mut MapEntry, key: &[u8]) -> *mut MapEntry {
    unsafe {
        while !entry.is_null() {
            let e = &*entry;
            if e.key_len == key.len() as u64 {
                let stored = slice::from_raw_parts(e.key, key.len());
                if stored == key {
                    return entry;
                }
            }
            entry = (*entry).next;
        }
        ptr::null_mut()
    }
}

// ---------------------------------------------------------------------------
// Public extern "C" API
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn map_new(capacity: u64) -> *mut u8 {
    unsafe {
        let mut map_ptr = gc_alloc_kind(size_of::<Map>(), OBJ_MAP) as *mut Map;
        if map_ptr.is_null() {
            return ptr::null_mut();
        }
        let map_root: *mut *mut u8 = std::ptr::addr_of_mut!(map_ptr).cast();
        gc_add_root(map_root);
        let buckets_bytes = size_of::<*mut MapEntry>() * capacity as usize;
        let buckets = gc_alloc(buckets_bytes) as *mut *mut MapEntry;
        if buckets.is_null() {
            gc_remove_root(map_root);
            return ptr::null_mut();
        }

        // Zero-initialise the bucket array.
        ptr::write_bytes(buckets, 0, capacity as usize);

        (*map_ptr).capacity = capacity;
        (*map_ptr).size = 0;
        (*map_ptr).buckets = buckets;

        gc_remove_root(map_root);
        map_ptr as *mut u8
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_set(map_ptr: *mut u8, key: *const u8, key_len: u64, value: u64) {
    unsafe {
        let map = &mut *(map_ptr as *mut Map);
        let key_slice = slice::from_raw_parts(key, key_len as usize);
        let idx = (fnv1a(key_slice) % map.capacity) as usize;

        // If key already exists, update in place.
        let existing = find_entry(*map.buckets.add(idx), key_slice);
        if !existing.is_null() {
            (*existing).value = value;
            return;
        }

        // Allocate a new entry and prepend to the bucket chain.
        let mut entry = gc_alloc_kind(size_of::<MapEntry>(), OBJ_ENTRY) as *mut MapEntry;
        if entry.is_null() {
            return;
        }
        let entry_root: *mut *mut u8 = std::ptr::addr_of_mut!(entry).cast();
        gc_add_root(entry_root);

        let mut key_copy = gc_alloc(key_len as usize);
        if key_copy.is_null() {
            gc_remove_root(entry_root);
            return;
        }
        let key_copy_root: *mut *mut u8 = std::ptr::addr_of_mut!(key_copy).cast();
        gc_add_root(key_copy_root);
        ptr::copy_nonoverlapping(key, key_copy, key_len as usize);

        (*entry).key = key_copy;
        (*entry).key_len = key_len;
        (*entry).value = value;
        (*entry).next = *map.buckets.add(idx);

        gc_remove_root(key_copy_root);
        *map.buckets.add(idx) = entry;
        gc_remove_root(entry_root);
        map.size += 1;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_get(map_ptr: *const u8, key: *const u8, key_len: u64) -> u64 {
    unsafe {
        let map = &*(map_ptr as *const Map);
        let key_slice = slice::from_raw_parts(key, key_len as usize);
        let idx = (fnv1a(key_slice) % map.capacity) as usize;

        let entry = find_entry(*map.buckets.add(idx), key_slice);
        if entry.is_null() {
            0
        } else {
            (*entry).value
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_has(map_ptr: *const u8, key: *const u8, key_len: u64) -> i64 {
    unsafe {
        let map = &*(map_ptr as *const Map);
        let key_slice = slice::from_raw_parts(key, key_len as usize);
        let idx = (fnv1a(key_slice) % map.capacity) as usize;

        let entry = find_entry(*map.buckets.add(idx), key_slice);
        if entry.is_null() {
            0
        } else {
            1
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_size(map_ptr: *const u8) -> i64 {
    if map_ptr.is_null() {
        return 0;
    }
    unsafe {
        let map = &*(map_ptr as *const Map);
        map.size as i64
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_key_at(map_ptr: *const u8, index: i64) -> *mut u8 {
    if map_ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe {
        let map = &*(map_ptr as *const Map);
        let target = index as u64;
        let mut seen: u64 = 0;

        for i in 0..map.capacity as usize {
            let mut entry = *map.buckets.add(i);
            while !entry.is_null() {
                if seen == target {
                    // Return a null-terminated GC-allocated copy of the key.
                    let len = (*entry).key_len as usize;
                    let copy = gc_alloc(len + 1);
                    ptr::copy_nonoverlapping((*entry).key, copy, len);
                    *copy.add(len) = 0;
                    return copy;
                }
                seen += 1;
                entry = (*entry).next;
            }
        }

        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_value_at(map_ptr: *const u8, index: i64) -> u64 {
    if map_ptr.is_null() {
        return 0;
    }
    unsafe {
        let map = &*(map_ptr as *const Map);
        let target = index as u64;
        let mut seen: u64 = 0;

        for i in 0..map.capacity as usize {
            let mut entry = *map.buckets.add(i);
            while !entry.is_null() {
                if seen == target {
                    return (*entry).value;
                }
                seen += 1;
                entry = (*entry).next;
            }
        }

        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn map_remove(map_ptr: *mut u8, key: *const u8, key_len: u64) {
    unsafe {
        let map = &mut *(map_ptr as *mut Map);
        let key_slice = slice::from_raw_parts(key, key_len as usize);
        let idx = (fnv1a(key_slice) % map.capacity) as usize;

        let mut prev: *mut *mut MapEntry = map.buckets.add(idx);
        let mut cur = *prev;

        while !cur.is_null() {
            let e = &*cur;
            if e.key_len == key_len {
                let stored = slice::from_raw_parts(e.key, key_len as usize);
                if stored == key_slice {
                    // Unlink entry from the chain.
                    *prev = (*cur).next;
                    map.size -= 1;
                    return;
                }
            }
            prev = &mut (*cur).next;
            cur = (*cur).next;
        }
    }
}
