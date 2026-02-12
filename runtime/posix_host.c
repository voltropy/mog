/*
 * posix_host.c — Default POSIX host providing fs + process capabilities.
 *
 * This is an optional file. Programs that need file I/O or process control
 * link against this + mog_async.c. Programs that only need the base language
 * (math, strings, arrays, structs, closures) don't need this file at all.
 *
 * Compile: clang -c posix_host.c -o posix_host.o
 * Link:    ... posix_host.o mog_async.o ...
 */

#include "mog.h"
#include "mog_async.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <time.h>

/* ---- fs capability ---- */

static MogValue fs_read_file(MogVM *vm, MogArgs *args) {
    if (args->count < 1) return mog_error("fs.read_file: missing path");
    const char *path = mog_as_string(args->values[0]);
    if (!path) return mog_error("fs.read_file: path must be a string");

    FILE *f = fopen(path, "rb");
    if (!f) return mog_error("fs.read_file: cannot open file");

    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);

    char *buf = (char *)malloc(len + 1);
    if (!buf) { fclose(f); return mog_error("fs.read_file: out of memory"); }

    size_t nread = fread(buf, 1, len, f);
    buf[nread] = '\0';
    fclose(f);

    /* Note: caller is responsible for freeing this string.
       In practice, the GC string system would manage this. */
    MogValue result = mog_string(buf);
    /* Don't free buf — the string value holds a pointer to it.
       This leaks in the current implementation. A proper fix would
       integrate with the GC's string table. */
    return result;
}

static MogValue fs_write_file(MogVM *vm, MogArgs *args) {
    if (args->count < 2) return mog_error("fs.write_file: missing args");
    const char *path = mog_as_string(args->values[0]);
    const char *contents = mog_as_string(args->values[1]);
    if (!path || !contents) return mog_error("fs.write_file: invalid args");

    FILE *f = fopen(path, "wb");
    if (!f) return mog_int(-1);

    size_t len = strlen(contents);
    size_t written = fwrite(contents, 1, len, f);
    fclose(f);

    return mog_int((int64_t)written);
}

static MogValue fs_append_file(MogVM *vm, MogArgs *args) {
    if (args->count < 2) return mog_error("fs.append_file: missing args");
    const char *path = mog_as_string(args->values[0]);
    const char *contents = mog_as_string(args->values[1]);
    if (!path || !contents) return mog_error("fs.append_file: invalid args");

    FILE *f = fopen(path, "ab");
    if (!f) return mog_int(-1);

    size_t len = strlen(contents);
    size_t written = fwrite(contents, 1, len, f);
    fclose(f);

    return mog_int((int64_t)written);
}

static MogValue fs_exists(MogVM *vm, MogArgs *args) {
    if (args->count < 1) return mog_error("fs.exists: missing path");
    const char *path = mog_as_string(args->values[0]);
    if (!path) return mog_error("fs.exists: path must be a string");

    return mog_bool(access(path, F_OK) == 0);
}

static MogValue fs_remove(MogVM *vm, MogArgs *args) {
    if (args->count < 1) return mog_error("fs.remove: missing path");
    const char *path = mog_as_string(args->values[0]);
    if (!path) return mog_error("fs.remove: path must be a string");

    return mog_int(remove(path) == 0 ? 0 : -1);
}

static MogValue fs_file_size(MogVM *vm, MogArgs *args) {
    if (args->count < 1) return mog_error("fs.file_size: missing path");
    const char *path = mog_as_string(args->values[0]);
    if (!path) return mog_error("fs.file_size: path must be a string");

    struct stat st;
    if (stat(path, &st) != 0) return mog_int(-1);

    return mog_int((int64_t)st.st_size);
}

/* ---- process capability ---- */

static MogValue process_sleep(MogVM *vm, MogArgs *args) {
    if (args->count < 1) return mog_error("process.sleep: missing ms");
    int64_t ms = mog_as_int(args->values[0]);

    MogEventLoop *loop = mog_loop_get_global();
    if (loop) {
        /* Truly async: create future, add timer */
        MogFuture *future = mog_future_new();
        mog_loop_add_timer(loop, (uint64_t)ms, future);
        /* Return the future pointer as an int for now
           (proper integration would use the coroutine await mechanism) */
        return mog_int((int64_t)(intptr_t)future);
    } else {
        /* No event loop: synchronous sleep */
        struct timespec ts;
        ts.tv_sec = ms / 1000;
        ts.tv_nsec = (ms % 1000) * 1000000L;
        nanosleep(&ts, NULL);
        return mog_int(0);
    }
}

static MogValue process_getenv(MogVM *vm, MogArgs *args) {
    if (args->count < 1) return mog_error("process.getenv: missing name");
    const char *name = mog_as_string(args->values[0]);
    if (!name) return mog_error("process.getenv: name must be a string");

    const char *val = getenv(name);
    if (!val) return mog_string("");

    return mog_string(val);
}

static MogValue process_cwd(MogVM *vm, MogArgs *args) {
    char buf[4096];
    if (getcwd(buf, sizeof(buf))) {
        return mog_string(buf);
    }
    return mog_error("process.cwd: getcwd failed");
}

static MogValue process_exit(MogVM *vm, MogArgs *args) {
    int64_t code = (args->count > 0) ? mog_as_int(args->values[0]) : 0;
    exit((int)code);
    return mog_int(0); /* unreachable */
}

static MogValue process_timestamp(MogVM *vm, MogArgs *args) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    int64_t ms = (int64_t)ts.tv_sec * 1000 + (int64_t)ts.tv_nsec / 1000000;
    return mog_int(ms);
}

/* ---- Registration ---- */

static MogCapEntry fs_entries[] = {
    { "read_file",  fs_read_file },
    { "write_file", fs_write_file },
    { "append_file",fs_append_file },
    { "exists",     fs_exists },
    { "remove",     fs_remove },
    { "file_size",  fs_file_size },
    { NULL, NULL }
};

static MogCapEntry process_entries[] = {
    { "sleep",     process_sleep },
    { "timestamp", process_timestamp },
    { "getenv",    process_getenv },
    { "cwd",       process_cwd },
    { "exit",      process_exit },
    { NULL, NULL }
};

void mog_register_posix_host(MogVM *vm) {
    mog_register_capability(vm, "fs", fs_entries);
    mog_register_capability(vm, "process", process_entries);
}

/* Auto-register when linked as part of a standard Mog program */
__attribute__((constructor))
static void _mog_posix_host_init(void) {
    MogVM *vm = mog_vm_get_global();
    if (vm) {
        mog_register_posix_host(vm);
    }
}
