#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <time.h>

/* Platform-specific includes for page allocation */
#ifdef _WIN32
#include <windows.h>
#else
#include <sys/mman.h>
#include <unistd.h>
#endif

/* --- Timing utilities --- */
static uint64_t get_nanos(void) {
  struct timespec ts;
  clock_gettime(CLOCK_MONOTONIC, &ts);
  return (uint64_t)ts.tv_sec * 1000000000ULL + ts.tv_nsec;
}

/* --- GC Benchmarking --- */
static uint64_t gc_total_mark_time = 0;
static uint64_t gc_total_sweep_time = 0;
static size_t gc_total_freed = 0;
static size_t gc_last_freed = 0;
static uint64_t gc_last_mark_time = 0;
static uint64_t gc_last_sweep_time = 0;

/* --- Object Kind Enum --- */
typedef enum {
  OBJ_RAW,      /* Raw memory block (no internal pointers) */
  OBJ_ARRAY,    /* Array with dimensions/strides/data */
  OBJ_MAP,      /* Hash map with buckets/entries */
  OBJ_STRING,   /* String buffer */
  OBJ_ENTRY,    /* Map entry with key/value */
  OBJ_CLOSURE   /* Closure environment — slots that may be GC pointers */
} ObjectKind;

/* --- GC Block Header --- */
typedef struct Block {
  size_t size;
  uint8_t marked;
  ObjectKind kind;
  struct Block* next;
} Block;

/* --- Mark Stack for Non-Recursive Marking --- */
typedef struct {
  Block** data;
  size_t capacity;
  size_t count;
} MarkStack;

static MarkStack mark_stack = {NULL, 0, 0};

static void mark_stack_init(void) {
  mark_stack.capacity = 1024;
  mark_stack.data = (Block**)malloc(sizeof(Block*) * mark_stack.capacity);
  mark_stack.count = 0;
}

static void mark_stack_push(Block* block) {
  if (!block || block->marked) return;
  
  if (mark_stack.count >= mark_stack.capacity) {
    mark_stack.capacity *= 2;
    mark_stack.data = (Block**)realloc(mark_stack.data, sizeof(Block*) * mark_stack.capacity);
  }
  
  mark_stack.data[mark_stack.count++] = block;
  block->marked = 1;
}

static Block* mark_stack_pop(void) {
  if (mark_stack.count == 0) return NULL;
  return mark_stack.data[--mark_stack.count];
}

/* --- Large Object Allocator Constants --- */
#define LARGE_OBJECT_THRESHOLD 4096  /* 1 page threshold for large objects */
#define MAX_CACHED_MAPPINGS 16       /* Maximum number of freed mappings to cache */

/* --- Large Object Tracking --- */
typedef struct CachedMapping {
  void* addr;
  size_t size;
  struct CachedMapping* next;
} CachedMapping;

typedef struct LargeBlock {
  Block base;
  void* mapping;
  size_t mapping_size;
} LargeBlock;

/* --- Standard GC Globals --- */
static Block* heap = NULL;
static size_t heap_size = 0;
static size_t alloc_threshold = 1024 * 1024;
static size_t alloc_count = 0;
static size_t gc_allocated_bytes = 0;
static size_t gc_live_bytes = 0;
static size_t gc_total_collections = 0;
static double gc_growth_factor = 2.0;
static size_t gc_min_threshold = 64 * 1024;

/* Large object cache */
static CachedMapping* mapping_cache = NULL;
static size_t mapping_cache_count = 0;
static size_t page_size = 0;

/* --- Shadow Stack for Root Tracking --- */
#define MAX_FRAME_SLOTS 256

typedef struct GCFrame {
  struct GCFrame* prev;
  int count;
  void** slots[MAX_FRAME_SLOTS];
} GCFrame;

static GCFrame* current_frame = NULL;

/* Forward declarations */
void gc_init(void);
void* gc_alloc(size_t size);
void* gc_alloc_kind(size_t size, ObjectKind kind);
void gc_collect(void);
void gc_mark(Block* ptr);
void gc_mark_roots(void);
void gc_sweep(void);
void gc_stats(void);
void gc_benchmark_stats(void);
void gc_reset_stats(void);
void gc_push_frame(void);
void gc_pop_frame(void);
void gc_add_root(void** slot);

/* --- Platform-Specific Page Allocation --- */

static void init_page_size(void) {
  if (page_size == 0) {
#ifdef _WIN32
    SYSTEM_INFO si;
    GetSystemInfo(&si);
    page_size = si.dwPageSize;
#else
    page_size = (size_t)sysconf(_SC_PAGESIZE);
    if (page_size == 0) page_size = 4096;
#endif
  }
}

static size_t round_up_to_pages(size_t size) {
  init_page_size();
  return (size + page_size - 1) & ~(page_size - 1);
}

static void* page_alloc(size_t size) {
  size_t aligned_size = round_up_to_pages(size);
  
#ifdef _WIN32
  void* addr = VirtualAlloc(NULL, aligned_size, MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
  if (addr == NULL) return NULL;
#else
  void* addr = mmap(NULL, aligned_size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  if (addr == MAP_FAILED) return NULL;
#endif
  
  return addr;
}

static void page_free(void* addr, size_t size) {
  if (addr == NULL) return;
  
#ifdef _WIN32
  VirtualFree(addr, 0, MEM_RELEASE);
#else
  size_t aligned_size = round_up_to_pages(size);
  munmap(addr, aligned_size);
#endif
}

/* --- Large Object Cache Management --- */

static void* cache_get_mapping(size_t min_size, size_t* out_actual_size) {
  CachedMapping** current = &mapping_cache;
  CachedMapping* best_fit = NULL;
  CachedMapping** best_fit_prev = NULL;
  
  while (*current) {
    CachedMapping* mapping = *current;
    if (mapping->size >= min_size) {
      if (!best_fit || mapping->size < best_fit->size) {
        best_fit = mapping;
        best_fit_prev = current;
      }
    }
    current = &mapping->next;
  }
  
  if (best_fit) {
    *best_fit_prev = best_fit->next;
    mapping_cache_count--;
    void* addr = best_fit->addr;
    *out_actual_size = best_fit->size;
    free(best_fit);
    return addr;
  }
  
  *out_actual_size = 0;
  return NULL;
}

static void cache_put_mapping(void* addr, size_t size) {
  if (mapping_cache_count >= MAX_CACHED_MAPPINGS) {
    if (mapping_cache) {
      CachedMapping* oldest = mapping_cache;
      mapping_cache = oldest->next;
      page_free(oldest->addr, oldest->size);
      free(oldest);
      mapping_cache_count--;
    }
  }
  
  CachedMapping* entry = (CachedMapping*)malloc(sizeof(CachedMapping));
  if (entry) {
    entry->addr = addr;
    entry->size = size;
    entry->next = mapping_cache;
    mapping_cache = entry;
    mapping_cache_count++;
  } else {
    page_free(addr, size);
  }
}

static void cache_clear(void) {
  while (mapping_cache) {
    CachedMapping* mapping = mapping_cache;
    mapping_cache = mapping->next;
    page_free(mapping->addr, mapping->size);
    free(mapping);
  }
  mapping_cache_count = 0;
}

/* --- Large Object Allocation --- */

static int is_large_block(Block* block) {
  return block->size >= LARGE_OBJECT_THRESHOLD;
}

static void* large_object_alloc(size_t size, ObjectKind kind) {
  size_t actual_mapping_size = 0;
  void* mapping = cache_get_mapping(size, &actual_mapping_size);
  
  if (mapping == NULL) {
    mapping = page_alloc(size);
    if (mapping == NULL) {
      cache_clear();
      mapping = page_alloc(size);
      if (mapping == NULL) {
        fprintf(stderr, "Out of memory (large object allocation)\n");
        exit(1);
      }
    }
    actual_mapping_size = round_up_to_pages(size);
  }
  
  LargeBlock* block = (LargeBlock*)malloc(sizeof(LargeBlock));
  if (!block) {
    cache_put_mapping(mapping, actual_mapping_size);
    fprintf(stderr, "Out of memory (large object header)\n");
    exit(1);
  }
  
  block->base.size = size;
  block->base.marked = 0;
  block->base.kind = kind;
  block->base.next = heap;
  block->mapping = mapping;
  block->mapping_size = actual_mapping_size;
  
  heap = (Block*)block;
  heap_size += size;
  
  return mapping;
}

/* --- Small Object Allocation --- */

static void* small_object_alloc(size_t size, ObjectKind kind) {
  void* ptr = malloc(sizeof(Block) + size);
  if (!ptr) {
    gc_collect();
    ptr = malloc(sizeof(Block) + size);
    if (!ptr) {
      fprintf(stderr, "Out of memory\n");
      exit(1);
    }
  }
  
  Block* block = (Block*)ptr;
  block->size = size;
  block->marked = 0;
  block->kind = kind;
  block->next = heap;
  heap = block;
  heap_size += size;
  
  return (void*)((uintptr_t)ptr + sizeof(Block));
}

/* --- GC Functions --- */

void gc_init(void) {
  heap = NULL;
  heap_size = 0;
  alloc_count = 0;
  gc_allocated_bytes = 0;
  gc_live_bytes = 0;
  gc_total_collections = 0;
  page_size = 0;
  mapping_cache = NULL;
  mapping_cache_count = 0;
  current_frame = NULL;
  mark_stack_init();
}

void* gc_alloc(size_t size) {
  return gc_alloc_kind(size, OBJ_RAW);
}

void* gc_alloc_closure(size_t num_slots) {
  return gc_alloc_kind(num_slots * 8, OBJ_CLOSURE);
}

void* gc_alloc_kind(size_t size, ObjectKind kind) {
  void* result;
  
  if (size >= LARGE_OBJECT_THRESHOLD) {
    result = large_object_alloc(size, kind);
  } else {
    result = small_object_alloc(size, kind);
  }
  
  alloc_count += size;
  gc_allocated_bytes += size;
  
  if (alloc_count >= alloc_threshold) {
    gc_collect();
  }
  
  return result;
}

/* Get Block header from user pointer */
static Block* ptr_to_block(void* ptr) {
  if (!ptr) return NULL;
  return (Block*)((uintptr_t)ptr - sizeof(Block));
}

/* --- Type Definitions (needed for tracing) --- */

typedef struct Array {
  uint64_t element_size;
  uint64_t dimension_count;
  uint64_t* dimensions;
  uint64_t* strides;
  void* data;
} Array;

typedef struct MapEntry {
  char* key;
  uint64_t key_len;
  uint64_t value;
  struct MapEntry* next;
} MapEntry;

typedef struct Map {
  MapEntry** buckets;
  uint64_t capacity;
  uint64_t count;
} Map;

/* --- Tracing Functions --- */

void gc_trace_array(Array* arr);
void gc_trace_map(Map* map);
void gc_trace_map_entry(MapEntry* entry);
void gc_trace_closure(void* env, size_t size);

void gc_mark(Block* ptr) {
  if (!ptr || ptr->marked) return;
  
  mark_stack_push(ptr);
  
  /* Process mark stack iteratively */
  while (mark_stack.count > 0) {
    Block* block = mark_stack_pop();
    
    /* Trace based on object kind */
    switch (block->kind) {
      case OBJ_ARRAY:
        gc_trace_array((Array*)((uintptr_t)block + sizeof(Block)));
        break;
      case OBJ_MAP:
        gc_trace_map((Map*)((uintptr_t)block + sizeof(Block)));
        break;
      case OBJ_ENTRY:
        gc_trace_map_entry((MapEntry*)((uintptr_t)block + sizeof(Block)));
        break;
      case OBJ_CLOSURE:
        gc_trace_closure((void*)((uintptr_t)block + sizeof(Block)), block->size);
        break;
      case OBJ_RAW:
      case OBJ_STRING:
      default:
        /* No internal pointers to trace */
        break;
    }
  }
}

void gc_trace_array(Array* arr) {
  if (!arr) return;
  
  /* Mark dimensions array */
  if (arr->dimensions) {
    Block* dim_block = ptr_to_block(arr->dimensions);
    if (dim_block) mark_stack_push(dim_block);
  }
  
  /* Mark strides array */
  if (arr->strides) {
    Block* stride_block = ptr_to_block(arr->strides);
    if (stride_block) mark_stack_push(stride_block);
  }
  
  /* Mark data - for now treat as raw, could be reference array in future */
  if (arr->data) {
    Block* data_block = ptr_to_block(arr->data);
    if (data_block) mark_stack_push(data_block);
  }
}

void gc_trace_map(Map* map) {
  if (!map) return;

  /* Trace the buckets array */
  if (map->buckets) {
    Block* buckets_block = ptr_to_block(map->buckets);
    gc_mark(buckets_block);
  }

  /* Trace all entries */
  for (uint64_t i = 0; i < map->capacity; i++) {
    MapEntry* entry = map->buckets[i];
    while (entry) {
      /* Trace the entry struct itself (already marked via map, but ensure traced) */
      Block* entry_block = ptr_to_block(entry);
      gc_mark(entry_block);
      /* Trace the key string */
      if (entry->key) {
        Block* key_block = ptr_to_block(entry->key);
        gc_mark(key_block);
      }
      entry = entry->next;
    }
  }
}

void gc_trace_map_entry(MapEntry* entry) {
  if (!entry) return;
  
  /* Mark the key string */
  if (entry->key) {
    Block* key_block = ptr_to_block(entry->key);
    if (key_block) mark_stack_push(key_block);
  }
  
  /* Note: entry->value could be a pointer to another heap object.
     For now we assume values are primitives (i64).
     If values can be references, we'd need to trace them here. */
}

void gc_trace_closure(void* env, size_t size) {
  if (!env) return;
  
  /* Closure environments contain slots that may be pointers to GC objects.
     Each slot is 8 bytes. Scan all slots conservatively. */
  size_t num_slots = size / 8;
  int64_t* slots = (int64_t*)env;
  for (size_t i = 0; i < num_slots; i++) {
    void* maybe_ptr = (void*)slots[i];
    if (maybe_ptr) {
      Block* child = ptr_to_block(maybe_ptr);
      if (child && !child->marked) {
        mark_stack_push(child);
      }
    }
  }
}

/* --- Shadow Stack Implementation --- */

void gc_push_frame(void) {
  GCFrame* frame = (GCFrame*)malloc(sizeof(GCFrame));
  if (!frame) {
    fprintf(stderr, "Out of memory (GC frame)\n");
    exit(1);
  }
  
  frame->prev = current_frame;
  frame->count = 0;
  current_frame = frame;
}

void gc_pop_frame(void) {
  if (!current_frame) return;
  
  GCFrame* frame = current_frame;
  current_frame = frame->prev;
  free(frame);
}

void gc_add_root(void** slot) {
  if (!current_frame || !slot) return;
  
  if (current_frame->count < MAX_FRAME_SLOTS) {
    current_frame->slots[current_frame->count++] = slot;
  }
}

/* Mark all roots from shadow stack */
void gc_mark_roots(void) {
  GCFrame* frame = current_frame;
  
  while (frame) {
    for (int i = 0; i < frame->count; i++) {
      void* ptr = *frame->slots[i];
      if (ptr) {
        Block* block = ptr_to_block(ptr);
        if (block) gc_mark(block);
      }
    }
    frame = frame->prev;
  }
}

void gc_sweep(void) {
  gc_live_bytes = 0;
  Block** ptr = &heap;
  while (*ptr) {
    if (!(*ptr)->marked) {
      Block* to_free = *ptr;
      *ptr = to_free->next;
      heap_size -= to_free->size;
      
      if (is_large_block(to_free)) {
        LargeBlock* large = (LargeBlock*)to_free;
        cache_put_mapping(large->mapping, large->mapping_size);
      }
      
      free(to_free);
    } else {
      gc_live_bytes += (*ptr)->size;
      (*ptr)->marked = 0;
      ptr = &(*ptr)->next;
    }
  }
}

void gc_collect(void) {
  uint64_t start = get_nanos();
  gc_mark_roots();
  gc_last_mark_time = get_nanos() - start;
  gc_total_mark_time += gc_last_mark_time;
  
  size_t pre_sweep_live = gc_live_bytes;
  size_t allocated_since_gc = gc_allocated_bytes;
  uint64_t sweep_start = get_nanos();
  gc_sweep();
  gc_last_sweep_time = get_nanos() - sweep_start;
  gc_total_sweep_time += gc_last_sweep_time;
  
  /* Freed = allocated since last GC - (new_live - old_live)
     = bytes allocated that didn't survive marking */
  size_t newly_live = gc_live_bytes > pre_sweep_live ? gc_live_bytes - pre_sweep_live : 0;
  gc_last_freed = allocated_since_gc > newly_live ? allocated_since_gc - newly_live : 0;
  gc_total_freed += gc_last_freed;
  
  gc_total_collections++;
  
  size_t new_threshold = (size_t)(gc_live_bytes * gc_growth_factor);
  if (new_threshold < gc_min_threshold) {
    new_threshold = gc_min_threshold;
  }
  alloc_threshold = new_threshold;
  
  gc_allocated_bytes = 0;
  alloc_count = 0;
}

void gc_stats(void) {
  fprintf(stderr, "[GC] collections=%zu allocated=%zu live=%zu threshold=%zu\n",
          gc_total_collections, gc_allocated_bytes, gc_live_bytes, alloc_threshold);
}

void gc_benchmark_stats(void) {
  fprintf(stderr, "[GC-BENCH] total_collections=%zu\n", gc_total_collections);
  fprintf(stderr, "[GC-BENCH] total_mark_time_ms=%.3f\n", gc_total_mark_time / 1000000.0);
  fprintf(stderr, "[GC-BENCH] total_sweep_time_ms=%.3f\n", gc_total_sweep_time / 1000000.0);
  fprintf(stderr, "[GC-BENCH] total_freed_bytes=%zu\n", gc_total_freed);
  fprintf(stderr, "[GC-BENCH] avg_mark_time_us=%.3f\n", gc_total_collections > 0 ? (gc_total_mark_time / 1000.0) / gc_total_collections : 0);
  fprintf(stderr, "[GC-BENCH] avg_sweep_time_us=%.3f\n", gc_total_collections > 0 ? (gc_total_sweep_time / 1000.0) / gc_total_collections : 0);
  fprintf(stderr, "[GC-BENCH] last_collection_freed=%zu\n", gc_last_freed);
  fprintf(stderr, "[GC-BENCH] last_mark_time_us=%.3f\n", gc_last_mark_time / 1000.0);
  fprintf(stderr, "[GC-BENCH] last_sweep_time_us=%.3f\n", gc_last_sweep_time / 1000.0);
}

void gc_reset_stats(void) {
  gc_total_collections = 0;
  gc_total_mark_time = 0;
  gc_total_sweep_time = 0;
  gc_total_freed = 0;
  gc_last_freed = 0;
  gc_last_mark_time = 0;
  gc_last_sweep_time = 0;
}

/* --- Array Implementation --- */

/* Array struct defined above for tracing */

uint64_t array_length(void* array_ptr) {
  Array* arr = (Array*)array_ptr;
  if (!arr || arr->dimension_count == 0) return 0;
  return arr->dimensions[0];
}

void* array_alloc(uint64_t element_size, uint64_t dimension_count, uint64_t* dimensions) {
  Array* arr = (Array*)gc_alloc_kind(sizeof(Array), OBJ_ARRAY);
  arr->element_size = element_size;
  arr->dimension_count = dimension_count;
  
  arr->dimensions = (uint64_t*)gc_alloc(sizeof(uint64_t) * dimension_count);
  arr->strides = (uint64_t*)gc_alloc(sizeof(uint64_t) * dimension_count);
  
  memcpy(arr->dimensions, dimensions, sizeof(uint64_t) * dimension_count);
  
  uint64_t total_elements = 1;
  for (uint64_t i = 0; i < dimension_count; i++) {
    arr->strides[i] = total_elements;
    total_elements *= dimensions[i];
  }
  
  arr->data = gc_alloc(element_size * total_elements);
  
  return arr;
}

uint64_t array_get(void* array_ptr, uint64_t index) {
  Array* arr = (Array*)array_ptr;
  uint8_t* data = (uint8_t*)arr->data;
  return *(uint64_t*)(data + index * arr->element_size);
}

void array_set(void* array_ptr, uint64_t index, uint64_t value) {
  Array* arr = (Array*)array_ptr;
  uint8_t* data = (uint8_t*)arr->data;
  *(uint64_t*)(data + index * arr->element_size) = value;
}

/* Typed array operations for f32 */
float array_get_f32(void* array_ptr, uint64_t index) {
  Array* arr = (Array*)array_ptr;
  uint8_t* data = (uint8_t*)arr->data;
  return *(float*)(data + index * arr->element_size);
}

void array_set_f32(void* array_ptr, uint64_t index, float value) {
  Array* arr = (Array*)array_ptr;
  uint8_t* data = (uint8_t*)arr->data;
  *(float*)(data + index * arr->element_size) = value;
}

/* Typed array operations for f64 */
double array_get_f64(void* array_ptr, uint64_t index) {
  Array* arr = (Array*)array_ptr;
  uint8_t* data = (uint8_t*)arr->data;
  return *(double*)(data + index * arr->element_size);
}

void array_set_f64(void* array_ptr, uint64_t index, double value) {
  Array* arr = (Array*)array_ptr;
  uint8_t* data = (uint8_t*)arr->data;
  *(double*)(data + index * arr->element_size) = value;
}

/* Dot product for f32 arrays */
float dot_f32(void* a_ptr, void* b_ptr) {
  Array* a = (Array*)a_ptr;
  Array* b = (Array*)b_ptr;
  uint64_t len = a->dimensions[0];
  if (b->dimensions[0] < len) len = b->dimensions[0];
  float sum = 0.0f;
  uint8_t* a_data = (uint8_t*)a->data;
  uint8_t* b_data = (uint8_t*)b->data;
  for (uint64_t i = 0; i < len; i++) {
    float av = *(float*)(a_data + i * a->element_size);
    float bv = *(float*)(b_data + i * b->element_size);
    sum += av * bv;
  }
  return sum;
}

/* Dot product for f64 arrays */
double dot_f64(void* a_ptr, void* b_ptr) {
  Array* a = (Array*)a_ptr;
  Array* b = (Array*)b_ptr;
  uint64_t len = a->dimensions[0];
  if (b->dimensions[0] < len) len = b->dimensions[0];
  double sum = 0.0;
  uint8_t* a_data = (uint8_t*)a->data;
  uint8_t* b_data = (uint8_t*)b->data;
  for (uint64_t i = 0; i < len; i++) {
    double av = *(double*)(a_data + i * a->element_size);
    double bv = *(double*)(b_data + i * b->element_size);
    sum += av * bv;
  }
  return sum;
}

void* array_slice(void* array_ptr, uint64_t start, uint64_t end) {
  Array* arr = (Array*)array_ptr;
  uint64_t slice_len = end - start;
  if (slice_len > arr->dimensions[0] - start) {
    slice_len = arr->dimensions[0] - start;
  }
  
  Array* slice = (Array*)gc_alloc_kind(sizeof(Array), OBJ_ARRAY);
  slice->element_size = arr->element_size;
  slice->dimension_count = 1;
  slice->dimensions = (uint64_t*)gc_alloc(sizeof(uint64_t));
  slice->dimensions[0] = slice_len;
  slice->strides = (uint64_t*)gc_alloc(sizeof(uint64_t));
  slice->strides[0] = 1;
  slice->data = gc_alloc(arr->element_size * slice_len);
  
  uint8_t* src_data = (uint8_t*)arr->data;
  uint8_t* dst_data = (uint8_t*)slice->data;
  for (uint64_t i = 0; i < slice_len; i++) {
    *(uint64_t*)(dst_data + i * arr->element_size) = 
      *(uint64_t*)(src_data + (start + i) * arr->element_size);
  }
  
  return slice;
}

void* array_slice_step(void* array_ptr, uint64_t start, uint64_t end, uint64_t step) {
  if (step == 0) step = 1;

  Array* arr = (Array*)array_ptr;

  // Calculate number of elements in sliced result
  uint64_t available_len = end > start ? end - start : 0;
  if (available_len > arr->dimensions[0] - start) {
    available_len = arr->dimensions[0] - start;
  }
  uint64_t slice_len = (available_len + step - 1) / step;  // Ceiling division

  Array* slice = (Array*)gc_alloc_kind(sizeof(Array), OBJ_ARRAY);
  slice->element_size = arr->element_size;
  slice->dimension_count = 1;
  slice->dimensions = (uint64_t*)gc_alloc(sizeof(uint64_t));
  slice->dimensions[0] = slice_len;
  slice->strides = (uint64_t*)gc_alloc(sizeof(uint64_t));
  slice->strides[0] = 1;
  slice->data = gc_alloc(arr->element_size * slice_len);

  uint8_t* src_data = (uint8_t*)arr->data;
  uint8_t* dst_data = (uint8_t*)slice->data;
  for (uint64_t i = 0; i < slice_len; i++) {
    uint64_t src_idx = start + i * step;
    if (src_idx >= arr->dimensions[0]) break;
    memcpy(dst_data + i * arr->element_size,
           src_data + src_idx * arr->element_size,
           arr->element_size);
  }

  return slice;
}

/* --- Array Methods Implementation --- */

void array_push(void* array_ptr, uint64_t value) {
  Array* arr = (Array*)array_ptr;
  if (!arr || arr->dimension_count == 0) return;

  uint64_t old_len = arr->dimensions[0];
  uint64_t new_len = old_len + 1;

  /* Allocate new data buffer with one more element */
  void* new_data = gc_alloc(arr->element_size * new_len);
  if (old_len > 0 && arr->data) {
    memcpy(new_data, arr->data, arr->element_size * old_len);
  }

  /* Set the new element */
  uint8_t* data = (uint8_t*)new_data;
  *(uint64_t*)(data + old_len * arr->element_size) = value;

  /* Update array */
  arr->data = new_data;
  arr->dimensions[0] = new_len;
}

uint64_t array_pop(void* array_ptr) {
  Array* arr = (Array*)array_ptr;
  if (!arr || arr->dimension_count == 0 || arr->dimensions[0] == 0) return 0;

  uint64_t old_len = arr->dimensions[0];
  uint64_t new_len = old_len - 1;

  /* Get the last element before shrinking */
  uint8_t* data = (uint8_t*)arr->data;
  uint64_t value = *(uint64_t*)(data + new_len * arr->element_size);

  /* Shrink: just update the dimension (data stays allocated, GC will handle) */
  arr->dimensions[0] = new_len;

  return value;
}

uint64_t array_contains(void* array_ptr, uint64_t value) {
  Array* arr = (Array*)array_ptr;
  if (!arr || arr->dimension_count == 0) return 0;

  uint64_t len = arr->dimensions[0];
  uint8_t* data = (uint8_t*)arr->data;
  for (uint64_t i = 0; i < len; i++) {
    uint64_t elem = *(uint64_t*)(data + i * arr->element_size);
    if (elem == value) return 1;
  }
  return 0;
}

/* Comparison function for qsort (ascending i64 order) */
static int array_sort_cmp(const void* a, const void* b) {
  int64_t va = *(const int64_t*)a;
  int64_t vb = *(const int64_t*)b;
  if (va < vb) return -1;
  if (va > vb) return 1;
  return 0;
}

void array_sort(void* array_ptr) {
  Array* arr = (Array*)array_ptr;
  if (!arr || arr->dimension_count == 0 || arr->dimensions[0] <= 1) return;

  qsort(arr->data, arr->dimensions[0], arr->element_size, array_sort_cmp);
}

void array_reverse(void* array_ptr) {
  Array* arr = (Array*)array_ptr;
  if (!arr || arr->dimension_count == 0 || arr->dimensions[0] <= 1) return;

  uint64_t len = arr->dimensions[0];
  uint8_t* data = (uint8_t*)arr->data;
  uint64_t elem_size = arr->element_size;

  /* Swap elements from both ends toward the middle */
  uint8_t temp[8]; /* element_size is at most 8 bytes (i64/f64/ptr) */
  for (uint64_t i = 0; i < len / 2; i++) {
    uint64_t j = len - 1 - i;
    uint8_t* a = data + i * elem_size;
    uint8_t* b = data + j * elem_size;
    memcpy(temp, a, elem_size);
    memcpy(a, b, elem_size);
    memcpy(b, temp, elem_size);
  }
}

/* --- Map Implementation --- */
/* MapEntry and Map structs defined above for tracing */

uint64_t map_hash(const char* key, uint64_t key_len) {
  uint64_t hash = 5381;
  for (uint64_t i = 0; i < key_len; i++) {
    hash = ((hash << 5) + hash) + key[i];
  }
  return hash;
}

void* map_new(uint64_t initial_capacity) {
  Map* map = (Map*)gc_alloc_kind(sizeof(Map), OBJ_MAP);
  map->capacity = initial_capacity;
  map->count = 0;
  map->buckets = (MapEntry**)gc_alloc(sizeof(MapEntry*) * initial_capacity);

  for (uint64_t i = 0; i < initial_capacity; i++) {
    map->buckets[i] = NULL;
  }

  return map;
}

uint64_t map_get(void* map_ptr, const char* key, uint64_t key_len) {
  Map* map = (Map*)map_ptr;
  uint64_t hash = map_hash(key, key_len) % map->capacity;

  MapEntry* entry = map->buckets[hash];
  while (entry) {
    if (entry->key_len == key_len && memcmp(entry->key, key, key_len) == 0) {
      return entry->value;
    }
    entry = entry->next;
  }

  return 0;
}

void map_set(void* map_ptr, const char* key, uint64_t key_len, uint64_t value) {
  Map* map = (Map*)map_ptr;
  uint64_t hash = map_hash(key, key_len) % map->capacity;

  MapEntry* entry = map->buckets[hash];
  while (entry) {
    if (entry->key_len == key_len && memcmp(entry->key, key, key_len) == 0) {
      entry->value = value;
      return;
    }
    entry = entry->next;
  }

  MapEntry* new_entry = (MapEntry*)gc_alloc_kind(sizeof(MapEntry), OBJ_ENTRY);
  new_entry->key = (char*)gc_alloc(key_len);
  memcpy(new_entry->key, key, key_len);
  new_entry->key_len = key_len;
  new_entry->value = value;
  new_entry->next = map->buckets[hash];
  map->buckets[hash] = new_entry;
  map->count++;
}

/* Map iteration helpers */
uint64_t map_size(void* map_ptr) {
  Map* map = (Map*)map_ptr;
  if (!map) return 0;
  return map->count;
}

void* map_key_at(void* map_ptr, uint64_t index) {
  Map* map = (Map*)map_ptr;
  if (!map) return NULL;
  uint64_t seen = 0;
  for (uint64_t i = 0; i < map->capacity; i++) {
    MapEntry* entry = map->buckets[i];
    while (entry) {
      if (seen == index) {
        /* Return key as a null-terminated GC-allocated string */
        char* key_copy = (char*)gc_alloc(entry->key_len + 1);
        memcpy(key_copy, entry->key, entry->key_len);
        key_copy[entry->key_len] = '\0';
        return key_copy;
      }
      seen++;
      entry = entry->next;
    }
  }
  return NULL;
}

uint64_t map_value_at(void* map_ptr, uint64_t index) {
  Map* map = (Map*)map_ptr;
  if (!map) return 0;
  uint64_t seen = 0;
  for (uint64_t i = 0; i < map->capacity; i++) {
    MapEntry* entry = map->buckets[i];
    while (entry) {
      if (seen == index) {
        return entry->value;
      }
      seen++;
      entry = entry->next;
    }
  }
  return 0;
}

/* Array dimension access */
uint64_t array_get_dimension(void* array_ptr, uint64_t dim_index) {
  Array* arr = (Array*)array_ptr;
  if (!arr || dim_index >= arr->dimension_count) return 0;
  return arr->dimensions[dim_index];
}

uint64_t array_get_dimension_count(void* array_ptr) {
  Array* arr = (Array*)array_ptr;
  if (!arr) return 0;
  return arr->dimension_count;
}

/* Matrix multiplication - handles f32 and f64 2D arrays */
void* matmul(void* a_ptr, void* b_ptr) {
  Array* a = (Array*)a_ptr;
  Array* b = (Array*)b_ptr;
  
  if (!a || !b) return NULL;
  
  /* Determine dimensions: treat 1D as row vector */
  uint64_t a_rows = a->dimensions[0];
  uint64_t a_cols = (a->dimension_count > 1) ? a->dimensions[1] : 1;
  uint64_t b_rows = b->dimensions[0];
  uint64_t b_cols = (b->dimension_count > 1) ? b->dimensions[1] : 1;
  
  /* Check compatibility: a_cols must equal b_rows */
  if (a_cols != b_rows) return NULL;
  
  /* Element size must match */
  if (a->element_size != b->element_size) return NULL;
  
  /* Create result array */
  uint64_t dims[2] = {a_rows, b_cols};
  Array* c = (Array*)array_alloc(a->element_size, 2, dims);
  
  uint8_t* a_data = (uint8_t*)a->data;
  uint8_t* b_data = (uint8_t*)b->data;
  uint8_t* c_data = (uint8_t*)c->data;
  
  if (a->element_size == 4) {  /* f32 */
    for (uint64_t i = 0; i < a_rows; i++) {
      for (uint64_t j = 0; j < b_cols; j++) {
        float sum = 0.0f;
        for (uint64_t k = 0; k < a_cols; k++) {
          float av = *(float*)(a_data + (i * a_cols + k) * 4);
          float bv = *(float*)(b_data + (k * b_cols + j) * 4);
          sum += av * bv;
        }
        *(float*)(c_data + (i * b_cols + j) * 4) = sum;
      }
    }
  } else if (a->element_size == 8) {  /* f64 */
    for (uint64_t i = 0; i < a_rows; i++) {
      for (uint64_t j = 0; j < b_cols; j++) {
        double sum = 0.0;
        for (uint64_t k = 0; k < a_cols; k++) {
          double av = *(double*)(a_data + (i * a_cols + k) * 8);
          double bv = *(double*)(b_data + (k * b_cols + j) * 8);
          sum += av * bv;
        }
        *(double*)(c_data + (i * b_cols + j) * 8) = sum;
      }
    }
  }
  
  return c;
}

/* Matrix addition - element-wise for 2D arrays */
void* matrix_add(void* a_ptr, void* b_ptr) {
  Array* a = (Array*)a_ptr;
  Array* b = (Array*)b_ptr;
  
  if (!a || !b) return NULL;
  
  /* Get dimensions - treat 1D as row vector */
  uint64_t a_rows = a->dimensions[0];
  uint64_t a_cols = (a->dimension_count > 1) ? a->dimensions[1] : 1;
  uint64_t b_rows = b->dimensions[0];
  uint64_t b_cols = (b->dimension_count > 1) ? b->dimensions[1] : 1;
  
  /* Dimensions must match */
  if (a_rows != b_rows || a_cols != b_cols) return NULL;
  
  /* Element size must match */
  if (a->element_size != b->element_size) return NULL;
  
  /* Create result array */
  uint64_t dims[2] = {a_rows, a_cols};
  Array* c = (Array*)array_alloc(a->element_size, 2, dims);
  
  uint8_t* a_data = (uint8_t*)a->data;
  uint8_t* b_data = (uint8_t*)b->data;
  uint8_t* c_data = (uint8_t*)c->data;
  uint64_t total = a_rows * a_cols;
  
  if (a->element_size == 4) {  /* f32 */
    for (uint64_t i = 0; i < total; i++) {
      float av = *(float*)(a_data + i * 4);
      float bv = *(float*)(b_data + i * 4);
      *(float*)(c_data + i * 4) = av + bv;
    }
  } else if (a->element_size == 8) {  /* f64 */
    for (uint64_t i = 0; i < total; i++) {
      double av = *(double*)(a_data + i * 8);
      double bv = *(double*)(b_data + i * 8);
      *(double*)(c_data + i * 8) = av + bv;
    }
  }
  
  return c;
}

/* Vector Operations */

void* vector_add(void* a, void* b, uint64_t size) {
  double* result = (double*)gc_alloc(sizeof(double) * size);
  double* av = (double*)a;
  double* bv = (double*)b;
  
  for (uint64_t i = 0; i < size; i++) {
    result[i] = av[i] + bv[i];
  }
  
  return result;
}

void* vector_sub(void* a, void* b, uint64_t size) {
  double* result = (double*)gc_alloc(sizeof(double) * size);
  double* av = (double*)a;
  double* bv = (double*)b;
  
  for (uint64_t i = 0; i < size; i++) {
    result[i] = av[i] - bv[i];
  }
  
  return result;
}

uint64_t vector_dot(void* a, void* b, uint64_t size) {
  double* av = (double*)a;
  double* bv = (double*)b;
  double sum = 0;
  
  for (uint64_t i = 0; i < size; i++) {
    sum += av[i] * bv[i];
  }
  
  return (uint64_t)sum;
}

void* matrix_mul(void* a, void* b, uint64_t rows_a, uint64_t cols_a, uint64_t cols_b) {
  double* result = (double*)gc_alloc(sizeof(double) * rows_a * cols_b);
  double* am = (double*)a;
  double* bm = (double*)b;
  
  for (uint64_t i = 0; i < rows_a; i++) {
    for (uint64_t j = 0; j < cols_b; j++) {
      double sum = 0;
      for (uint64_t k = 0; k < cols_a; k++) {
        sum += am[i * cols_a + k] * bm[k * cols_b + j];
      }
      result[i * cols_b + j] = sum;
    }
  }
  
  return result;
}

/* String Operations */

uint64_t string_length(const char* str) {
  if (!str) return 0;
  return strlen(str);
}

char* string_concat(const char* a, const char* b) {
  uint64_t len_a = strlen(a);
  uint64_t len_b = strlen(b);
  
  char* result = (char*)gc_alloc(len_a + len_b + 1);
  memcpy(result, a, len_a);
  memcpy(result + len_a, b, len_b + 1);
  
  return result;
}

// Get character at index (returns new single-char string)
char* string_char_at(const char* str, uint64_t index) {
  uint64_t len = strlen(str);
  if (index >= len) {
    return "";
  }
  char* result = (char*)gc_alloc(2);
  result[0] = str[index];
  result[1] = '\0';
  return result;
}

// Slice string from start (inclusive) to end (exclusive)
char* string_slice(const char* str, uint64_t start, uint64_t end) {
  uint64_t len = strlen(str);
  if (start >= len) {
    return "";
  }
  if (end > len) {
    end = len;
  }
  if (end <= start) {
    return "";
  }
  uint64_t slice_len = end - start;
  char* result = (char*)gc_alloc(slice_len + 1);
  memcpy(result, str + start, slice_len);
  result[slice_len] = '\0';
  return result;
}

/* String Conversion Functions */

char* i64_to_string(int64_t value) {
  // Enough for int64_t including sign and null terminator
  char buffer[21];
  snprintf(buffer, sizeof(buffer), "%lld", (long long)value);
  uint64_t len = strlen(buffer);
  char* result = (char*)gc_alloc(len + 1);
  memcpy(result, buffer, len + 1);
  return result;
}

char* u64_to_string(uint64_t value) {
  // Enough for uint64_t and null terminator
  char buffer[21];
  snprintf(buffer, sizeof(buffer), "%llu", (unsigned long long)value);
  uint64_t len = strlen(buffer);
  char* result = (char*)gc_alloc(len + 1);
  memcpy(result, buffer, len + 1);
  return result;
}

char* f64_to_string(double value) {
  // Enough for double with precision and null terminator
  char buffer[64];
  snprintf(buffer, sizeof(buffer), "%f", value);
  uint64_t len = strlen(buffer);
  char* result = (char*)gc_alloc(len + 1);
  memcpy(result, buffer, len + 1);
  return result;
}

/* Forward declarations for Result functions used by string methods */
void* mog_result_ok(uint64_t value);
void* mog_result_err(const char* message);

/* String Method Functions */

char* string_upper(const char* str) {
  if (!str) return "";
  uint64_t len = strlen(str);
  char* result = (char*)gc_alloc(len + 1);
  for (uint64_t i = 0; i < len; i++) {
    unsigned char c = (unsigned char)str[i];
    result[i] = (c >= 'a' && c <= 'z') ? (char)(c - 32) : (char)c;
  }
  result[len] = '\0';
  return result;
}

char* string_lower(const char* str) {
  if (!str) return "";
  uint64_t len = strlen(str);
  char* result = (char*)gc_alloc(len + 1);
  for (uint64_t i = 0; i < len; i++) {
    unsigned char c = (unsigned char)str[i];
    result[i] = (c >= 'A' && c <= 'Z') ? (char)(c + 32) : (char)c;
  }
  result[len] = '\0';
  return result;
}

char* string_trim(const char* str) {
  if (!str) return "";
  uint64_t len = strlen(str);
  uint64_t start = 0;
  while (start < len && (str[start] == ' ' || str[start] == '\t' || str[start] == '\n' || str[start] == '\r')) {
    start++;
  }
  uint64_t end = len;
  while (end > start && (str[end - 1] == ' ' || str[end - 1] == '\t' || str[end - 1] == '\n' || str[end - 1] == '\r')) {
    end--;
  }
  uint64_t result_len = end - start;
  char* result = (char*)gc_alloc(result_len + 1);
  memcpy(result, str + start, result_len);
  result[result_len] = '\0';
  return result;
}

void* string_split(const char* str, const char* delimiter) {
  if (!str || !delimiter) {
    /* Return empty array */
    uint64_t dims[1] = {0};
    return array_alloc(8, 1, dims);
  }
  uint64_t delim_len = strlen(delimiter);
  if (delim_len == 0) {
    /* Split into individual characters */
    uint64_t len = strlen(str);
    uint64_t dims[1] = {len};
    void* arr = array_alloc(8, 1, dims);
    for (uint64_t i = 0; i < len; i++) {
      char* ch = (char*)gc_alloc(2);
      ch[0] = str[i];
      ch[1] = '\0';
      array_set(arr, i, (uint64_t)(uintptr_t)ch);
    }
    return arr;
  }
  
  /* First pass: count splits */
  uint64_t count = 1;
  const char* p = str;
  while ((p = strstr(p, delimiter)) != NULL) {
    count++;
    p += delim_len;
  }
  
  /* Allocate array */
  uint64_t dims[1] = {count};
  void* arr = array_alloc(8, 1, dims);
  
  /* Second pass: extract substrings */
  const char* start = str;
  uint64_t idx = 0;
  p = str;
  while ((p = strstr(start, delimiter)) != NULL) {
    uint64_t part_len = (uint64_t)(p - start);
    char* part = (char*)gc_alloc(part_len + 1);
    memcpy(part, start, part_len);
    part[part_len] = '\0';
    array_set(arr, idx, (uint64_t)(uintptr_t)part);
    idx++;
    start = p + delim_len;
  }
  /* Last part */
  uint64_t last_len = strlen(start);
  char* last = (char*)gc_alloc(last_len + 1);
  memcpy(last, start, last_len + 1);
  array_set(arr, idx, (uint64_t)(uintptr_t)last);
  
  return arr;
}

uint64_t string_contains(const char* str, const char* substring) {
  if (!str || !substring) return 0;
  return strstr(str, substring) != NULL ? 1 : 0;
}

uint64_t string_starts_with(const char* str, const char* prefix) {
  if (!str || !prefix) return 0;
  uint64_t prefix_len = strlen(prefix);
  if (strlen(str) < prefix_len) return 0;
  return strncmp(str, prefix, prefix_len) == 0 ? 1 : 0;
}

uint64_t string_ends_with(const char* str, const char* suffix) {
  if (!str || !suffix) return 0;
  uint64_t str_len = strlen(str);
  uint64_t suffix_len = strlen(suffix);
  if (str_len < suffix_len) return 0;
  return strcmp(str + str_len - suffix_len, suffix) == 0 ? 1 : 0;
}

char* string_replace(const char* str, const char* old_str, const char* new_str) {
  if (!str || !old_str || !new_str) return (char*)str;
  uint64_t old_len = strlen(old_str);
  uint64_t new_len = strlen(new_str);
  if (old_len == 0) return (char*)str;
  
  /* Count occurrences */
  uint64_t count = 0;
  const char* p = str;
  while ((p = strstr(p, old_str)) != NULL) {
    count++;
    p += old_len;
  }
  if (count == 0) {
    uint64_t len = strlen(str);
    char* result = (char*)gc_alloc(len + 1);
    memcpy(result, str, len + 1);
    return result;
  }
  
  uint64_t str_len = strlen(str);
  uint64_t result_len = str_len + count * (new_len - old_len);
  char* result = (char*)gc_alloc(result_len + 1);
  
  char* dest = result;
  const char* src = str;
  while ((p = strstr(src, old_str)) != NULL) {
    uint64_t chunk_len = (uint64_t)(p - src);
    memcpy(dest, src, chunk_len);
    dest += chunk_len;
    memcpy(dest, new_str, new_len);
    dest += new_len;
    src = p + old_len;
  }
  /* Copy remaining */
  strcpy(dest, src);
  
  return result;
}

/* Conversion functions: string -> int/float (returns Result) */

void* int_from_string(const char* str) {
  if (!str) return mog_result_err("null string");
  char* endptr;
  long long val = strtoll(str, &endptr, 10);
  if (endptr == str || *endptr != '\0') {
    return mog_result_err("invalid integer");
  }
  return mog_result_ok((int64_t)val);
}

void* float_from_string(const char* str) {
  if (!str) return mog_result_err("null string");
  char* endptr;
  double val = strtod(str, &endptr);
  if (endptr == str || *endptr != '\0') {
    return mog_result_err("invalid float");
  }
  int64_t bits;
  memcpy(&bits, &val, sizeof(bits));
  return mog_result_ok(bits);
}

/* I/O Functions */

void print_i64(int64_t value) {
  printf("%lld", (long long)value);
}

void print_u64(uint64_t value) {
  printf("%llu", (unsigned long long)value);
}

void print_f64(double value) {
  printf("%f", value);
}

void print_string(const char* str) {
  printf("%s", str ? str : "(null)");
}

void print_buffer(const char* buf, int64_t len) {
  if (!buf || len <= 0) return;
  fwrite(buf, 1, (size_t)len, stdout);
}

void println(void) {
  printf("\n");
}

void println_i64(int64_t value) {
  printf("%lld\n", (long long)value);
}

void println_u64(uint64_t value) {
  printf("%llu\n", (unsigned long long)value);
}

void println_f64(double value) {
  printf("%f\n", value);
}

void println_string(const char* str) {
  printf("%s\n", str ? str : "(null)");
}

int64_t input_i64(void) {
  long long value;
  scanf("%lld", &value);
  return (int64_t)value;
}

uint64_t input_u64(void) {
  unsigned long long value;
  scanf("%llu", &value);
  return (uint64_t)value;
}

/* CLI Argument Access */

uint64_t get_argc_value(void* cli_map) {
  if (!cli_map) return 0;
  return map_get(cli_map, "argc", 4);
}

uint64_t get_argv_value(void* cli_map, uint64_t index) {
  if (!cli_map) return 0;

  uint64_t args_array = map_get(cli_map, "args", 4);
  if (!args_array) return 0;

  return array_get((void*)args_array, index);
}

/* ============================================================================
 * Result<T> and Optional<T> (Phase 4: Error Handling)
 *
 * Result is a tagged union: 16 bytes allocated via gc_alloc
 *   offset 0: tag (i64) - 0 = ok, 1 = err
 *   offset 8: value (i64) - for ok: the value; for err: pointer to error string
 *
 * Optional is the same layout:
 *   offset 0: tag (i64) - 0 = none, 1 = some
 *   offset 8: value (i64) - for some: the value; for none: unused
 * ============================================================================ */

void* mog_result_ok(uint64_t value) {
  uint64_t* result = (uint64_t*)gc_alloc(16);
  result[0] = 0;  /* tag = ok */
  result[1] = value;
  return result;
}

void* mog_result_err(const char* message) {
  uint64_t* result = (uint64_t*)gc_alloc(16);
  result[0] = 1;  /* tag = err */
  result[1] = (uint64_t)(uintptr_t)message;
  return result;
}

uint64_t mog_result_is_ok(void* result) {
  if (!result) return 0;
  uint64_t* r = (uint64_t*)result;
  return r[0] == 0 ? 1 : 0;
}

uint64_t mog_result_unwrap(void* result) {
  if (!result) return 0;
  uint64_t* r = (uint64_t*)result;
  return r[1];
}

void* mog_result_unwrap_err(void* result) {
  if (!result) return NULL;
  uint64_t* r = (uint64_t*)result;
  return (void*)(uintptr_t)r[1];
}

void* mog_optional_some(uint64_t value) {
  uint64_t* opt = (uint64_t*)gc_alloc(16);
  opt[0] = 1;  /* tag = some */
  opt[1] = value;
  return opt;
}

void* mog_optional_none(void) {
  uint64_t* opt = (uint64_t*)gc_alloc(16);
  opt[0] = 0;  /* tag = none */
  opt[1] = 0;
  return opt;
}

uint64_t mog_optional_is_some(void* optional) {
  if (!optional) return 0;
  uint64_t* o = (uint64_t*)optional;
  return o[0] == 1 ? 1 : 0;
}

uint64_t mog_optional_unwrap(void* optional) {
  if (!optional) return 0;
  uint64_t* o = (uint64_t*)optional;
  return o[1];
}
