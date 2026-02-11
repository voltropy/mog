#include "mog.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

#define MAX_CAPABILITIES 32
#define MAX_FUNCTIONS_PER_CAP 64

// Internal representation of a registered capability
typedef struct {
    char name[128];
    MogCapEntry functions[MAX_FUNCTIONS_PER_CAP];
    int func_count;
} MogCapability;

// The VM structure
struct MogVM {
    MogCapability capabilities[MAX_CAPABILITIES];
    int cap_count;
    MogLimits limits;
};

// --- Global VM pointer for embedded programs ---

static MogVM *g_mog_vm = NULL;

void mog_vm_set_global(MogVM *vm) {
    g_mog_vm = vm;
}

MogVM *mog_vm_get_global(void) {
    return g_mog_vm;
}

// --- VM lifecycle ---

MogVM *mog_vm_new(void) {
    MogVM *vm = (MogVM *)calloc(1, sizeof(MogVM));
    if (!vm) {
        fprintf(stderr, "mog_vm_new: allocation failed\n");
        return NULL;
    }
    vm->cap_count = 0;
    vm->limits.max_memory = 0;
    vm->limits.max_cpu_ms = 0;
    vm->limits.max_stack_depth = 1024;
    return vm;
}

void mog_vm_free(MogVM *vm) {
    if (vm) {
        free(vm);
    }
}

// --- Capability registration ---

int mog_register_capability(MogVM *vm, const char *cap_name, const MogCapEntry *entries) {
    if (!vm || !cap_name || !entries) return -1;
    if (vm->cap_count >= MAX_CAPABILITIES) {
        fprintf(stderr, "mog: max capabilities (%d) exceeded\n", MAX_CAPABILITIES);
        return -1;
    }

    MogCapability *cap = &vm->capabilities[vm->cap_count];
    strncpy(cap->name, cap_name, sizeof(cap->name) - 1);
    cap->name[sizeof(cap->name) - 1] = '\0';
    cap->func_count = 0;

    // Copy entries until we hit a NULL sentinel (name == NULL)
    for (int i = 0; entries[i].name != NULL; i++) {
        if (cap->func_count >= MAX_FUNCTIONS_PER_CAP) {
            fprintf(stderr, "mog: max functions per capability (%d) exceeded for '%s'\n",
                    MAX_FUNCTIONS_PER_CAP, cap_name);
            return -1;
        }
        cap->functions[cap->func_count].name = entries[i].name;
        cap->functions[cap->func_count].func = entries[i].func;
        cap->func_count++;
    }

    vm->cap_count++;
    return 0;
}

// --- Capability call wrapper (used by generated LLVM IR code) ---
// Takes an explicit output pointer to avoid ABI issues with large struct returns on ARM64.
// On AAPCS64, structs > 16 bytes use x8 for indirect return, but LLVM IR can't reliably
// match this convention. Using an explicit pointer parameter makes the ABI unambiguous.
void mog_cap_call_out(MogValue *out, MogVM *vm, const char *cap_name, const char *func_name, MogValue *args, int nargs) {
    *out = mog_cap_call(vm, cap_name, func_name, args, nargs);
}

// --- Capability call (internal, also callable from C host code) ---

MogValue mog_cap_call(MogVM *vm, const char *cap_name, const char *func_name, MogValue *args, int nargs) {
    if (!vm) vm = g_mog_vm;  // Fall back to global VM
    if (!vm) return mog_error("mog_cap_call: no VM (pass one or call mog_vm_set_global)");
    if (!cap_name) return mog_error("mog_cap_call: NULL cap_name");
    if (!func_name) return mog_error("mog_cap_call: NULL func_name");

    // Find the capability
    for (int i = 0; i < vm->cap_count; i++) {
        if (strcmp(vm->capabilities[i].name, cap_name) == 0) {
            MogCapability *cap = &vm->capabilities[i];
            // Find the function
            for (int j = 0; j < cap->func_count; j++) {
                if (strcmp(cap->functions[j].name, func_name) == 0) {
                    MogArgs mog_args = { args, nargs };
                    return cap->functions[j].func(vm, &mog_args);
                }
            }
            // Function not found in capability
            static char err_buf[256];
            snprintf(err_buf, sizeof(err_buf), "capability '%s' has no function '%s'", cap_name, func_name);
            return mog_error(err_buf);
        }
    }

    // Capability not found
    static char err_buf[256];
    snprintf(err_buf, sizeof(err_buf), "capability '%s' not registered", cap_name);
    return mog_error(err_buf);
}

// --- Value constructors ---

MogValue mog_int(int64_t value) {
    MogValue v;
    v.tag = MOG_INT;
    v.data.i = value;
    return v;
}

MogValue mog_float(double value) {
    MogValue v;
    v.tag = MOG_FLOAT;
    v.data.f = value;
    return v;
}

MogValue mog_bool(bool value) {
    MogValue v;
    v.tag = MOG_BOOL;
    v.data.b = value;
    return v;
}

MogValue mog_string(const char *s) {
    MogValue v;
    v.tag = MOG_STRING;
    v.data.s = s;
    return v;
}

MogValue mog_none(void) {
    MogValue v;
    v.tag = MOG_NONE;
    v.data.i = 0;
    return v;
}

MogValue mog_error(const char *message) {
    MogValue v;
    v.tag = MOG_ERROR;
    v.data.error = message;
    return v;
}

MogValue mog_handle(void *ptr, const char *type_name) {
    MogValue v;
    v.tag = MOG_HANDLE;
    v.data.handle.ptr = ptr;
    v.data.handle.type_name = type_name;
    return v;
}

// --- Value extractors (abort on mismatch - these are host programming errors) ---

int64_t mog_arg_int(MogArgs *args, int index) {
    if (!args || index < 0 || index >= args->count) {
        fprintf(stderr, "mog_arg_int: index %d out of bounds (count=%d)\n", index, args ? args->count : 0);
        exit(1);
    }
    if (args->values[index].tag != MOG_INT) {
        fprintf(stderr, "mog_arg_int: expected MOG_INT at index %d, got tag %d\n", index, args->values[index].tag);
        exit(1);
    }
    return args->values[index].data.i;
}

double mog_arg_float(MogArgs *args, int index) {
    if (!args || index < 0 || index >= args->count) {
        fprintf(stderr, "mog_arg_float: index %d out of bounds (count=%d)\n", index, args ? args->count : 0);
        exit(1);
    }
    if (args->values[index].tag != MOG_FLOAT) {
        fprintf(stderr, "mog_arg_float: expected MOG_FLOAT at index %d, got tag %d\n", index, args->values[index].tag);
        exit(1);
    }
    return args->values[index].data.f;
}

bool mog_arg_bool(MogArgs *args, int index) {
    if (!args || index < 0 || index >= args->count) {
        fprintf(stderr, "mog_arg_bool: index %d out of bounds (count=%d)\n", index, args ? args->count : 0);
        exit(1);
    }
    if (args->values[index].tag != MOG_BOOL) {
        fprintf(stderr, "mog_arg_bool: expected MOG_BOOL at index %d, got tag %d\n", index, args->values[index].tag);
        exit(1);
    }
    return args->values[index].data.b;
}

const char *mog_arg_string(MogArgs *args, int index) {
    if (!args || index < 0 || index >= args->count) {
        fprintf(stderr, "mog_arg_string: index %d out of bounds (count=%d)\n", index, args ? args->count : 0);
        exit(1);
    }
    if (args->values[index].tag != MOG_STRING) {
        fprintf(stderr, "mog_arg_string: expected MOG_STRING at index %d, got tag %d\n", index, args->values[index].tag);
        exit(1);
    }
    return args->values[index].data.s;
}

void *mog_arg_handle(MogArgs *args, int index, const char *expected_type) {
    if (!args || index < 0 || index >= args->count) {
        fprintf(stderr, "mog_arg_handle: index %d out of bounds (count=%d)\n", index, args ? args->count : 0);
        exit(1);
    }
    if (args->values[index].tag != MOG_HANDLE) {
        fprintf(stderr, "mog_arg_handle: expected MOG_HANDLE at index %d, got tag %d\n", index, args->values[index].tag);
        exit(1);
    }
    // Type safety: strcmp the type_name (like Lua's luaL_checkudata metatable check)
    if (expected_type && args->values[index].data.handle.type_name) {
        if (strcmp(args->values[index].data.handle.type_name, expected_type) != 0) {
            fprintf(stderr, "mog_arg_handle: type mismatch at index %d: expected '%s', got '%s'\n",
                    index, expected_type, args->values[index].data.handle.type_name);
            exit(1);
        }
    }
    return args->values[index].data.handle.ptr;
}

bool mog_arg_present(MogArgs *args, int index) {
    if (!args || index < 0 || index >= args->count) {
        return false;
    }
    return args->values[index].tag != MOG_NONE;
}

// --- Result helpers ---

MogValue mog_ok_int(int64_t value) {
    return mog_int(value);
}

MogValue mog_ok_float(double value) {
    return mog_float(value);
}

MogValue mog_ok_string(const char *s) {
    return mog_string(s);
}

// --- Capability queries ---

bool mog_has_capability(MogVM *vm, const char *cap_name) {
    if (!vm || !cap_name) return false;
    for (int i = 0; i < vm->cap_count; i++) {
        if (strcmp(vm->capabilities[i].name, cap_name) == 0) {
            return true;
        }
    }
    return false;
}

int mog_validate_capabilities(MogVM *vm, const char **required_caps) {
    if (!vm || !required_caps) return -1;
    int missing = 0;
    for (int i = 0; required_caps[i] != NULL; i++) {
        if (!mog_has_capability(vm, required_caps[i])) {
            fprintf(stderr, "mog: missing required capability '%s'\n", required_caps[i]);
            missing++;
        }
    }
    return missing;
}

// --- Resource limits ---

void mog_vm_set_limits(MogVM *vm, const MogLimits *limits) {
    if (!vm || !limits) return;
    vm->limits = *limits;
}
