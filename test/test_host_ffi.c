/**
 * End-to-end integration test for the Mog Host FFI system.
 *
 * This test:
 * 1. Creates a MogVM
 * 2. Registers "math" capability with add() and multiply()
 * 3. Tests calling these functions through mog_cap_call
 * 4. Tests value constructors and extractors
 * 5. Tests safety features (type confusion, bounds checking)
 * 6. Tests capability validation
 *
 * Compile with:
 *   clang -I../runtime test_host_ffi.c ../runtime/mog_vm.c -o test_host_ffi
 *
 * Run:
 *   ./test_host_ffi
 */

#include "../runtime/mog.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <assert.h>

// Track test results
static int tests_run = 0;
static int tests_passed = 0;

#define TEST(name) do { \
    tests_run++; \
    printf("  TEST: %s ... ", name); \
} while(0)

#define PASS() do { \
    tests_passed++; \
    printf("PASS\n"); \
} while(0)

#define FAIL(msg) do { \
    printf("FAIL: %s\n", msg); \
} while(0)

// ============================================================
// Host functions for the "math" capability
// ============================================================

static MogValue math_add(MogVM *vm, MogArgs *args) {
    int64_t a = mog_arg_int(args, 0);
    int64_t b = mog_arg_int(args, 1);
    return mog_int(a + b);
}

static MogValue math_multiply(MogVM *vm, MogArgs *args) {
    int64_t a = mog_arg_int(args, 0);
    int64_t b = mog_arg_int(args, 1);
    return mog_int(a * b);
}

static MogValue math_subtract(MogVM *vm, MogArgs *args) {
    int64_t a = mog_arg_int(args, 0);
    int64_t b = mog_arg_int(args, 1);
    return mog_int(a - b);
}

// ============================================================
// Host functions for the "log" capability
// ============================================================

static char last_log_message[256] = {0};

static MogValue log_info(MogVM *vm, MogArgs *args) {
    const char *msg = mog_arg_string(args, 0);
    strncpy(last_log_message, msg, sizeof(last_log_message) - 1);
    printf("    [LOG] %s\n", msg);
    return mog_none();
}

// ============================================================
// Test: VM Lifecycle
// ============================================================

void test_vm_lifecycle(void) {
    printf("\n--- VM Lifecycle ---\n");
    
    TEST("create and free VM");
    MogVM *vm = mog_vm_new();
    assert(vm != NULL);
    mog_vm_free(vm);
    PASS();
    
    TEST("free NULL VM (no crash)");
    mog_vm_free(NULL);
    PASS();
}

// ============================================================
// Test: Value Constructors and Extractors
// ============================================================

void test_value_constructors(void) {
    printf("\n--- Value Constructors ---\n");
    
    TEST("mog_int");
    MogValue v = mog_int(42);
    assert(v.tag == MOG_INT);
    assert(v.data.i == 42);
    PASS();
    
    TEST("mog_float");
    v = mog_float(3.14);
    assert(v.tag == MOG_FLOAT);
    assert(v.data.f == 3.14);
    PASS();
    
    TEST("mog_bool true");
    v = mog_bool(true);
    assert(v.tag == MOG_BOOL);
    assert(v.data.b == true);
    PASS();
    
    TEST("mog_bool false");
    v = mog_bool(false);
    assert(v.tag == MOG_BOOL);
    assert(v.data.b == false);
    PASS();
    
    TEST("mog_string");
    v = mog_string("hello");
    assert(v.tag == MOG_STRING);
    assert(strcmp(v.data.s, "hello") == 0);
    PASS();
    
    TEST("mog_none");
    v = mog_none();
    assert(v.tag == MOG_NONE);
    PASS();
    
    TEST("mog_error");
    v = mog_error("something went wrong");
    assert(v.tag == MOG_ERROR);
    assert(strcmp(v.data.error, "something went wrong") == 0);
    PASS();
    
    TEST("mog_handle");
    int dummy = 123;
    v = mog_handle(&dummy, "MyType");
    assert(v.tag == MOG_HANDLE);
    assert(v.data.handle.ptr == &dummy);
    assert(strcmp(v.data.handle.type_name, "MyType") == 0);
    PASS();
}

// ============================================================
// Test: Arg Extractors
// ============================================================

void test_arg_extractors(void) {
    printf("\n--- Arg Extractors ---\n");
    
    MogValue vals[3];
    vals[0] = mog_int(10);
    vals[1] = mog_string("hello");
    vals[2] = mog_float(2.5);
    MogArgs args = { vals, 3 };
    
    TEST("mog_arg_int");
    assert(mog_arg_int(&args, 0) == 10);
    PASS();
    
    TEST("mog_arg_string");
    assert(strcmp(mog_arg_string(&args, 1), "hello") == 0);
    PASS();
    
    TEST("mog_arg_float");
    assert(mog_arg_float(&args, 2) == 2.5);
    PASS();
    
    TEST("mog_arg_present - present");
    assert(mog_arg_present(&args, 0) == true);
    PASS();
    
    TEST("mog_arg_present - out of bounds");
    assert(mog_arg_present(&args, 5) == false);
    PASS();
    
    TEST("mog_arg_present - none value");
    MogValue none_val = mog_none();
    MogArgs none_args = { &none_val, 1 };
    assert(mog_arg_present(&none_args, 0) == false);
    PASS();
}

// ============================================================
// Test: Handle Type Safety
// ============================================================

void test_handle_safety(void) {
    printf("\n--- Handle Type Safety ---\n");
    
    TEST("mog_arg_handle with correct type");
    int data = 42;
    MogValue handle_val = mog_handle(&data, "IntPtr");
    MogArgs handle_args = { &handle_val, 1 };
    void *ptr = mog_arg_handle(&handle_args, 0, "IntPtr");
    assert(ptr == &data);
    assert(*(int*)ptr == 42);
    PASS();
    
    // Note: testing wrong handle type would call exit(1).
    // We just verify the correct case works.
    TEST("handle extraction returns correct pointer");
    double dval = 99.9;
    MogValue dhandle = mog_handle(&dval, "DoublePtr");
    MogArgs dargs = { &dhandle, 1 };
    double *dptr = (double *)mog_arg_handle(&dargs, 0, "DoublePtr");
    assert(*dptr == 99.9);
    PASS();
}

// ============================================================
// Test: Capability Registration and Calling
// ============================================================

void test_capability_registration(void) {
    printf("\n--- Capability Registration ---\n");
    
    MogVM *vm = mog_vm_new();
    
    // Register math capability
    MogCapEntry math_entries[] = {
        { "add", math_add },
        { "multiply", math_multiply },
        { "subtract", math_subtract },
        { NULL, NULL }  // sentinel
    };
    
    TEST("register math capability");
    int result = mog_register_capability(vm, "math", math_entries);
    assert(result == 0);
    PASS();
    
    TEST("has_capability - registered");
    assert(mog_has_capability(vm, "math") == true);
    PASS();
    
    TEST("has_capability - not registered");
    assert(mog_has_capability(vm, "fs") == false);
    PASS();
    
    // Register log capability
    MogCapEntry log_entries[] = {
        { "info", log_info },
        { NULL, NULL }
    };
    
    TEST("register log capability");
    result = mog_register_capability(vm, "log", log_entries);
    assert(result == 0);
    PASS();
    
    // Test calling math.add(10, 20)
    TEST("call math.add(10, 20)");
    MogValue add_args[2] = { mog_int(10), mog_int(20) };
    MogValue add_result = mog_cap_call(vm, "math", "add", add_args, 2);
    assert(add_result.tag == MOG_INT);
    assert(add_result.data.i == 30);
    PASS();
    
    // Test calling math.multiply(6, 7)
    TEST("call math.multiply(6, 7)");
    MogValue mul_args[2] = { mog_int(6), mog_int(7) };
    MogValue mul_result = mog_cap_call(vm, "math", "multiply", mul_args, 2);
    assert(mul_result.tag == MOG_INT);
    assert(mul_result.data.i == 42);
    PASS();
    
    // Test calling math.subtract(100, 58)
    TEST("call math.subtract(100, 58)");
    MogValue sub_args[2] = { mog_int(100), mog_int(58) };
    MogValue sub_result = mog_cap_call(vm, "math", "subtract", sub_args, 2);
    assert(sub_result.tag == MOG_INT);
    assert(sub_result.data.i == 42);
    PASS();
    
    // Test calling log.info
    TEST("call log.info('test message')");
    MogValue log_args[1] = { mog_string("test message") };
    MogValue log_result = mog_cap_call(vm, "log", "info", log_args, 1);
    assert(log_result.tag == MOG_NONE);
    assert(strcmp(last_log_message, "test message") == 0);
    PASS();
    
    mog_vm_free(vm);
}

// ============================================================
// Test: Error Cases
// ============================================================

void test_error_cases(void) {
    printf("\n--- Error Cases ---\n");
    
    MogVM *vm = mog_vm_new();
    MogCapEntry math_entries[] = {
        { "add", math_add },
        { NULL, NULL }
    };
    mog_register_capability(vm, "math", math_entries);
    
    // Call non-existent capability
    TEST("call non-existent capability returns error");
    MogValue result = mog_cap_call(vm, "nonexistent", "foo", NULL, 0);
    assert(result.tag == MOG_ERROR);
    assert(strstr(result.data.error, "not registered") != NULL);
    PASS();
    
    // Call non-existent function
    TEST("call non-existent function returns error");
    result = mog_cap_call(vm, "math", "nonexistent", NULL, 0);
    assert(result.tag == MOG_ERROR);
    assert(strstr(result.data.error, "no function") != NULL);
    PASS();
    
    // NULL VM
    TEST("cap_call with NULL VM returns error");
    result = mog_cap_call(NULL, "math", "add", NULL, 0);
    assert(result.tag == MOG_ERROR);
    PASS();
    
    mog_vm_free(vm);
}

// ============================================================
// Test: Capability Validation
// ============================================================

void test_capability_validation(void) {
    printf("\n--- Capability Validation ---\n");
    
    MogVM *vm = mog_vm_new();
    MogCapEntry math_entries[] = {
        { "add", math_add },
        { NULL, NULL }
    };
    mog_register_capability(vm, "math", math_entries);
    
    TEST("validate with all required present");
    const char *required1[] = { "math", NULL };
    assert(mog_validate_capabilities(vm, required1) == 0);
    PASS();
    
    TEST("validate with missing capability");
    const char *required2[] = { "math", "fs", NULL };
    assert(mog_validate_capabilities(vm, required2) == 1);  // 1 missing
    PASS();
    
    TEST("validate with all missing");
    const char *required3[] = { "fs", "net", NULL };
    assert(mog_validate_capabilities(vm, required3) == 2);  // 2 missing
    PASS();
    
    mog_vm_free(vm);
}

// ============================================================
// Test: Resource Limits
// ============================================================

void test_resource_limits(void) {
    printf("\n--- Resource Limits ---\n");
    
    TEST("set resource limits");
    MogVM *vm = mog_vm_new();
    MogLimits limits = { .max_memory = 1024 * 1024, .max_cpu_ms = 5000, .max_stack_depth = 256 };
    mog_vm_set_limits(vm, &limits);
    // Just verify it doesn't crash - limits are stored but enforcement is future work
    mog_vm_free(vm);
    PASS();
}

// ============================================================
// Test: Result Helpers
// ============================================================

void test_result_helpers(void) {
    printf("\n--- Result Helpers ---\n");
    
    TEST("mog_ok_int");
    MogValue v = mog_ok_int(42);
    assert(v.tag == MOG_INT);
    assert(v.data.i == 42);
    PASS();
    
    TEST("mog_ok_float");
    v = mog_ok_float(3.14);
    assert(v.tag == MOG_FLOAT);
    assert(v.data.f == 3.14);
    PASS();
    
    TEST("mog_ok_string");
    v = mog_ok_string("result");
    assert(v.tag == MOG_STRING);
    assert(strcmp(v.data.s, "result") == 0);
    PASS();
}

// ============================================================
// Main
// ============================================================

int main(void) {
    printf("=== Mog Host FFI Integration Tests ===\n");
    
    test_vm_lifecycle();
    test_value_constructors();
    test_arg_extractors();
    test_handle_safety();
    test_capability_registration();
    test_error_cases();
    test_capability_validation();
    test_resource_limits();
    test_result_helpers();
    
    printf("\n=== Results: %d/%d tests passed ===\n", tests_passed, tests_run);
    
    if (tests_passed == tests_run) {
        printf("ALL TESTS PASSED\n");
        return 0;
    } else {
        printf("SOME TESTS FAILED\n");
        return 1;
    }
}
