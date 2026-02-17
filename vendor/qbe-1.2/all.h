#include <assert.h>
#include <inttypes.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define MAKESURE(what, x) typedef char make_sure_##what[(x)?1:-1]
#define die(...) die_(__FILE__, __VA_ARGS__)

typedef unsigned char uchar;
typedef unsigned int uint;
typedef unsigned long ulong;
typedef unsigned long long bits;

typedef struct BSet BSet;
typedef struct Ref Ref;
typedef struct Op Op;
typedef struct Ins Ins;
typedef struct Phi Phi;
typedef struct Blk Blk;
typedef struct Use Use;
typedef struct Sym Sym;
typedef struct Alias Alias;
typedef struct Tmp Tmp;
typedef struct Con Con;
typedef struct Addr Mem;
typedef struct Fn Fn;
typedef struct Typ Typ;
typedef struct Field Field;
typedef struct Dat Dat;
typedef struct Lnk Lnk;
typedef struct Target Target;

enum {
	NString = 80,
	NIns    = 1 << 20,
	NAlign  = 3,
	NField  = 32,
	NBit    = CHAR_BIT * sizeof(bits),
};

struct Target {
	char name[16];
	char apple;
	int gpr0;   /* first general purpose reg */
	int ngpr;
	int fpr0;   /* first floating point reg */
	int nfpr;
	bits rglob; /* globally live regs (e.g., sp, fp) */
	int nrglob;
	int *rsave; /* caller-save */
	int nrsave[2];
	bits (*retregs)(Ref, int[2]);
	bits (*argregs)(Ref, int[2]);
	int (*memargs)(int);
	void (*abi0)(Fn *);
	void (*abi1)(Fn *);
	void (*isel)(Fn *);
	void (*emitfn)(Fn *, FILE *);
	void (*emitfin)(FILE *);
	char asloc[4];
	char assym[4];
};

#define BIT(n) ((bits)1 << (n))

enum {
	RXX = 0,
	Tmp0 = NBit, /* first non-reg temporary */
};

struct BSet {
	uint nt;
	bits *t;
};

struct Ref {
	uint type:3;
	uint val:29;
};

enum {
	RTmp,
	RCon,
	RInt,
	RType, /* last kind to come out of the parser */
	RSlot,
	RCall,
	RMem,
};

#define R        (Ref){RTmp, 0}
#define UNDEF    (Ref){RCon, 0}  /* represents uninitialized data */
#define CON_Z    (Ref){RCon, 1}
#define TMP(x)   (Ref){RTmp, x}
#define CON(x)   (Ref){RCon, x}
#define SLOT(x)  (Ref){RSlot, (x)&0x1fffffff}
#define TYPE(x)  (Ref){RType, x}
#define CALL(x)  (Ref){RCall, x}
#define MEM(x)   (Ref){RMem, x}
#define INT(x)   (Ref){RInt, (x)&0x1fffffff}

static inline int req(Ref a, Ref b)
{
	return a.type == b.type && a.val == b.val;
}

static inline int rtype(Ref r)
{
	if (req(r, R))
		return -1;
	return r.type;
}

static inline int rsval(Ref r)
{
	return (int32_t)((int64_t)r.val << 3) >> 3;
}

enum CmpI {
	Cieq,
	Cine,
	Cisge,
	Cisgt,
	Cisle,
	Cislt,
	Ciuge,
	Ciugt,
	Ciule,
	Ciult,
	NCmpI,
};

enum CmpF {
	Cfeq,
	Cfge,
	Cfgt,
	Cfle,
	Cflt,
	Cfne,
	Cfo,
	Cfuo,
	NCmpF,
	NCmp = NCmpI + NCmpF,
};

enum O {
	Oxxx,
#define O(op, x, y) O##op,
	#include "ops.h"
	NOp,
};

enum J {
	Jxxx,
#define JMPS(X)                                 \
	X(retw)   X(retl)   X(rets)   X(retd)   \
	X(retsb)  X(retub)  X(retsh)  X(retuh)  \
	X(retc)   X(ret0)   X(jmp)    X(jnz)    \
	X(jfieq)  X(jfine)  X(jfisge) X(jfisgt) \
	X(jfisle) X(jfislt) X(jfiuge) X(jfiugt) \
	X(jfiule) X(jfiult) X(jffeq)  X(jffge)  \
	X(jffgt)  X(jffle)  X(jfflt)  X(jffne)  \
	X(jffo)   X(jffuo)  X(hlt)
#define X(j) J##j,
	JMPS(X)
#undef X
	NJmp
};

enum {
	Ocmpw = Oceqw,
	Ocmpw1 = Ocultw,
	Ocmpl = Oceql,
	Ocmpl1 = Ocultl,
	Ocmps = Oceqs,
	Ocmps1 = Ocuos,
	Ocmpd = Oceqd,
	Ocmpd1 = Ocuod,
	Oalloc = Oalloc4,
	Oalloc1 = Oalloc16,
	Oflag = Oflagieq,
	Oflag1 = Oflagfuo,
	NPubOp = Onop,
	Jjf = Jjfieq,
	Jjf1 = Jjffuo,
};

#define INRANGE(x, l, u) ((unsigned)(x) - l <= u - l) /* linear in x */
#define isstore(o) INRANGE(o, Ostoreb, Ostored)
#define isload(o) INRANGE(o, Oloadsb, Oload)
#define isext(o) INRANGE(o, Oextsb, Oextuw)
#define ispar(o) INRANGE(o, Opar, Opare)
#define isarg(o) INRANGE(o, Oarg, Oargv)
#define isret(j) INRANGE(j, Jretw, Jret0)
#define isparbh(o) INRANGE(o, Oparsb, Oparuh)
#define isargbh(o) INRANGE(o, Oargsb, Oarguh)
#define isretbh(j) INRANGE(j, Jretsb, Jretuh)

enum {
	Kx = -1, /* "top" class (see usecheck() and clsmerge()) */
	Kw,
	Kl,
	Ks,
	Kd
};

#define KWIDE(k) ((k)&1)
#define KBASE(k) ((k)>>1)

struct Op {
	char *name;
	short argcls[2][4];
	int canfold;
};

struct Ins {
	uint op:30;
	uint cls:2;
	Ref to;
	Ref arg[2];
};

struct Phi {
	Ref to;
	Ref *arg;
	Blk **blk;
	uint narg;
	int cls;
	Phi *link;
};

struct Blk {
	Phi *phi;
	Ins *ins;
	uint nins;
	struct {
		short type;
		Ref arg;
	} jmp;
	Blk *s1;
	Blk *s2;
	Blk *link;

	uint id;
	uint visit;

	Blk *idom;
	Blk *dom, *dlink;
	Blk **fron;
	uint nfron;

	Blk **pred;
	uint npred;
	BSet in[1], out[1], gen[1];
	int nlive[2];
	int loop;
	char name[NString];
};

struct Use {
	enum {
		UXXX,
		UPhi,
		UIns,
		UJmp,
	} type;
	uint bid;
	union {
		Ins *ins;
		Phi *phi;
	} u;
};

struct Sym {
	enum {
		SGlo,
		SThr,
	} type;
	uint32_t id;
};

enum {
	NoAlias,
	MayAlias,
	MustAlias
};

struct Alias {
	enum {
		ABot = 0,
		ALoc = 1, /* stack local */
		ACon = 2,
		AEsc = 3, /* stack escaping */
		ASym = 4,
		AUnk = 6,
	#define astack(t) ((t) & 1)
	} type;
	int base;
	int64_t offset;
	union {
		Sym sym;
		struct {
			int sz; /* -1 if > NBit */
			bits m;
		} loc;
	} u;
	Alias *slot;
};

struct Tmp {
	char name[NString];
	Ins *def;
	Use *use;
	uint ndef, nuse;
	uint bid; /* id of a defining block */
	uint cost;
	int slot; /* -1 for unset */
	short cls;
	struct {
		int r;  /* register or -1 */
		int w;  /* weight */
		bits m; /* avoid these registers */
	} hint;
	int phi;
	Alias alias;
	enum {
		WFull,
		Wsb, /* must match Oload/Oext order */
		Wub,
		Wsh,
		Wuh,
		Wsw,
		Wuw
	} width;
	int visit;
};

struct Con {
	enum {
		CUndef,
		CBits,
		CAddr,
	} type;
	Sym sym;
	union {
		int64_t i;
		double d;
		float s;
	} bits;
	char flt; /* 1 to print as s, 2 to print as d */
};

typedef struct Addr Addr;

struct Addr { /* amd64 addressing */
	Con offset;
	Ref base;
	Ref index;
	int scale;
};

struct Lnk {
	char export;
	char thread;
	char align;
	char *sec;
	char *secf;
};

struct Fn {
	Blk *start;
	Tmp *tmp;
	Con *con;
	Mem *mem;
	int ntmp;
	int ncon;
	int nmem;
	uint nblk;
	int retty; /* index in typ[], -1 if no aggregate return */
	Ref retr;
	Blk **rpo;
	bits reg;
	int slot;
	char vararg;
	char dynalloc;
	char name[NString];
	Lnk lnk;
};

struct Typ {
	char name[NString];
	char isdark;
	char isunion;
	int align;
	uint64_t size;
	uint nunion;
	struct Field {
		enum {
			FEnd,
			Fb,
			Fh,
			Fw,
			Fl,
			Fs,
			Fd,
			FPad,
			FTyp,
		} type;
		uint len; /* or index in typ[] for FTyp */
	} (*fields)[NField+1];
};

struct Dat {
	enum {
		DStart,
		DEnd,
		DB,
		DH,
		DW,
		DL,
		DZ
	} type;
	char *name;
	Lnk *lnk;
	union {
		int64_t num;
		double fltd;
		float flts;
		char *str;
		struct {
			char *name;
			int64_t off;
		} ref;
	} u;
	char isref;
	char isstr;
};

/* main.c */
extern Target T;
extern char debug['Z'+1];

/* util.c */
typedef enum {
	PHeap, /* free() necessary */
	PFn, /* discarded after processing the function */
} Pool;

extern Typ *typ;
extern Ins insb[NIns], *curi;
uint32_t hash(char *);
void die_(char *, char *, ...) __attribute__((noreturn));
void *emalloc(size_t);
void *alloc(size_t);
void freeall(void);
void *vnew(ulong, size_t, Pool);
void vfree(void *);
void vgrow(void *, ulong);
void strf(char[NString], char *, ...);
uint32_t intern(char *);
char *str(uint32_t);
int argcls(Ins *, int);
int isreg(Ref);
int iscmp(int, int *, int *);
void emit(int, int, Ref, Ref, Ref);
void emiti(Ins);
void idup(Ins **, Ins *, ulong);
Ins *icpy(Ins *, Ins *, ulong);
int cmpop(int);
int cmpneg(int);
int clsmerge(short *, short);
int phicls(int, Tmp *);
Ref newtmp(char *, int, Fn *);
void chuse(Ref, int, Fn *);
int symeq(Sym, Sym);
Ref newcon(Con *, Fn *);
Ref getcon(int64_t, Fn *);
int addcon(Con *, Con *);
void salloc(Ref, Ref, Fn *);
void dumpts(BSet *, Tmp *, FILE *);

void bsinit(BSet *, uint);
void bszero(BSet *);
uint bscount(BSet *);
void bsset(BSet *, uint);
void bsclr(BSet *, uint);
void bscopy(BSet *, BSet *);
void bsunion(BSet *, BSet *);
void bsinter(BSet *, BSet *);
void bsdiff(BSet *, BSet *);
int bsequal(BSet *, BSet *);
int bsiter(BSet *, int *);

static inline int
bshas(BSet *bs, uint elt)
{
	assert(elt < bs->nt * NBit);
	return (bs->t[elt/NBit] & BIT(elt%NBit)) != 0;
}

/* parse.c */
extern Op optab[NOp];
void parse(FILE *, char *, void (char *), void (Dat *), void (Fn *));
void printfn(Fn *, FILE *);
void printref(Ref, Fn *, FILE *);
void err(char *, ...) __attribute__((noreturn));

/* abi.c */
void elimsb(Fn *);

/* cfg.c */
Blk *newblk(void);
void edgedel(Blk *, Blk **);
void fillpreds(Fn *);
void fillrpo(Fn *);
void filldom(Fn *);
int sdom(Blk *, Blk *);
int dom(Blk *, Blk *);
void fillfron(Fn *);
void loopiter(Fn *, void (*)(Blk *, Blk *));
void fillloop(Fn *);
void simpljmp(Fn *);

/* mem.c */
void promote(Fn *);
void coalesce(Fn *);

/* alias.c */
void fillalias(Fn *);
void getalias(Alias *, Ref, Fn *);
int alias(Ref, int, int, Ref, int, int *, Fn *);
int escapes(Ref, Fn *);

/* load.c */
int loadsz(Ins *);
int storesz(Ins *);
void loadopt(Fn *);

/* ssa.c */
void filluse(Fn *);
void fillpreds(Fn *);
void fillrpo(Fn *);
void ssa(Fn *);
void ssacheck(Fn *);

/* copy.c */
void copy(Fn *);

/* fold.c */
void fold(Fn *);

/* simpl.c */
void simpl(Fn *);

/* live.c */
void liveon(BSet *, Blk *, Blk *);
void filllive(Fn *);

/* spill.c */
void fillcost(Fn *);
void spill(Fn *);

/* rega.c */
void rega(Fn *);

/* emit.c */
void emitfnlnk(char *, Lnk *, FILE *);
void emitdat(Dat *, FILE *);
void emitdbgfile(char *, FILE *);
void emitdbgloc(uint, uint, FILE *);
int stashbits(void *, int);
void elf_emitfnfin(char *, FILE *);
void elf_emitfin(FILE *);
void macho_emitfin(FILE *);
