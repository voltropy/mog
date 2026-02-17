#include "all.h"

static void
blit(Ref sd[2], int sz, Fn *fn)
{
	struct { int st, ld, cls, size; } *p, tbl[] = {
		{ Ostorel, Oload,   Kl, 8 },
		{ Ostorew, Oload,   Kw, 4 },
		{ Ostoreh, Oloaduh, Kw, 2 },
		{ Ostoreb, Oloadub, Kw, 1 }
	};
	Ref r, r1, ro;
	int off, fwd, n;

	fwd = sz >= 0;
	sz = abs(sz);
	off = fwd ? sz : 0;
	for (p=tbl; sz; p++)
		for (n=p->size; sz>=n; sz-=n) {
			off -= fwd ? n : 0;
			r = newtmp("blt", Kl, fn);
			r1 = newtmp("blt", Kl, fn);
			ro = getcon(off, fn);
			emit(p->st, 0, R, r, r1);
			emit(Oadd, Kl, r1, sd[1], ro);
			r1 = newtmp("blt", Kl, fn);
			emit(p->ld, p->cls, r, r1, R);
			emit(Oadd, Kl, r1, sd[0], ro);
			off += fwd ? 0 : n;
		}
}

static void
ins(Ins **pi, int *new, Blk *b, Fn *fn)
{
	ulong ni;
	Ins *i;

	i = *pi;
	/* simplify more instructions here;
	 * copy 0 into xor, mul 2^n into shift,
	 * bit rotations, ... */
	switch (i->op) {
	case Oblit1:
		assert(i > b->ins);
		assert((i-1)->op == Oblit0);
		if (!*new) {
			curi = &insb[NIns];
			ni = &b->ins[b->nins] - (i+1);
			curi -= ni;
			icpy(curi, i+1, ni);
			*new = 1;
		}
		blit((i-1)->arg, rsval(i->arg[0]), fn);
		*pi = i-1;
		break;
	default:
		if (*new)
			emiti(*i);
		break;
	}
}

void
simpl(Fn *fn)
{
	Blk *b;
	Ins *i;
	int new;

	for (b=fn->start; b; b=b->link) {
		new = 0;
		for (i=&b->ins[b->nins]; i!=b->ins;) {
			--i;
			ins(&i, &new, b, fn);
		}
		if (new) {
			b->nins = &insb[NIns] - curi;
			idup(&b->ins, curi, b->nins);
		}
	}
}
