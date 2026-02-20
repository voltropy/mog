#ifndef MOG_PLUGIN_H
#define MOG_PLUGIN_H

#include "mog.h"

// Plugin info structure - returned by mog_plugin_info() in the plugin
typedef struct {
    const char *name;           // plugin name (e.g. "my_plugin")
    const char *version;        // semver string (e.g. "1.0.0")
    const char **required_caps; // NULL-terminated array of required capability names
    int64_t num_exports;        // number of exported functions
    const char **export_names;  // NULL-terminated array of exported function names
} MogPluginInfo;

// Plugin function entry - maps name to function pointer
typedef struct {
    const char *name;
    void *func_ptr;  // actual function pointer (cast by caller)
} MogPluginExport;

// Plugin handle (opaque)
typedef struct MogPlugin MogPlugin;

// Load a compiled Mog plugin from a .dylib/.so file
// Returns NULL on failure (check mog_plugin_error())
// The VM must be set up with all capabilities the plugin requires
MogPlugin *mog_load_plugin(const char *path, MogVM *vm);

// Load with explicit capability allowlist
// allowed_caps is NULL-terminated array of capability names the host permits
// Returns NULL if plugin requires a capability not in the allowlist
MogPlugin *mog_load_plugin_sandboxed(const char *path, MogVM *vm, const char **allowed_caps);

// Call an exported plugin function by name
// Returns mog_error() if function not found
MogValue mog_plugin_call(MogPlugin *plugin, const char *func_name, MogValue *args, int nargs);

// Get plugin info
const MogPluginInfo *mog_plugin_get_info(MogPlugin *plugin);

// Get the last plugin error message (or NULL)
const char *mog_plugin_error(void);

// Unload a plugin (dlclose)
void mog_unload_plugin(MogPlugin *plugin);

#endif
