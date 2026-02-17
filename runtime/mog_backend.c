#include "all.h"
#include "mog_backend.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <spawn.h>
#include <sys/wait.h>

/* QBE globals that we must define (normally in main.c) */
Target T;
char debug['Z'+1] = {0};

/* Target declarations (normally in main.c) */
extern Target T_arm64_apple;

/* Module-level output stream for callbacks */
static FILE *g_outf;

extern char **environ;

/*
 * QBE parse callbacks — mirrors main.c's func/data/dbgfile
 */

static void
cb_dbgfile(char *fn)
{
	emitdbgfile(fn, g_outf);
}

static void
cb_data(Dat *d)
{
	emitdat(d, g_outf);
	if (d->type == DEnd)
		freeall();
}

static void
cb_func(Fn *fn)
{
	uint n;

	T.abi0(fn);
	fillrpo(fn);
	fillpreds(fn);
	filluse(fn);
	promote(fn);
	filluse(fn);
	ssa(fn);
	filluse(fn);
	ssacheck(fn);
	fillalias(fn);
	loadopt(fn);
	filluse(fn);
	fillalias(fn);
	coalesce(fn);
	filluse(fn);
	ssacheck(fn);
	copy(fn);
	filluse(fn);
	fold(fn);
	T.abi1(fn);
	simpl(fn);
	fillpreds(fn);
	filluse(fn);
	T.isel(fn);
	fillrpo(fn);
	filllive(fn);
	fillloop(fn);
	fillcost(fn);
	spill(fn);
	rega(fn);
	fillrpo(fn);
	simpljmp(fn);
	fillpreds(fn);
	fillrpo(fn);
	assert(fn->rpo[0] == fn->start);
	for (n=0;; n++)
		if (n == fn->nblk-1) {
			fn->rpo[n]->link = 0;
			break;
		} else
			fn->rpo[n]->link = fn->rpo[n+1];
	T.emitfn(fn, g_outf);
	freeall();
}

/*
 * mog_qbe_compile — compile QBE IL string to assembly string
 */
char*
mog_qbe_compile(const char* qbe_il, int qbe_il_len)
{
	FILE *inf;
	char *asm_buf = NULL;
	size_t asm_size = 0;

	if (!qbe_il || qbe_il_len <= 0)
		return NULL;

	/* Set target to arm64 Apple (macOS) */
	T = T_arm64_apple;

	/* Open input from memory */
	inf = fmemopen((void*)qbe_il, (size_t)qbe_il_len, "r");
	if (!inf)
		return NULL;

	/* Open output to memory */
	g_outf = open_memstream(&asm_buf, &asm_size);
	if (!g_outf) {
		fclose(inf);
		return NULL;
	}

	/* Run the QBE pipeline */
	parse(inf, "<memory>", cb_dbgfile, cb_data, cb_func);
	T.emitfin(g_outf);

	/* Flush and close */
	fclose(g_outf);
	fclose(inf);
	g_outf = NULL;

	return asm_buf;
}

/*
 * Helper: write string to a temporary file, return the path.
 * Caller must free the returned path.
 */
static char*
write_temp_file(const char *suffix, const char *content, size_t len)
{
	char template[256];
	int fd;
	FILE *f;
	char *path;

	snprintf(template, sizeof(template), "/tmp/mog_XXXXXX%s", suffix);
	fd = mkstemps(template, (int)strlen(suffix));
	if (fd < 0)
		return NULL;

	f = fdopen(fd, "w");
	if (!f) {
		close(fd);
		unlink(template);
		return NULL;
	}

	if (fwrite(content, 1, len, f) != len) {
		fclose(f);
		unlink(template);
		return NULL;
	}
	fclose(f);

	path = strdup(template);
	return path;
}

/*
 * Helper: run a command via posix_spawn, wait for it.
 * Returns the exit status (0 on success).
 */
static int
run_command(const char *path, char *const argv[])
{
	pid_t pid;
	int status;

	if (posix_spawn(&pid, path, NULL, NULL, argv, environ) != 0)
		return -1;

	if (waitpid(pid, &status, 0) < 0)
		return -1;

	if (WIFEXITED(status))
		return WEXITSTATUS(status);

	return -1;
}

/*
 * mog_assemble — QBE IL string -> .o file on disk
 */
int
mog_assemble(const char* qbe_il, int qbe_il_len, const char* output_obj_path)
{
	char *asm_str;
	char *asm_path;
	int ret;

	/* Compile QBE IL to assembly */
	asm_str = mog_qbe_compile(qbe_il, qbe_il_len);
	if (!asm_str)
		return 1;

	/* Write assembly to temp file */
	asm_path = write_temp_file(".s", asm_str, strlen(asm_str));
	free(asm_str);
	if (!asm_path)
		return 2;

	/* Assemble with `as` */
	{
		char *argv[] = {
			"as", "-o", (char*)output_obj_path, asm_path, NULL
		};
		ret = run_command("/usr/bin/as", argv);
	}

	/* Clean up temp file */
	unlink(asm_path);
	free(asm_path);

	return ret;
}

/*
 * mog_compile_and_link — QBE IL -> linked binary
 */
int
mog_compile_and_link(const char* qbe_il, int qbe_il_len,
                     const char* output_path,
                     const char** extra_objects, int num_extra)
{
	char obj_template[] = "/tmp/mog_link_XXXXXX.o";
	char *obj_path;
	int fd, ret;
	const char *sdk_path;

	/* Create temp .o path */
	fd = mkstemps(obj_template, 2); /* ".o" is 2 chars */
	if (fd < 0)
		return 1;
	close(fd);
	obj_path = obj_template;

	/* Assemble to .o */
	ret = mog_assemble(qbe_il, qbe_il_len, obj_path);
	if (ret != 0) {
		unlink(obj_path);
		return ret;
	}

	/* Get SDK path from environment or use default */
	sdk_path = getenv("SDKROOT");
	if (!sdk_path)
		sdk_path = "/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk";

	/* Link with ld */
	{
		/* Build argument list:
		 * ld -o <output> <obj> [extra_objects...] -lSystem -lm
		 *    -syslibroot <sdk> -arch arm64
		 */
		int argc = 0;
		int max_args = 12 + num_extra;
		char **argv = calloc((size_t)(max_args + 1), sizeof(char*));
		if (!argv) {
			unlink(obj_path);
			return 3;
		}

		argv[argc++] = "ld";
		argv[argc++] = "-o";
		argv[argc++] = (char*)output_path;
		argv[argc++] = obj_path;

		/* Add extra object files */
		for (int i = 0; i < num_extra; i++)
			argv[argc++] = (char*)extra_objects[i];

		argv[argc++] = "-lSystem";
		argv[argc++] = "-lm";
		argv[argc++] = "-syslibroot";
		argv[argc++] = (char*)sdk_path;
		argv[argc++] = "-arch";
		argv[argc++] = "arm64";
		argv[argc] = NULL;

		ret = run_command("/usr/bin/ld", argv);
		free(argv);
	}

	/* Clean up temp object file */
	unlink(obj_path);

	return ret;
}
