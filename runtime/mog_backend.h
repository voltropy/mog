#ifndef MOG_BACKEND_H
#define MOG_BACKEND_H

#ifdef __cplusplus
extern "C" {
#endif

/*
 * mog_backend.h — C bridge to QBE compiler backend
 *
 * Provides functions to compile QBE IL strings to assembly,
 * object files, and linked binaries.
 */

/* Takes QBE IL as a string, returns assembly as a string (caller must free).
 * Returns NULL on error. */
char* mog_qbe_compile(const char* qbe_il, int qbe_il_len);

/* Full pipeline: QBE IL string -> object file on disk.
 * Uses mog_qbe_compile() internally, then spawns `as` to assemble.
 * Returns 0 on success, non-zero on error. */
int mog_assemble(const char* qbe_il, int qbe_il_len, const char* output_obj_path);

/* Full pipeline: QBE IL string -> linked binary on disk.
 * Does QBE + as + ld in sequence.
 * extra_objects is an array of additional .o or .a files to link.
 * Returns 0 on success, non-zero on error. */
int mog_compile_and_link(const char* qbe_il, int qbe_il_len,
                         const char* output_path,
                         const char** extra_objects, int num_extra);

#ifdef __cplusplus
}
#endif

#endif /* MOG_BACKEND_H */
