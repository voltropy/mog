/*
 * Mog Plugin Host Example
 *
 * Demonstrates loading a compiled Mog plugin (.dylib) at runtime and calling
 * its exported functions from C. The math_plugin exports pure functions that
 * require no host capabilities, making this the simplest possible plugin host.
 *
 * Build with build_plugin.sh, then run:
 *   ./plugin_host ./math_plugin.dylib
 */

#include <stdio.h>
#include <stdlib.h>
#include "mog.h"
#include "mog_plugin.h"

int main(int argc, char **argv) {
    /* Create a VM instance. The math plugin has no `requires` declarations
     * so it needs no registered capabilities — an empty VM suffices. */
    MogVM *vm = mog_vm_new();
    if (!vm) {
        fprintf(stderr, "Failed to create MogVM\n");
        return 1;
    }
    mog_vm_set_global(vm);

    /* Load the plugin from the path given on the command line, or a default. */
    const char *plugin_path = argc > 1 ? argv[1] : "./math_plugin.dylib";
    MogPlugin *plugin = mog_load_plugin(plugin_path, vm);
    if (!plugin) {
        fprintf(stderr, "Failed to load plugin: %s\n", mog_plugin_error());
        mog_vm_free(vm);
        return 1;
    }

    /* Print plugin metadata. */
    const MogPluginInfo *info = mog_plugin_get_info(plugin);
    printf("Loaded plugin: %s v%s\n", info->name, info->version);
    printf("Exports: %lld function(s)\n", (long long)info->num_exports);
    if (info->export_names) {
        for (int i = 0; info->export_names[i]; i++) {
            printf("  - %s\n", info->export_names[i]);
        }
    }
    printf("\n");

    /* --- Call exported functions --- */

    /* fibonacci(10) = 55 */
    MogValue fib_args[] = { mog_int(10) };
    MogValue fib_result = mog_plugin_call(plugin, "fibonacci", fib_args, 1);
    printf("fibonacci(10) = %lld\n", fib_result.data.i);

    /* factorial(7) = 5040 */
    MogValue fact_args[] = { mog_int(7) };
    MogValue fact_result = mog_plugin_call(plugin, "factorial", fact_args, 1);
    printf("factorial(7) = %lld\n", fact_result.data.i);

    /* sum_of_squares(3, 4) = 25 */
    MogValue sos_args[] = { mog_int(3), mog_int(4) };
    MogValue sos_result = mog_plugin_call(plugin, "sum_of_squares", sos_args, 2);
    printf("sum_of_squares(3, 4) = %lld\n", sos_result.data.i);

    /* gcd(48, 18) = 6 */
    MogValue gcd_args[] = { mog_int(48), mog_int(18) };
    MogValue gcd_result = mog_plugin_call(plugin, "gcd", gcd_args, 2);
    printf("gcd(48, 18) = %lld\n", gcd_result.data.i);

    /* Attempting to call a function that doesn't exist returns MOG_ERROR. */
    MogValue bad = mog_plugin_call(plugin, "nonexistent", NULL, 0);
    if (bad.tag == MOG_ERROR) {
        printf("\nExpected error: %s\n", bad.data.error);
    }

    /* Cleanup. */
    mog_unload_plugin(plugin);
    mog_vm_free(vm);

    return 0;
}
