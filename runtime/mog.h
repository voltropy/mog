#ifndef MOG_H
#define MOG_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

// Opaque types
typedef struct MogVM MogVM;

// MogValue is a tagged union for passing values across the boundary
typedef struct {
    enum { MOG_INT, MOG_FLOAT, MOG_BOOL, MOG_STRING, MOG_NONE, MOG_HANDLE, MOG_ERROR } tag;
    union {
        int64_t     i;
        double      f;
        bool        b;
        const char *s;
        struct { void *ptr; const char *type_name; } handle;
        const char *error;
    } data;
} MogValue;

// Args accessor - wraps the raw argument array
typedef struct {
    MogValue *values;
    int count;
} MogArgs;

// Host function signature (like Lua's lua_CFunction)
typedef MogValue (*MogHostFn)(MogVM *vm, MogArgs *args);

// Capability entry (like Lua's luaL_Reg)
typedef struct {
    const char *name;
    MogHostFn   func;
} MogCapEntry;

// VM lifecycle
MogVM *mog_vm_new(void);
void   mog_vm_free(MogVM *vm);

// Global VM pointer for embedded programs
void    mog_vm_set_global(MogVM *vm);
MogVM  *mog_vm_get_global(void);

// Capability registration (like luaL_newlib)
int  mog_register_capability(MogVM *vm, const char *cap_name, const MogCapEntry *entries);

// Call a registered capability function (internal, also callable from C host code)
MogValue mog_cap_call(MogVM *vm, const char *cap_name, const char *func_name, MogValue *args, int nargs);

// Wrapper with explicit output pointer (used by generated LLVM IR code to avoid ARM64 ABI issues)
void mog_cap_call_out(MogValue *out, MogVM *vm, const char *cap_name, const char *func_name, MogValue *args, int nargs);

// Value constructors
MogValue mog_int(int64_t value);
MogValue mog_float(double value);
MogValue mog_bool(bool value);
MogValue mog_string(const char *s);
MogValue mog_none(void);
MogValue mog_error(const char *message);
MogValue mog_handle(void *ptr, const char *type_name);

// Value extractors (with type checking - abort on mismatch)
int64_t     mog_arg_int(MogArgs *args, int index);
double      mog_arg_float(MogArgs *args, int index);
bool        mog_arg_bool(MogArgs *args, int index);
const char *mog_arg_string(MogArgs *args, int index);
void       *mog_arg_handle(MogArgs *args, int index, const char *expected_type);
bool        mog_arg_present(MogArgs *args, int index); // for optional params

// Result helpers
MogValue mog_ok_int(int64_t value);
MogValue mog_ok_float(double value);
MogValue mog_ok_string(const char *s);

// Check if a capability is registered
bool mog_has_capability(MogVM *vm, const char *cap_name);

// Validate that all required capabilities are present
// required_caps is a NULL-terminated array of capability names
int mog_validate_capabilities(MogVM *vm, const char **required_caps);

// Resource limits
typedef struct {
    size_t max_memory;      // 0 = unlimited
    int    max_cpu_ms;      // 0 = unlimited
    int    max_stack_depth; // 0 = default (1024)
} MogLimits;
void mog_vm_set_limits(MogVM *vm, const MogLimits *limits);

#endif // MOG_H
