#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* GC Implementation - Mark and Sweep */

typedef struct Block {
  size_t size;
  uint8_t marked;
  struct Block* next;
} Block;

static Block* heap = NULL;
static size_t heap_size = 0;
static size_t alloc_threshold = 1024 * 1024;
static size_t alloc_count = 0;

void gc_init(void) {
  heap = NULL;
  heap_size = 0;
  alloc_count = 0;
}

void gc_mark(Block* ptr) {
  if (!ptr || ptr->marked) return;
  ptr->marked = 1;
}

void gc_mark_roots(void) {
}

void gc_sweep(void) {
  Block** ptr = &heap;
  while (*ptr) {
    if (!(*ptr)->marked) {
      Block* to_free = *ptr;
      *ptr = to_free->next;
      heap_size -= to_free->size;
      free(to_free);
    } else {
      (*ptr)->marked = 0;
      ptr = &(*ptr)->next;
    }
  }
}

void gc_collect(void) {
  gc_mark_roots();
  gc_sweep();
  alloc_count = 0;
}

void* gc_alloc(size_t size) {
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
  block->next = heap;
  heap = block;
  heap_size += size;
  alloc_count += size;
  
  if (alloc_count >= alloc_threshold) {
    gc_collect();
  }
  
  return (void*)((uintptr_t)ptr + sizeof(Block));
}

/* Array Implementation */

typedef struct Array {
  uint64_t element_size;
  uint64_t dimension_count;
  uint64_t* dimensions;
  uint64_t* strides;
  void* data;
} Array;

void* array_alloc(uint64_t element_size, uint64_t dimension_count, uint64_t* dimensions) {
  Array* arr = (Array*)gc_alloc(sizeof(Array));
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

/* Table Implementation */

typedef struct TableEntry {
  char* key;
  uint64_t key_len;
  uint64_t value;
  struct TableEntry* next;
} TableEntry;

typedef struct Table {
  TableEntry** buckets;
  uint64_t capacity;
  uint64_t count;
} Table;

uint64_t table_hash(const char* key, uint64_t key_len) {
  uint64_t hash = 5381;
  for (uint64_t i = 0; i < key_len; i++) {
    hash = ((hash << 5) + hash) + key[i];
  }
  return hash;
}

void* table_new(uint64_t initial_capacity) {
  Table* table = (Table*)gc_alloc(sizeof(Table));
  table->capacity = initial_capacity;
  table->count = 0;
  table->buckets = (TableEntry**)gc_alloc(sizeof(TableEntry*) * initial_capacity);
  
  for (uint64_t i = 0; i < initial_capacity; i++) {
    table->buckets[i] = NULL;
  }
  
  return table;
}

uint64_t table_get(void* table_ptr, const char* key, uint64_t key_len) {
  Table* table = (Table*)table_ptr;
  uint64_t hash = table_hash(key, key_len) % table->capacity;
  
  TableEntry* entry = table->buckets[hash];
  while (entry) {
    if (entry->key_len == key_len && memcmp(entry->key, key, key_len) == 0) {
      return entry->value;
    }
    entry = entry->next;
  }
  
  return 0;
}

void table_set(void* table_ptr, const char* key, uint64_t key_len, uint64_t value) {
  Table* table = (Table*)table_ptr;
  uint64_t hash = table_hash(key, key_len) % table->capacity;
  
  TableEntry* entry = table->buckets[hash];
  while (entry) {
    if (entry->key_len == key_len && memcmp(entry->key, key, key_len) == 0) {
      entry->value = value;
      return;
    }
    entry = entry->next;
  }
  
  TableEntry* new_entry = (TableEntry*)gc_alloc(sizeof(TableEntry));
  new_entry->key = (char*)gc_alloc(key_len);
  memcpy(new_entry->key, key, key_len);
  new_entry->key_len = key_len;
  new_entry->value = value;
  new_entry->next = table->buckets[hash];
  table->buckets[hash] = new_entry;
  table->count++;
}

/* LLM Integration Placeholder */

void* llm_call(const char* prompt, const char* options, const char* return_type) {
  printf("[LLM Call] Prompt: %s\n", prompt);
  printf("[LLM Call] Options: %s\n", options);
  printf("[LLM Call] Return Type: %s\n", return_type);
  
  /* Placeholder: return the prompt as-is */
  uint64_t len = strlen(prompt);
  void* result = gc_alloc(len + 1);
  memcpy(result, prompt, len + 1);
  
  return result;
}

/* Vector and Matrix Operations */

void* vector_add(void* a, void* b, uint64_t size) {
  uint64_t* result = (uint64_t*)gc_alloc(sizeof(uint64_t) * size);
  uint64_t* arr_a = (uint64_t*)a;
  uint64_t* arr_b = (uint64_t*)b;
  
  for (uint64_t i = 0; i < size; i++) {
    result[i] = arr_a[i] + arr_b[i];
  }
  
  return result;
}

void* vector_sub(void* a, void* b, uint64_t size) {
  uint64_t* result = (uint64_t*)gc_alloc(sizeof(uint64_t) * size);
  uint64_t* arr_a = (uint64_t*)a;
  uint64_t* arr_b = (uint64_t*)b;
  
  for (uint64_t i = 0; i < size; i++) {
    result[i] = arr_a[i] - arr_b[i];
  }
  
  return result;
}

uint64_t vector_dot(void* a, void* b, uint64_t size) {
  uint64_t* arr_a = (uint64_t*)a;
  uint64_t* arr_b = (uint64_t*)b;
  uint64_t result = 0;
  
  for (uint64_t i = 0; i < size; i++) {
    result += arr_a[i] * arr_b[i];
  }
  
  return result;
}

void* matrix_mul(void* a, void* b, uint64_t rows_a, uint64_t cols_a, uint64_t cols_b) {
  uint64_t* result = (uint64_t*)gc_alloc(sizeof(uint64_t) * rows_a * cols_b);
  uint64_t* mat_a = (uint64_t*)a;
  uint64_t* mat_b = (uint64_t*)b;
  
  for (uint64_t i = 0; i < rows_a; i++) {
    for (uint64_t j = 0; j < cols_b; j++) {
      uint64_t sum = 0;
      for (uint64_t k = 0; k < cols_a; k++) {
        sum += mat_a[i * cols_a + k] * mat_b[k * cols_b + j];
      }
      result[i * cols_b + j] = sum;
    }
  }
  
  return result;
}

/* Print Functions */

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
  printf("%s", str);
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
  printf("%s\n", str);
}

void println(void) {
  printf("\n");
}

/* String Functions */

uint64_t string_length(const char* str) {
  if (!str) return 0;
  return strlen(str);
}

char* string_concat(const char* a, const char* b) {
  if (!a) a = "";
  if (!b) b = "";
  size_t len_a = strlen(a);
  size_t len_b = strlen(b);
  char* result = (char*)gc_alloc(len_a + len_b + 1);
  memcpy(result, a, len_a);
  memcpy(result + len_a, b, len_b);
  result[len_a + len_b] = '\0';
  return result;
}

/* Input Functions */

int64_t input_i64(void) {
  int64_t value;
  scanf("%lld", (long long*)&value);
  return value;
}

uint64_t input_u64(void) {
  uint64_t value;
  scanf("%llu", (unsigned long long*)&value);
  return value;
}

double input_f64(void) {
  double value;
  scanf("%lf", &value);
  return value;
}

char* input_string(void) {
  char* buffer = (char*)gc_alloc(1024);
  if (fgets(buffer, 1024, stdin) != NULL) {
    size_t len = strlen(buffer);
    if (len > 0 && buffer[len-1] == '\n') {
      buffer[len-1] = '\0';
    }
  }
  return buffer;
}

/* CLI Argument Helpers */

uint64_t get_argc_value(void* cli_table) {
  if (!cli_table) return 0;
  return table_get(cli_table, "argc", 4);
}

uint64_t get_argv_value(void* cli_table, uint64_t index) {
  if (!cli_table) return 0;
  
  Array* args_array = (Array*)table_get(cli_table, "args", 4);
  if (!args_array) return 0;
  
  return array_get((void*)args_array, index);
}