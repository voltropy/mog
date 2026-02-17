.POSIX:
.SUFFIXES: .o .c

PREFIX = /usr/local
BINDIR = $(PREFIX)/bin

COMMOBJ  = main.o util.o parse.o abi.o cfg.o mem.o ssa.o alias.o load.o \
           copy.o fold.o simpl.o live.o spill.o rega.o emit.o
AMD64OBJ = amd64/targ.o amd64/sysv.o amd64/isel.o amd64/emit.o
ARM64OBJ = arm64/targ.o arm64/abi.o arm64/isel.o arm64/emit.o
RV64OBJ  = rv64/targ.o rv64/abi.o rv64/isel.o rv64/emit.o
OBJ      = $(COMMOBJ) $(AMD64OBJ) $(ARM64OBJ) $(RV64OBJ)

SRCALL   = $(OBJ:.o=.c)

CC       = cc
CFLAGS   = -std=c99 -g -Wall -Wextra -Wpedantic

qbe: $(OBJ)
	$(CC) $(LDFLAGS) $(OBJ) -o $@

.c.o:
	$(CC) $(CFLAGS) -c $< -o $@

$(OBJ): all.h ops.h
$(AMD64OBJ): amd64/all.h
$(ARM64OBJ): arm64/all.h
$(RV64OBJ): rv64/all.h
main.o: config.h

config.h:
	@case `uname` in                               \
	*Darwin*)                                      \
		case `uname -m` in                     \
		*arm64*)                               \
			echo "#define Deftgt T_arm64_apple";\
			;;                             \
		*)                                     \
			echo "#define Deftgt T_amd64_apple";\
			;;                             \
		esac                                   \
		;;                                     \
	*)                                             \
		case `uname -m` in                     \
		*aarch64*|*arm64*)                     \
			echo "#define Deftgt T_arm64"; \
			;;                             \
		*riscv64*)                             \
			echo "#define Deftgt T_rv64";  \
			;;                             \
		*)                                     \
			echo "#define Deftgt T_amd64_sysv";\
			;;                             \
		esac                                   \
		;;                                     \
	esac > $@

install: qbe
	mkdir -p "$(DESTDIR)$(BINDIR)"
	install -m755 qbe "$(DESTDIR)$(BINDIR)/qbe"

uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/qbe"

clean:
	rm -f *.o */*.o qbe

clean-gen: clean
	rm -f config.h

check: qbe
	tools/test.sh all

check-arm64: qbe
	TARGET=arm64 tools/test.sh all

check-rv64: qbe
	TARGET=rv64 tools/test.sh all

src:
	@echo $(SRCALL)

80:
	@for F in $(SRCALL);                       \
	do                                         \
		awk "{                             \
			gsub(/\\t/, \"        \"); \
			if (length(\$$0) > $@)     \
				printf(\"$$F:%d: %s\\n\", NR, \$$0); \
		}" < $$F;                          \
	done

.PHONY: clean clean-gen check check-arm64 src 80 install uninstall
