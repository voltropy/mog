#include "mog_plugin.h"
#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdarg.h>

// Plugin structure
struct MogPlugin {
    void *dl_handle;           // dlopen handle
    MogPluginInfo info;        // cached info
    MogPluginExport *exports;  // cached export table
    int export_count;
    char path[512];            // path to the loaded library
};

// Thread-local error message
static __thread char g_plugin_error[1024] = {0};

static void set_plugin_error(const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    vsnprintf(g_plugin_error, sizeof(g_plugin_error), fmt, args);
    va_end(args);
}

const char *mog_plugin_error(void) {
    return g_plugin_error[0] ? g_plugin_error : NULL;
}

// Load plugin implementation
MogPlugin *mog_load_plugin(const char *path, MogVM *vm) {
    return mog_load_plugin_sandboxed(path, vm, NULL);
}

MogPlugin *mog_load_plugin_sandboxed(const char *path, MogVM *vm, const char **allowed_caps) {
    g_plugin_error[0] = 0;

    // Open the shared library
    void *handle = dlopen(path, RTLD_NOW | RTLD_LOCAL);
    if (!handle) {
        set_plugin_error("dlopen failed: %s", dlerror());
        return NULL;
    }

    // Look up required symbols
    typedef const MogPluginInfo *(*InfoFn)(void);
    typedef int (*InitFn)(MogVM *);
    typedef MogPluginExport *(*ExportsFn)(int *);

    InfoFn info_fn = (InfoFn)dlsym(handle, "mog_plugin_info");
    if (!info_fn) {
        set_plugin_error("plugin missing mog_plugin_info symbol");
        dlclose(handle);
        return NULL;
    }

    InitFn init_fn = (InitFn)dlsym(handle, "mog_plugin_init");
    if (!init_fn) {
        set_plugin_error("plugin missing mog_plugin_init symbol");
        dlclose(handle);
        return NULL;
    }

    ExportsFn exports_fn = (ExportsFn)dlsym(handle, "mog_plugin_exports");
    if (!exports_fn) {
        set_plugin_error("plugin missing mog_plugin_exports symbol");
        dlclose(handle);
        return NULL;
    }

    // Get plugin info (returned as pointer to static data in the plugin)
    const MogPluginInfo *info_ptr = info_fn();
    if (!info_ptr) {
        set_plugin_error("mog_plugin_info returned NULL");
        dlclose(handle);
        return NULL;
    }

    // Validate required capabilities
    if (info_ptr->required_caps) {
        for (int i = 0; info_ptr->required_caps[i]; i++) {
            const char *cap = info_ptr->required_caps[i];

            // Check against allowlist if provided
            if (allowed_caps) {
                int allowed = 0;
                for (int j = 0; allowed_caps[j]; j++) {
                    if (strcmp(allowed_caps[j], cap) == 0) {
                        allowed = 1;
                        break;
                    }
                }
                if (!allowed) {
                    set_plugin_error("plugin requires capability '%s' which is not in the allowlist", cap);
                    dlclose(handle);
                    return NULL;
                }
            }

            // Check that the VM actually has the capability registered
            if (!mog_has_capability(vm, cap)) {
                set_plugin_error("plugin requires capability '%s' which is not registered", cap);
                dlclose(handle);
                return NULL;
            }
        }
    }

    // Get exports
    int export_count = 0;
    MogPluginExport *exports = exports_fn(&export_count);

    // Allocate plugin structure
    MogPlugin *plugin = (MogPlugin *)calloc(1, sizeof(MogPlugin));
    plugin->dl_handle = handle;
    plugin->info = *info_ptr;
    plugin->exports = exports;
    plugin->export_count = export_count;
    strncpy(plugin->path, path, sizeof(plugin->path) - 1);

    // Initialize the plugin (sets up GC frame, runs top-level code)
    int result = init_fn(vm);
    if (result != 0) {
        set_plugin_error("plugin init returned error code %d", result);
        dlclose(handle);
        free(plugin);
        return NULL;
    }

    return plugin;
}

MogValue mog_plugin_call(MogPlugin *plugin, const char *func_name, MogValue *args, int nargs) {
    if (!plugin) {
        return mog_error("null plugin");
    }

    // Find the export
    for (int i = 0; i < plugin->export_count; i++) {
        if (strcmp(plugin->exports[i].name, func_name) == 0) {
            // Call the function
            // Exported Mog functions take i64 args and return i64
            // We need to convert MogValue to i64 and back
            typedef int64_t (*MogExportFn)();
            MogExportFn fn = (MogExportFn)plugin->exports[i].func_ptr;

            // For now, pass raw i64 values extracted from MogValue
            // This matches how Mog functions work internally (everything is i64/ptr)
            int64_t raw_args[16];
            for (int j = 0; j < nargs && j < 16; j++) {
                switch (args[j].tag) {
                    case MOG_INT: raw_args[j] = args[j].data.i; break;
                    case MOG_FLOAT: {
                        // Pack double bits into i64
                        double d = args[j].data.f;
                        memcpy(&raw_args[j], &d, sizeof(double));
                        break;
                    }
                    case MOG_STRING: raw_args[j] = (int64_t)(intptr_t)args[j].data.s; break;
                    case MOG_BOOL: raw_args[j] = args[j].data.b ? 1 : 0; break;
                    case MOG_HANDLE: raw_args[j] = (int64_t)(intptr_t)args[j].data.handle.ptr; break;
                    default: raw_args[j] = 0; break;
                }
            }

            // Call with appropriate number of args using switch
            int64_t result;
            switch (nargs) {
                case 0: result = ((int64_t(*)())fn)(); break;
                case 1: result = ((int64_t(*)(int64_t))fn)(raw_args[0]); break;
                case 2: result = ((int64_t(*)(int64_t,int64_t))fn)(raw_args[0], raw_args[1]); break;
                case 3: result = ((int64_t(*)(int64_t,int64_t,int64_t))fn)(raw_args[0], raw_args[1], raw_args[2]); break;
                case 4: result = ((int64_t(*)(int64_t,int64_t,int64_t,int64_t))fn)(raw_args[0], raw_args[1], raw_args[2], raw_args[3]); break;
                default:
                    return mog_error("too many arguments (max 4)");
            }

            return mog_int(result);
        }
    }

    set_plugin_error("function '%s' not found in plugin '%s'", func_name, plugin->info.name);
    return mog_error(g_plugin_error);
}

const MogPluginInfo *mog_plugin_get_info(MogPlugin *plugin) {
    return plugin ? &plugin->info : NULL;
}

void mog_unload_plugin(MogPlugin *plugin) {
    if (!plugin) return;
    if (plugin->dl_handle) {
        dlclose(plugin->dl_handle);
    }
    free(plugin);
}
