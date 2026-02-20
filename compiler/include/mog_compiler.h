/**
 * mog_compiler.h — C interface to the Mog compiler.
 *
 * Link against libmog.dylib / libmog.so / libmog.a to use these functions.
 * All returned pointers that are documented as "caller-owned" must be freed
 * with the corresponding free function to avoid memory leaks.
 */

#ifndef MOG_COMPILER_H
#define MOG_COMPILER_H

#ifdef __cplusplus
extern "C" {
#endif

/* ------------------------------------------------------------------------ */
/* Opaque types                                                             */
/* ------------------------------------------------------------------------ */

/** Opaque compiler instance. */
typedef struct MogCompiler MogCompiler;

/** Opaque compilation result. */
typedef struct MogCompileResult MogCompileResult;

/* ------------------------------------------------------------------------ */
/* Options                                                                  */
/* ------------------------------------------------------------------------ */

/** Options controlling a single compilation.
 *  Pass NULL to mog_compile() for defaults (opt_level=0, no debug, host target). */
typedef struct {
    int  opt_level;    /**< 0 = none, 1 = basic, 2 = full */
    int  debug_info;   /**< Non-zero to emit debug information */
    const char *target; /**< Target triple, or NULL for host default */
} MogCompileOptions;

/* ------------------------------------------------------------------------ */
/* Compiler lifecycle                                                       */
/* ------------------------------------------------------------------------ */

/** Create a new compiler instance.  Returns NULL on failure.
 *  Free with mog_compiler_free(). */
MogCompiler *mog_compiler_new(void);

/** Destroy a compiler instance.  NULL is a safe no-op. */
void mog_compiler_free(MogCompiler *compiler);

/* ------------------------------------------------------------------------ */
/* Compilation                                                              */
/* ------------------------------------------------------------------------ */

/** Compile source code, returning a result handle.
 *
 *  @param compiler  Compiler instance (may be NULL; a temporary is used).
 *  @param source    NUL-terminated UTF-8 source string.
 *  @param options   Compilation options, or NULL for defaults.
 *  @return Caller-owned result; free with mog_result_free().
 *          Returns NULL only if source is NULL or invalid UTF-8. */
MogCompileResult *mog_compile(MogCompiler *compiler,
                              const char  *source,
                              const MogCompileOptions *options);

/** Compile source and return the IR as a C string.
 *  Returns NULL on error.  Free with mog_free_string(). */
char *mog_compile_to_ir(MogCompiler *compiler, const char *source);

/** Compile source and write a binary to output_path.
 *  @return 0 on success, -1 on error. */
int mog_compile_to_binary(MogCompiler *compiler,
                          const char  *source,
                          const char  *output_path);

/** Compile source as a Mog plugin shared library.
 *  @param name     Plugin name (e.g. "my_plugin").
 *  @param version  Semver string (e.g. "1.0.0").
 *  @return 0 on success, -1 on error. */
int mog_compile_plugin(MogCompiler *compiler,
                       const char  *source,
                       const char  *name,
                       const char  *version,
                       const char  *output_path);

/* ------------------------------------------------------------------------ */
/* Result access                                                            */
/* ------------------------------------------------------------------------ */

/** Get the compiled IR string.  Borrowed — valid until mog_result_free().
 *  Returns NULL if compilation failed or result is NULL. */
const char *mog_result_get_ir(const MogCompileResult *result);

/** Number of errors in the result.  Returns 0 if result is NULL. */
int mog_result_get_error_count(const MogCompileResult *result);

/** Get the i-th error message.  Borrowed — valid until mog_result_free().
 *  Returns NULL if out of bounds or result is NULL. */
const char *mog_result_get_error(const MogCompileResult *result, int index);

/** Free a compilation result.  NULL is a safe no-op. */
void mog_result_free(MogCompileResult *result);

/* ------------------------------------------------------------------------ */
/* Memory                                                                   */
/* ------------------------------------------------------------------------ */

/** Free a string returned by mog_compile_to_ir() or similar.
 *  NULL is a safe no-op. */
void mog_free_string(char *s);

#ifdef __cplusplus
}
#endif

#endif /* MOG_COMPILER_H */
