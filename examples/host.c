/*
 * Mog Host Embedding Example
 *
 * This C program acts as the "host" for a Mog program. It:
 * 1. Creates a MogVM and registers the "env" capability
 * 2. Sets the global VM pointer (so generated code can find it)
 * 3. The Mog-generated @main runs automatically, calling program_user()
 * 4. program_user() calls env.* functions which route through mog_cap_call
 *
 * Uses __attribute__((constructor)) so the VM is ready before main() runs.
 */

#include "mog.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* ============================================================
 * Host function implementations for the "env" capability
 * ============================================================ */

static MogValue host_env_get_name(MogVM *vm, MogArgs *args) {
    (void)vm; (void)args;
    return mog_string("MogShowcase");
}

static MogValue host_env_get_version(MogVM *vm, MogArgs *args) {
    (void)vm; (void)args;
    return mog_int(1);
}

static MogValue host_env_timestamp(MogVM *vm, MogArgs *args) {
    (void)vm; (void)args;
    return mog_int((int64_t)time(NULL));
}

static MogValue host_env_random(MogVM *vm, MogArgs *args) {
    (void)vm;
    int64_t min_val = mog_arg_int(args, 0);
    int64_t max_val = mog_arg_int(args, 1);
    if (max_val <= min_val) return mog_int(min_val);
    int64_t range = max_val - min_val + 1;
    int64_t result = min_val + (rand() % range);
    return mog_int(result);
}

static MogValue host_env_log(MogVM *vm, MogArgs *args) {
    (void)vm;
    const char *message = mog_arg_string(args, 0);
    printf("%s\n", message);
    return mog_none();
}

/* ============================================================
 * Capability registration table
 * ============================================================ */

static const MogCapEntry env_functions[] = {
    { "get_name",    host_env_get_name    },
    { "get_version", host_env_get_version },
    { "timestamp",   host_env_timestamp   },
    { "random",      host_env_random      },
    { "log",         host_env_log         },
    { NULL, NULL }  /* sentinel */
};

/* ============================================================
 * Constructor: runs before main() to set up the VM
 * ============================================================ */

__attribute__((constructor))
static void setup_mog_vm(void) {
    /* Seed random number generator */
    srand((unsigned int)time(NULL));

    /* Create and configure the VM */
    MogVM *vm = mog_vm_new();
    if (!vm) {
        fprintf(stderr, "host: failed to create MogVM\n");
        exit(1);
    }

    /* Register the "env" capability */
    if (mog_register_capability(vm, "env", env_functions) != 0) {
        fprintf(stderr, "host: failed to register 'env' capability\n");
        mog_vm_free(vm);
        exit(1);
    }

    /* Make it available globally for generated code */
    mog_vm_set_global(vm);
}
