// Mog runtime — mark-sweep garbage collector.
//
// Page-based heap with small-object malloc and large-object mmap.
// Shadow stack for GC root tracking (push_frame / pop_frame / add_root).
// Non-recursive mark phase using an explicit mark stack.
//
// Block header layout (24 bytes, immediately before user data):
//
//   offset 0:  kind  (u32)   — ObjectKind discriminant
//   offset 4:  mark  (u32)   — 0 = unmarked, 1 = marked
//   offset 8:  size  (u32)   — user-data size in bytes
//   offset 12: _pad  (u32)   — alignment padding
//   offset 16: next  (*Block) — intrusive linked-list pointer
//
// All `extern "C"` functions use the exact names the QBE-generated code calls.

use core::ptr;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Objects at or above this size are allocated via mmap.
const LARGE_OBJECT_THRESHOLD: usize = 4096;

/// Maximum number of freed large mappings to cache for reuse.
const MAX_CACHED_MAPPINGS: usize = 16;

/// Maximum GC root slots per shadow-stack frame.
const MAX_FRAME_SLOTS: usize = 1024;

/// Trigger a collection when usage is this close to the memory cap.
const MEM_LIMIT_SOFT_SLACK: usize = 64 * 1024;

/// Default allocation threshold before any explicit limits are configured.
const DEFAULT_ALLOC_THRESHOLD: usize = 1024 * 1024;

// ---------------------------------------------------------------------------
// ObjectKind — must match the compiler's tag values
// ---------------------------------------------------------------------------

const OBJ_RAW: u32 = 0;
const OBJ_ARRAY: u32 = 1;
const OBJ_MAP: u32 = 2;
const _OBJ_STRING: u32 = 3;
const OBJ_ENTRY: u32 = 4;
const OBJ_CLOSURE: u32 = 5;
const OBJ_TENSOR: u32 = 7;

// ---------------------------------------------------------------------------
// Block header — 24 bytes, #[repr(C)] for deterministic layout
// ---------------------------------------------------------------------------

#[repr(C)]
struct Block {
    kind: u32,
    mark: u32,
    size: u32,
    _pad: u32,
    next: *mut Block,
}

const BLOCK_SIZE: usize = size_of::<Block>(); // must be 24

// ---------------------------------------------------------------------------
// Large-object wrapper — sits on the heap list like a normal Block but also
// tracks the underlying mmap region.
// ---------------------------------------------------------------------------

#[repr(C)]
struct LargeBlock {
    base: Block,
    mapping: *mut u8,
    mapping_size: usize,
}

// ---------------------------------------------------------------------------
// Mark stack — growable array of Block pointers for iterative marking
// ---------------------------------------------------------------------------

struct MarkStack {
    data: *mut *mut Block,
    capacity: usize,
    count: usize,
}

impl MarkStack {
    const fn new() -> Self {
        Self {
            data: ptr::null_mut(),
            capacity: 0,
            count: 0,
        }
    }

    fn init(&mut self) {
        self.capacity = 1024;
        self.data =
            unsafe { libc::malloc(size_of::<*mut Block>() * self.capacity) as *mut *mut Block };
        self.count = 0;
    }

    /// Push a block onto the mark stack, marking it immediately.
    /// No-op if `block` is null or already marked.
    fn push(&mut self, block: *mut Block) {
        if block.is_null() {
            return;
        }
        unsafe {
            if (*block).mark != 0 {
                return;
            }
            if self.count >= self.capacity {
                self.capacity *= 2;
                self.data = libc::realloc(
                    self.data as *mut libc::c_void,
                    size_of::<*mut Block>() * self.capacity,
                ) as *mut *mut Block;
            }
            *self.data.add(self.count) = block;
            self.count += 1;
            (*block).mark = 1;
        }
    }

    fn pop(&mut self) -> *mut Block {
        if self.count == 0 {
            return ptr::null_mut();
        }
        self.count -= 1;
        unsafe { *self.data.add(self.count) }
    }

    fn free_memory(&mut self) {
        if !self.data.is_null() {
            unsafe { libc::free(self.data as *mut libc::c_void) };
            self.data = ptr::null_mut();
        }
        self.capacity = 0;
        self.count = 0;
    }
}

// ---------------------------------------------------------------------------
// Mapping cache — reuse recently-freed mmap regions for large objects
// ---------------------------------------------------------------------------

struct CachedMapping {
    addr: *mut u8,
    size: usize,
    next: *mut CachedMapping,
}

// ---------------------------------------------------------------------------
// Shadow-stack frame
// ---------------------------------------------------------------------------

struct GCFrame {
    prev: *mut GCFrame,
    count: usize,
    slots: [*mut *mut u8; MAX_FRAME_SLOTS],
}

// ---------------------------------------------------------------------------
// Type definitions needed for tracing (must match compiled Mog ABI)
// ---------------------------------------------------------------------------

#[repr(C)]
struct Array {
    element_size: u64,
    dimension_count: u64,
    dimensions: *mut u64,
    strides: *mut u64,
    data: *mut u8,
}

#[repr(C)]
struct MapEntry {
    key: *mut u8,
    key_len: u64,
    value: u64,
    next: *mut MapEntry,
}

#[repr(C)]
struct Map {
    capacity: u64,
    size: u64,
    buckets: *mut *mut MapEntry,
}

#[repr(C)]
struct MogTensor {
    ndim: i64,
    shape: *mut i64,
    strides: *mut i64,
    data: *mut f32,
    size: i64,
    dtype: i8,
}

// ---------------------------------------------------------------------------
// Global GC state (single-threaded runtime — `static mut` is fine)
// ---------------------------------------------------------------------------

static mut HEAP: *mut Block = ptr::null_mut();
static mut HEAP_SIZE: usize = 0;
static mut ALLOC_THRESHOLD: usize = DEFAULT_ALLOC_THRESHOLD;
static mut ALLOC_COUNT: usize = 0;
static mut GC_ALLOCATED_BYTES: usize = 0;
static mut GC_LIVE_BYTES: usize = 0;
static mut GC_TOTAL_COLLECTIONS: usize = 0;
static mut GC_GROWTH_FACTOR: f64 = 2.0;
static mut GC_MIN_THRESHOLD: usize = 64 * 1024;
static mut GC_MEMORY_LIMIT: usize = 0;
static mut GC_INITIAL_MEMORY: usize = 0;
static mut GC_EXTERNAL_BYTES: usize = 0;
static mut GC_OOM: bool = false;

static mut MARK_STACK: MarkStack = MarkStack::new();

static mut MAPPING_CACHE: *mut CachedMapping = ptr::null_mut();
static mut MAPPING_CACHE_COUNT: usize = 0;
static mut PAGE_SIZE: usize = 0;

static mut CURRENT_FRAME: *mut GCFrame = ptr::null_mut();

// Benchmarking counters.
static mut GC_TOTAL_MARK_NS: u64 = 0;
static mut GC_TOTAL_SWEEP_NS: u64 = 0;
static mut GC_TOTAL_FREED: usize = 0;
static mut GC_LAST_FREED: usize = 0;
static mut GC_LAST_MARK_NS: u64 = 0;
static mut GC_LAST_SWEEP_NS: u64 = 0;

// ---------------------------------------------------------------------------
// Timing helper
// ---------------------------------------------------------------------------

fn get_nanos() -> u64 {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    unsafe {
        libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut ts);
    }
    ts.tv_sec as u64 * 1_000_000_000 + ts.tv_nsec as u64
}

fn tracked_heap_bytes() -> usize {
    unsafe { HEAP_SIZE.saturating_add(GC_EXTERNAL_BYTES) }
}

fn total_alloc_bytes(size: usize) -> Option<usize> {
    let header = std::mem::size_of::<usize>();
    size.checked_add(header)
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_external_alloc(size: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }

    let total = match total_alloc_bytes(size) {
        Some(v) => v,
        None => return std::ptr::null_mut(),
    };

    if !gc_reserve_external_memory(total) {
        return std::ptr::null_mut();
    }

    let raw = unsafe { libc::malloc(total) as *mut usize };
    if raw.is_null() {
        gc_release_external_memory(total);
        return std::ptr::null_mut();
    }

    unsafe {
        *raw = size;
    }
    unsafe { raw.add(1) as *mut u8 }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_external_free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }

    let base = unsafe { (ptr as *mut usize).sub(1) };
    let user_size = unsafe { *base };
    if let Some(total) = total_alloc_bytes(user_size) {
        unsafe {
            libc::free(base as *mut libc::c_void);
            gc_release_external_memory(total);
        }
    } else {
        unsafe {
            libc::free(base as *mut libc::c_void);
        }
    }
}

/// Configure the GC memory limit and starting threshold.
///
/// `max_memory` is the hard cap in bytes (0 = unlimited).
/// `initial_memory` is the initial threshold for collection before growing.
pub(crate) fn gc_set_memory_limits(max_memory: usize, initial_memory: usize) {
    let initial_memory = if max_memory > 0 && initial_memory > max_memory {
        max_memory
    } else {
        initial_memory
    };

    unsafe {
        GC_MEMORY_LIMIT = max_memory;
        GC_INITIAL_MEMORY = initial_memory;
        GC_OOM = false;
        GC_EXTERNAL_BYTES = 0;

        let base_threshold = if initial_memory > 0 {
            initial_memory.max(GC_MIN_THRESHOLD)
        } else if max_memory > 0 {
            max_memory
                .min(DEFAULT_ALLOC_THRESHOLD)
                .max(GC_MIN_THRESHOLD)
        } else {
            DEFAULT_ALLOC_THRESHOLD
        };
        ALLOC_THRESHOLD = base_threshold;
    }
}

fn enforce_memory_limit(size: usize) -> bool {
    let limit = unsafe { GC_MEMORY_LIMIT };
    if limit == 0 {
        unsafe {
            GC_OOM = false;
        }
        return true;
    }

    let projected = tracked_heap_bytes().saturating_add(size);
    let soft_limit = limit.saturating_sub(MEM_LIMIT_SOFT_SLACK.min(limit));
    if projected >= soft_limit {
        gc_collect();
        if tracked_heap_bytes().saturating_add(size) <= limit {
            unsafe {
                GC_OOM = false;
            }
            return true;
        }

        unsafe {
            if !GC_OOM {
                eprintln!("Out of memory: allocation ({size} bytes) exceeds configured memory limit {limit}");
            }
            GC_OOM = true;
            crate::vm::mog_request_interrupt();
        }
        return false;
    }

    unsafe {
        GC_OOM = false;
    }
    true
}

pub(crate) fn gc_reserve_external_memory(size: usize) -> bool {
    if size == 0 {
        return true;
    }

    if !enforce_memory_limit(size) {
        return false;
    }

    unsafe {
        GC_EXTERNAL_BYTES = GC_EXTERNAL_BYTES.saturating_add(size);
    }
    true
}

pub(crate) fn gc_release_external_memory(size: usize) {
    if size == 0 {
        return;
    }

    unsafe {
        GC_EXTERNAL_BYTES = GC_EXTERNAL_BYTES.saturating_sub(size);
    }
}

// ---------------------------------------------------------------------------
// Page-level allocation (mmap / munmap)
// ---------------------------------------------------------------------------

fn init_page_size() {
    unsafe {
        if PAGE_SIZE == 0 {
            let ps = libc::sysconf(libc::_SC_PAGESIZE);
            PAGE_SIZE = if ps > 0 { ps as usize } else { 4096 };
        }
    }
}

fn round_up_to_pages(size: usize) -> usize {
    init_page_size();
    let ps = unsafe { PAGE_SIZE };
    (size + ps - 1) & !(ps - 1)
}

fn page_alloc(size: usize) -> *mut u8 {
    let aligned = round_up_to_pages(size);
    unsafe {
        let addr = libc::mmap(
            ptr::null_mut(),
            aligned,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_PRIVATE | libc::MAP_ANON,
            -1,
            0,
        );
        if addr == libc::MAP_FAILED {
            ptr::null_mut()
        } else {
            addr as *mut u8
        }
    }
}

fn page_free(addr: *mut u8, size: usize) {
    if addr.is_null() {
        return;
    }
    let aligned = round_up_to_pages(size);
    unsafe {
        libc::munmap(addr as *mut libc::c_void, aligned);
    }
}

// ---------------------------------------------------------------------------
// Mapping cache
// ---------------------------------------------------------------------------

/// Search the mapping cache for a region >= `min_size`.  Returns null if none
/// found, otherwise removes the best-fit entry and writes its size to
/// `out_size`.
unsafe fn cache_get_mapping(min_size: usize, out_size: &mut usize) -> *mut u8 {
    unsafe {
        let mut current = &raw mut MAPPING_CACHE as *mut *mut CachedMapping;
        let mut best: *mut CachedMapping = ptr::null_mut();
        let mut best_prev: *mut *mut CachedMapping = ptr::null_mut();

        while !(*current).is_null() {
            let m = *current;
            if (*m).size >= min_size && (best.is_null() || (*m).size < (*best).size) {
                best = m;
                best_prev = current;
            }
            current = &raw mut (*(*current)).next;
        }

        if !best.is_null() {
            *best_prev = (*best).next;
            MAPPING_CACHE_COUNT -= 1;
            let addr = (*best).addr;
            *out_size = (*best).size;
            libc::free(best as *mut libc::c_void);
            return addr;
        }

        *out_size = 0;
        ptr::null_mut()
    }
}

unsafe fn cache_put_mapping(addr: *mut u8, size: usize) {
    unsafe {
        if MAPPING_CACHE_COUNT >= MAX_CACHED_MAPPINGS {
            if !MAPPING_CACHE.is_null() {
                let oldest = MAPPING_CACHE;
                MAPPING_CACHE = (*oldest).next;
                page_free((*oldest).addr, (*oldest).size);
                libc::free(oldest as *mut libc::c_void);
                MAPPING_CACHE_COUNT -= 1;
            }
        }

        let entry = libc::malloc(size_of::<CachedMapping>()) as *mut CachedMapping;
        if !entry.is_null() {
            (*entry).addr = addr;
            (*entry).size = size;
            (*entry).next = MAPPING_CACHE;
            MAPPING_CACHE = entry;
            MAPPING_CACHE_COUNT += 1;
        } else {
            page_free(addr, size);
        }
    }
}

unsafe fn cache_clear() {
    unsafe {
        while !MAPPING_CACHE.is_null() {
            let m = MAPPING_CACHE;
            MAPPING_CACHE = (*m).next;
            page_free((*m).addr, (*m).size);
            libc::free(m as *mut libc::c_void);
        }
        MAPPING_CACHE_COUNT = 0;
    }
}

// ---------------------------------------------------------------------------
// Allocation helpers
// ---------------------------------------------------------------------------

fn is_large_block(block: *const Block) -> bool {
    unsafe { (*block).size as usize >= LARGE_OBJECT_THRESHOLD }
}

fn mark_out_of_memory(size: usize) {
    unsafe {
        let limit = GC_MEMORY_LIMIT;
        if !GC_OOM {
            if limit == 0 {
                eprintln!("Out of memory ({size} bytes)");
            } else {
                eprintln!(
                    "Out of memory: allocation ({size} bytes) exceeds configured memory limit {limit}"
                );
            }
            GC_OOM = true;
        }
    }
    crate::vm::mog_request_interrupt();
}

/// Allocate a large object via mmap.  The Block header is malloc'd separately
/// (as a `LargeBlock`) while the user data lives in the mmap region.
unsafe fn large_object_alloc(size: usize, kind: u32) -> *mut u8 {
    unsafe {
        let mut actual_size: usize = 0;
        let mut mapping = cache_get_mapping(size, &mut actual_size);

        if mapping.is_null() {
            mapping = page_alloc(size);
            if mapping.is_null() {
                cache_clear();
                mapping = page_alloc(size);
                if mapping.is_null() {
                    mark_out_of_memory(size);
                    return ptr::null_mut();
                }
            }
            actual_size = round_up_to_pages(size);
        }

        let lb = libc::malloc(size_of::<LargeBlock>()) as *mut LargeBlock;
        if lb.is_null() {
            cache_put_mapping(mapping, actual_size);
            mark_out_of_memory(size);
            return ptr::null_mut();
        }

        (*lb).base.kind = kind;
        (*lb).base.mark = 0;
        (*lb).base.size = size as u32;
        (*lb).base._pad = 0;
        (*lb).base.next = HEAP;
        (*lb).mapping = mapping;
        (*lb).mapping_size = actual_size;

        HEAP = &raw mut (*lb).base;
        HEAP_SIZE += size;

        mapping
    }
}

/// Allocate a small object via malloc.  The Block header is inline, directly
/// before the user data.
unsafe fn small_object_alloc(size: usize, kind: u32) -> *mut u8 {
    unsafe {
        let total = BLOCK_SIZE + size;
        let mut raw = libc::malloc(total) as *mut u8;
        if raw.is_null() {
            gc_collect();
            raw = libc::malloc(total) as *mut u8;
            if raw.is_null() {
                mark_out_of_memory(size);
                return ptr::null_mut();
            }
        }

        let block = raw as *mut Block;
        (*block).kind = kind;
        (*block).mark = 0;
        (*block).size = size as u32;
        (*block)._pad = 0;
        (*block).next = HEAP;
        HEAP = block;
        HEAP_SIZE += size;

        // Zero-initialize user data.
        let user = raw.add(BLOCK_SIZE);
        ptr::write_bytes(user, 0, size);

        user
    }
}

/// Given a user-data pointer, step back to the Block header.
#[inline]
unsafe fn ptr_to_block(ptr: *mut u8) -> *mut Block {
    if ptr.is_null() {
        return ptr::null_mut();
    }
    unsafe { ptr.sub(BLOCK_SIZE) as *mut Block }
}

#[inline]
unsafe fn gc_ptr_to_block(ptr: *mut u8) -> *mut Block {
    if ptr.is_null() {
        return ptr::null_mut();
    }

    let addr = ptr as usize;
    if addr < BLOCK_SIZE {
        return ptr::null_mut();
    }

    let candidate = (addr - BLOCK_SIZE) as *mut Block;
    if is_block_on_heap(candidate) {
        candidate
    } else {
        ptr::null_mut()
    }
}

#[inline]
unsafe fn is_block_on_heap(block: *mut Block) -> bool {
    let mut cursor = HEAP;
    while !cursor.is_null() {
        if cursor == block {
            return true;
        }
        cursor = (*cursor).next;
    }
    false
}

// ---------------------------------------------------------------------------
// Tracing functions — called during the mark phase to discover children
// ---------------------------------------------------------------------------

/// Trace an Array's inner allocations (dimensions, strides, data).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gc_trace_array(arr: *mut u8) {
    if arr.is_null() {
        return;
    }
    unsafe {
        let a = &*(arr as *const Array);

        if !a.dimensions.is_null() {
            let b = ptr_to_block(a.dimensions as *mut u8);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
        if !a.strides.is_null() {
            let b = ptr_to_block(a.strides as *mut u8);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
        if !a.data.is_null() {
            let b = ptr_to_block(a.data);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
    }
}

/// Trace a Map — the bucket array and every entry (including keys).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gc_trace_map(map: *mut u8) {
    if map.is_null() {
        return;
    }
    unsafe {
        let m = &*(map as *const Map);

        if !m.buckets.is_null() {
            let buckets_block = ptr_to_block(m.buckets as *mut u8);
            gc_mark(buckets_block as *mut u8);
        }

        for i in 0..m.capacity as usize {
            let mut entry = *m.buckets.add(i);
            while !entry.is_null() {
                let entry_block = ptr_to_block(entry as *mut u8);
                gc_mark(entry_block as *mut u8);

                if !(*entry).key.is_null() {
                    let key_block = ptr_to_block((*entry).key);
                    gc_mark(key_block as *mut u8);
                }
                entry = (*entry).next;
            }
        }
    }
}

/// Trace a single MapEntry (key string).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gc_trace_map_entry(entry: *mut u8) {
    if entry.is_null() {
        return;
    }
    unsafe {
        let e = &*(entry as *const MapEntry);
        if !e.key.is_null() {
            let b = ptr_to_block(e.key);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
    }
}

/// Trace a closure environment — each 8-byte slot may be a GC pointer.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gc_trace_closure(env: *mut u8, size: usize) {
    if env.is_null() {
        return;
    }
    unsafe {
        let num_slots = size / 8;
        let slots = env as *const i64;
        for i in 0..num_slots {
            let maybe_ptr = *slots.add(i) as *mut u8;
            if !maybe_ptr.is_null() {
                let child = ptr_to_block(maybe_ptr);
                if !child.is_null() && (*child).mark == 0 {
                    (*std::ptr::addr_of_mut!(MARK_STACK)).push(child);
                }
            }
        }
    }
}

/// Trace a tensor's shape, strides, and data buffers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gc_trace_tensor(t: *mut u8) {
    if t.is_null() {
        return;
    }
    unsafe {
        let tensor = &*(t as *const MogTensor);

        if !tensor.shape.is_null() {
            let b = ptr_to_block(tensor.shape as *mut u8);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
        if !tensor.strides.is_null() {
            let b = ptr_to_block(tensor.strides as *mut u8);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
        if !tensor.data.is_null() {
            let b = ptr_to_block(tensor.data as *mut u8);
            (*std::ptr::addr_of_mut!(MARK_STACK)).push(b);
        }
    }
}

// ---------------------------------------------------------------------------
// Mark phase
// ---------------------------------------------------------------------------

/// Mark a block and iteratively trace all reachable children.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn gc_mark(ptr: *mut u8) {
    let block = ptr as *mut Block;
    if block.is_null() {
        return;
    }
    unsafe {
        if (*block).mark != 0 {
            return;
        }

        (*std::ptr::addr_of_mut!(MARK_STACK)).push(block);

        while (*std::ptr::addr_of!(MARK_STACK)).count > 0 {
            let b = (*std::ptr::addr_of_mut!(MARK_STACK)).pop();
            let user = (b as *mut u8).add(BLOCK_SIZE);

            match (*b).kind {
                OBJ_ARRAY => gc_trace_array(user),
                OBJ_MAP => gc_trace_map(user),
                OBJ_ENTRY => gc_trace_map_entry(user),
                OBJ_CLOSURE => gc_trace_closure(user, (*b).size as usize),
                OBJ_TENSOR => gc_trace_tensor(user),
                _ => { /* OBJ_RAW, OBJ_STRING — no internal pointers */ }
            }
        }
    }
}

/// Walk the shadow stack and mark every root.
unsafe fn gc_mark_roots() {
    unsafe {
        let mut frame = CURRENT_FRAME;
        while !frame.is_null() {
            for i in 0..(*frame).count {
                let slot = (*frame).slots[i];
                let user_ptr = *slot;
                if !user_ptr.is_null() {
                    let block = gc_ptr_to_block(user_ptr);
                    if !block.is_null() {
                        gc_mark(block as *mut u8);
                    }
                }
            }
            frame = (*frame).prev;
        }
    }
}

// ---------------------------------------------------------------------------
// Sweep phase
// ---------------------------------------------------------------------------

unsafe fn gc_sweep() {
    unsafe {
        GC_LIVE_BYTES = 0;
        let mut cursor = &raw mut HEAP as *mut *mut Block;

        while !(*cursor).is_null() {
            let block = *cursor;
            if (*block).mark == 0 {
                // Unreachable — free it.
                *cursor = (*block).next;
                let sz = (*block).size as usize;
                HEAP_SIZE = HEAP_SIZE.saturating_sub(sz);

                if is_large_block(block) {
                    let lb = block as *mut LargeBlock;
                    cache_put_mapping((*lb).mapping, (*lb).mapping_size);
                }
                libc::free(block as *mut libc::c_void);
            } else {
                // Reachable — clear mark for next cycle.
                GC_LIVE_BYTES += (*block).size as usize;
                (*block).mark = 0;
                cursor = &raw mut (*block).next;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Public extern "C" API
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn gc_init() {
    unsafe {
        let max_memory = GC_MEMORY_LIMIT;
        let initial_memory = GC_INITIAL_MEMORY;
        HEAP = ptr::null_mut();
        HEAP_SIZE = 0;
        ALLOC_COUNT = 0;
        ALLOC_THRESHOLD = if initial_memory > 0 {
            initial_memory.max(GC_MIN_THRESHOLD)
        } else if max_memory > 0 {
            max_memory
                .min(DEFAULT_ALLOC_THRESHOLD)
                .max(GC_MIN_THRESHOLD)
        } else {
            DEFAULT_ALLOC_THRESHOLD
        };
        GC_ALLOCATED_BYTES = 0;
        GC_LIVE_BYTES = 0;
        GC_TOTAL_COLLECTIONS = 0;
        GC_GROWTH_FACTOR = 2.0;
        GC_MIN_THRESHOLD = 64 * 1024;
        PAGE_SIZE = 0;
        MAPPING_CACHE = ptr::null_mut();
        MAPPING_CACHE_COUNT = 0;
        CURRENT_FRAME = ptr::null_mut();

        GC_TOTAL_MARK_NS = 0;
        GC_TOTAL_SWEEP_NS = 0;
        GC_TOTAL_FREED = 0;
        GC_LAST_FREED = 0;
        GC_LAST_MARK_NS = 0;
        GC_LAST_SWEEP_NS = 0;
        GC_OOM = false;
        GC_EXTERNAL_BYTES = 0;

        (*std::ptr::addr_of_mut!(MARK_STACK)).init();
    }

    // Initialize stack guard page protection (idempotent — safe to call multiple times)
    crate::stack_guard::mog_stack_guard_init();
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_alloc(size: usize) -> *mut u8 {
    gc_alloc_kind(size, OBJ_RAW as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_alloc_closure(num_slots: usize) -> *mut u8 {
    gc_alloc_kind(num_slots * 8, OBJ_CLOSURE as i32)
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_alloc_kind(size: usize, kind: i32) -> *mut u8 {
    if !enforce_memory_limit(size) {
        return ptr::null_mut();
    }

    let kind_u32 = kind as u32;
    let result = unsafe {
        if size >= LARGE_OBJECT_THRESHOLD {
            large_object_alloc(size, kind_u32)
        } else {
            small_object_alloc(size, kind_u32)
        }
    };

    unsafe {
        ALLOC_COUNT += size;
        GC_ALLOCATED_BYTES += size;

        if ALLOC_COUNT >= ALLOC_THRESHOLD {
            gc_collect();
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Shadow stack
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn gc_push_frame() {
    unsafe {
        let frame = libc::malloc(size_of::<GCFrame>()) as *mut GCFrame;
        if frame.is_null() {
            eprintln!("Out of memory (GC frame)");
            crate::vm::mog_request_interrupt();
            return;
        }
        (*frame).prev = CURRENT_FRAME;
        (*frame).count = 0;
        // Zero the slots array.
        ptr::write_bytes((*frame).slots.as_mut_ptr(), 0, MAX_FRAME_SLOTS);
        CURRENT_FRAME = frame;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_pop_frame() {
    unsafe {
        if CURRENT_FRAME.is_null() {
            return;
        }
        let frame = CURRENT_FRAME;
        CURRENT_FRAME = (*frame).prev;
        libc::free(frame as *mut libc::c_void);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_add_root(slot: *mut *mut u8) {
    unsafe {
        if CURRENT_FRAME.is_null() || slot.is_null() {
            return;
        }
        let frame = &mut *CURRENT_FRAME;
        if frame.count < MAX_FRAME_SLOTS {
            frame.slots[frame.count] = slot;
            frame.count += 1;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_remove_root(slot: *mut *mut u8) {
    unsafe {
        if CURRENT_FRAME.is_null() || slot.is_null() {
            return;
        }
        let frame = &mut *CURRENT_FRAME;
        // Scan backwards — most recent addition is the likely removal target.
        let mut i = frame.count;
        while i > 0 {
            i -= 1;
            if frame.slots[i] == slot {
                // Swap with the last element and shrink.
                frame.count -= 1;
                frame.slots[i] = frame.slots[frame.count];
                return;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Collection
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn gc_collect() {
    unsafe {
        let start = get_nanos();
        gc_mark_roots();
        GC_LAST_MARK_NS = get_nanos() - start;
        GC_TOTAL_MARK_NS += GC_LAST_MARK_NS;

        let allocated_since_gc = GC_ALLOCATED_BYTES;
        let sweep_start = get_nanos();
        gc_sweep();
        GC_LAST_SWEEP_NS = get_nanos() - sweep_start;
        GC_TOTAL_SWEEP_NS += GC_LAST_SWEEP_NS;

        GC_LAST_FREED = allocated_since_gc.saturating_sub(GC_LIVE_BYTES);
        GC_TOTAL_FREED += GC_LAST_FREED;

        GC_TOTAL_COLLECTIONS += 1;

        let new_threshold = (GC_LIVE_BYTES as f64 * GC_GROWTH_FACTOR) as usize;
        ALLOC_THRESHOLD = new_threshold.max(GC_MIN_THRESHOLD);

        GC_ALLOCATED_BYTES = 0;
        ALLOC_COUNT = 0;
    }
}

// ---------------------------------------------------------------------------
// Shutdown — free everything
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn gc_shutdown() {
    unsafe {
        // Free all blocks.
        let mut block = HEAP;
        while !block.is_null() {
            let next = (*block).next;
            if is_large_block(block) {
                let lb = block as *mut LargeBlock;
                page_free((*lb).mapping, (*lb).mapping_size);
            }
            libc::free(block as *mut libc::c_void);
            block = next;
        }
        HEAP = ptr::null_mut();
        HEAP_SIZE = 0;

        // Free all shadow-stack frames.
        while !CURRENT_FRAME.is_null() {
            let frame = CURRENT_FRAME;
            CURRENT_FRAME = (*frame).prev;
            libc::free(frame as *mut libc::c_void);
        }

        // Free mapping cache.
        cache_clear();

        // Free mark stack.
        (*std::ptr::addr_of_mut!(MARK_STACK)).free_memory();
    }
}

// ---------------------------------------------------------------------------
// Stats / diagnostics
// ---------------------------------------------------------------------------

#[unsafe(no_mangle)]
pub extern "C" fn gc_stats() {
    unsafe {
        eprintln!(
            "[GC] collections={} allocated={} live={} threshold={}",
            std::ptr::addr_of!(GC_TOTAL_COLLECTIONS).read(),
            std::ptr::addr_of!(GC_ALLOCATED_BYTES).read(),
            std::ptr::addr_of!(GC_LIVE_BYTES).read(),
            std::ptr::addr_of!(ALLOC_THRESHOLD).read(),
        );
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_heap_size() -> usize {
    unsafe { HEAP_SIZE }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_set_threshold(t: usize) {
    unsafe {
        ALLOC_THRESHOLD = t;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn gc_get_threshold() -> usize {
    unsafe { ALLOC_THRESHOLD }
}

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    static TEST_GC_MUTEX: Mutex<()> = Mutex::new(());

    fn lock_gc_runtime() -> MutexGuard<'static, ()> {
        TEST_GC_MUTEX.lock().unwrap()
    }

    fn configure_limits(max_memory: usize, initial_memory: usize) {
        gc_set_memory_limits(max_memory, initial_memory);
        gc_init();
        crate::vm::mog_clear_interrupt();
    }

    fn tracked_bytes() -> usize {
        unsafe { HEAP_SIZE.saturating_add(GC_EXTERNAL_BYTES) }
    }

    #[test]
    fn gc_collects_unreachable_memory_before_hitting_limit() {
        let _lock = lock_gc_runtime();
        configure_limits(200 * 1024, 180 * 1024);

        let first = gc_alloc(140_000);
        assert!(!first.is_null(), "first allocation should succeed");

        let second = gc_alloc(60_000);
        assert!(
            !second.is_null(),
            "second allocation should succeed after GC reclaimed unreachable bytes"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            0,
            "allocation should be throttled, not an OOM interrupt"
        );
        assert_eq!(
            gc_heap_size(),
            60_000,
            "first unreachable allocation should be reclaimed before second allocation"
        );

        gc_shutdown();
    }

    #[test]
    fn gc_storm_stays_under_limit_when_everything_is_garbage() {
        let _lock = lock_gc_runtime();
        configure_limits(512 * 1024, 256 * 1024);

        for _ in 0..128 {
            let scratch = gc_alloc(4 * 1024);
            assert!(
                !scratch.is_null(),
                "storm allocation should succeed while below hard limit"
            );
            gc_collect();
            assert_eq!(
                gc_heap_size(),
                0,
                "garbage-only allocations should be reclaimed after collection"
            );
            assert_eq!(crate::vm::mog_interrupt_requested(), 0);
        }

        assert!(tracked_bytes() <= 512 * 1024);
        gc_shutdown();
    }

    #[test]
    fn gc_interrupt_is_limited_to_max_and_host_can_reclaim_by_scoping_roots() {
        let _lock = lock_gc_runtime();
        configure_limits(64 * 1024, 64 * 1024);
        gc_push_frame();

        let mut roots: [*mut u8; 32] = [std::ptr::null_mut(); 32];
        for i in 0..32 {
            let p = gc_alloc(2 * 1024);
            assert!(!p.is_null(), "rooted scratch allocation should succeed");
            roots[i] = p;
            gc_add_root(&mut roots[i]);
        }

        let blocked = gc_alloc(40 * 1024);
        assert!(
            blocked.is_null(),
            "allocation should fail when live rooted memory is at hard limit"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            1,
            "OOM should raise the interrupt flag instead of aborting the process"
        );
        assert!(tracked_bytes() <= 64 * 1024);

        gc_pop_frame();
        crate::vm::mog_clear_interrupt();
        gc_collect();
        assert_eq!(
            gc_heap_size(),
            0,
            "scoping rooted allocations should release tracked heap bytes"
        );

        let recovered = gc_alloc(32 * 1024);
        assert!(
            !recovered.is_null(),
            "host recovery path should allow further allocations after reclaim"
        );
        assert_eq!(crate::vm::mog_interrupt_requested(), 0);
        gc_shutdown();
    }

    #[test]
    fn gc_external_alloc_tracks_and_releases_memory_with_host_visible_budget() {
        let _lock = lock_gc_runtime();
        configure_limits(200 * 1024, 180 * 1024);

        let external_a = gc_external_alloc(120 * 1024);
        let external_b = gc_external_alloc(64 * 1024);
        assert!(
            !external_a.is_null(),
            "first external allocation should succeed"
        );
        assert!(
            !external_b.is_null(),
            "second external allocation should succeed"
        );

        let blocked = gc_external_alloc(32 * 1024);
        assert!(
            blocked.is_null(),
            "external overrun should fail at hard limit"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            1,
            "hard limit should convert OOM into an interrupt"
        );
        assert!(tracked_bytes() <= 200 * 1024);

        gc_external_free(external_a);
        gc_external_free(external_b);
        crate::vm::mog_clear_interrupt();
        let recovered = gc_external_alloc(150 * 1024);
        assert!(
            !recovered.is_null(),
            "reclaiming external buffers should restore budget for subsequent allocations"
        );

        gc_external_free(recovered);
        gc_shutdown();
    }

    #[test]
    fn gc_internal_and_external_limits_share_the_same_budget() {
        let _lock = lock_gc_runtime();
        configure_limits(192 * 1024, 120 * 1024);

        let external = gc_external_alloc(96 * 1024);
        assert!(
            !external.is_null(),
            "external allocation should occupy shared budget before internal allocation"
        );
        gc_push_frame();

        let mut keeps: [*mut u8; 12] = [std::ptr::null_mut(); 12];
        for i in 0..12 {
            let ptr = gc_alloc(2 * 1024);
            assert!(
                !ptr.is_null(),
                "rooted internal allocation should allocate while near limit"
            );
            keeps[i] = ptr;
            gc_add_root(&mut keeps[i]);
        }

        let blocked = gc_alloc(80 * 1024);
        assert!(
            blocked.is_null(),
            "internal allocation should fail once shared budget is full"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            1,
            "hard limit should convert OOM into an interrupt"
        );
        assert!(tracked_bytes() <= 192 * 1024);

        gc_pop_frame();
        gc_external_free(external);
        crate::vm::mog_clear_interrupt();
        gc_collect();
        let recovered = gc_alloc(32 * 1024);
        assert!(
            !recovered.is_null(),
            "heap should recover after shared budget is freed"
        );
        gc_shutdown();
    }

    #[test]
    fn gc_zero_limit_remains_unlimited() {
        let _lock = lock_gc_runtime();
        configure_limits(0, 0);
        let chunk = gc_alloc(768 * 1024);
        assert!(
            !chunk.is_null(),
            "zero max memory should disable hard memory limits"
        );
        assert_eq!(crate::vm::mog_interrupt_requested(), 0);
        gc_shutdown();
    }

    #[test]
    fn gc_reports_interrupt_when_reclaim_fails_and_limit_is_exceeded() {
        let _lock = lock_gc_runtime();
        configure_limits(200 * 1024, 180 * 1024);

        gc_push_frame();
        let mut live = gc_alloc(1024);
        assert!(!live.is_null(), "rooted allocation should succeed");
        gc_add_root(&mut live);
        for _ in 0..64 {
            assert!(
                !gc_alloc(1024).is_null(),
                "scratch allocations should remain available"
            );
        }

        let blocked = gc_alloc(200 * 1024);
        assert!(
            blocked.is_null(),
            "allocation should fail when GC cannot reclaim enough memory"
        );
        assert_eq!(
            gc_heap_size(),
            1024,
            "rooted allocation should remain after reclaim"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            1,
            "OOM should raise the interrupt flag instead of aborting the process"
        );

        gc_pop_frame();
        gc_shutdown();
    }

    #[test]
    fn gc_external_allocates_share_the_same_hard_limit() {
        let _lock = lock_gc_runtime();
        configure_limits(200 * 1024, 180 * 1024);

        let first = gc_external_alloc(120 * 1024);
        assert!(!first.is_null(), "first external allocation should succeed");

        let second = gc_external_alloc(64 * 1024);
        assert!(
            !second.is_null(),
            "second external allocation should succeed after GC check"
        );

        let blocked = gc_external_alloc(32 * 1024);
        assert!(
            blocked.is_null(),
            "allocation beyond configured limit should fail"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            1,
            "hard limit should convert OOM into an interrupt"
        );

        gc_shutdown();
    }

    #[test]
    fn gc_initial_memory_is_clamped_to_max_when_too_large() {
        let _lock = lock_gc_runtime();
        configure_limits(100 * 1024, 10 * 1024 * 1024);
        let clamped_initial =
            unsafe { std::ptr::read_volatile(std::ptr::addr_of!(GC_INITIAL_MEMORY)) };
        assert_eq!(
            clamped_initial,
            100 * 1024,
            "initial memory should be clamped when greater than max"
        );
        gc_shutdown();
    }

    #[test]
    fn gc_limits_prevent_huge_single_request_without_overwriting_heap_accounting() {
        let _lock = lock_gc_runtime();
        configure_limits(128 * 1024, 64 * 1024);

        let blocked = gc_alloc(usize::MAX);
        assert!(
            blocked.is_null(),
            "oversized allocation should fail cleanly"
        );
        assert_eq!(
            tracked_bytes(),
            0,
            "rejecting oversized internal allocation should not change arena accounting"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            1,
            "hard limit enforcement should surface as an interrupt"
        );

        let blocked_external = gc_external_alloc(usize::MAX);
        assert!(
            blocked_external.is_null(),
            "oversized host-visible allocation should fail cleanly"
        );
        assert_eq!(tracked_bytes(), 0);
        assert_eq!(crate::vm::mog_interrupt_requested(), 1);

        gc_shutdown();
    }

    #[test]
    fn gc_collects_on_soft_limit_pressure_from_live_allocations() {
        let _lock = lock_gc_runtime();
        configure_limits(512 * 1024, 256 * 1024);

        let first = gc_alloc(260_000);
        assert!(!first.is_null(), "initial allocation should succeed");

        let second = gc_alloc(190_000);
        assert!(
            !second.is_null(),
            "allocation just past soft threshold should still succeed after forced collection"
        );
        assert_eq!(
            crate::vm::mog_interrupt_requested(),
            0,
            "soft-threshold collection should avoid OOM"
        );
        assert!(
            gc_heap_size() <= 190_000,
            "allocation under soft-limit pressure should have reclaimed unreachable memory"
        );

        gc_shutdown();
    }
}
