#include "all.h"

typedef struct Range Range;
typedef struct Store Store;
typedef struct Slot Slot;

/* require use, maintains use counts */
void
promote(Fn *fn)
{
	Blk *b;
	Ins *i, *l;
	Tmp *t;
	Use *u, *ue;
	int s, k;

	/* promote uniform stack slots to temporaries */
	b = fn->start;
	for (i=b->ins; i<&b->ins[b->nins]; i++) {
		if (Oalloc > i->op || i->op > Oalloc1)
			continue;
		/* specific to NAlign == 3 */
		assert(rtype(i->to) == RTmp);
		t = &fn->tmp[i->to.val];
		if (t->ndef != 1)
			goto Skip;
		k = -1;
		s = -1;
		for (u=t->use; u<&t->use[t->nuse]; u++) {
			if (u->type != UIns)
				goto Skip;
			l = u->u.ins;
			if (isload(l->op))
			if (s == -1 || s == loadsz(l)) {
				s = loadsz(l);
				continue;
			}
			if (isstore(l->op))
			if (req(i->to, l->arg[1]) && !req(i->to, l->arg[0]))
			if (s == -1 || s == storesz(l))
			if (k == -1 || k == optab[l->op].argcls[0][0]) {
				s = storesz(l);
				k = optab[l->op].argcls[0][0];
				continue;
			}
			goto Skip;
		}
		/* get rid of the alloc and replace uses */
		*i = (Ins){.op = Onop};
		t->ndef--;
		ue = &t->use[t->nuse];
		for (u=t->use; u!=ue; u++) {
			l = u->u.ins;
			if (isstore(l->op)) {
				l->cls = k;
				l->op = Ocopy;
				l->to = l->arg[1];
				l->arg[1] = R;
				t->nuse--;
				t->ndef++;
			} else {
				if (k == -1)
					err("slot %%%s is read but never stored to",
						fn->tmp[l->arg[0].val].name);
				/* try to turn loads into copies so we
				 * can eliminate them later */
				switch(l->op) {
				case Oloadsw:
				case Oloaduw:
					if (k == Kl)
						goto Extend;
					/* fall through */
				case Oload:
					if (KBASE(k) != KBASE(l->cls))
						l->op = Ocast;
					else
						l->op = Ocopy;
					break;
				default:
				Extend:
					l->op = Oextsb + (l->op - Oloadsb);
					break;
				}
			}
		}
	Skip:;
	}
	if (debug['M']) {
		fprintf(stderr, "\n> After slot promotion:\n");
		printfn(fn, stderr);
	}
}

/* [a, b) with 0 <= a */
struct Range {
	int a, b;
};

struct Store {
	int ip;
	Ins *i;
};

struct Slot {
	int t;
	int sz;
	bits m;
	bits l;
	Range r;
	Slot *s;
	Store *st;
	int nst;
};

static inline int
rin(Range r, int n)
{
	return r.a <= n && n < r.b;
}

static inline int
rovlap(Range r0, Range r1)
{
	return r0.b && r1.b && r0.a < r1.b && r1.a < r0.b;
}

static void
radd(Range *r, int n)
{
	if (!r->b)
		*r = (Range){n, n+1};
	else if (n < r->a)
		r->a = n;
	else if (n >= r->b)
		r->b = n+1;
}

static int
slot(Slot **ps, int64_t *off, Ref r, Fn *fn, Slot *sl)
{
	Alias a;
	Tmp *t;

	getalias(&a, r, fn);
	if (a.type != ALoc)
		return 0;
	t = &fn->tmp[a.base];
	if (t->visit < 0)
		return 0;
	*off = a.offset;
	*ps = &sl[t->visit];
	return 1;
}

static void
load(Ref r, bits x, int ip, Fn *fn, Slot *sl)
{
	int64_t off;
	Slot *s;

	if (slot(&s, &off, r, fn, sl)) {
		s->l |= x << off;
		s->l &= s->m;
		if (s->l)
			radd(&s->r, ip);
	}
}

static void
store(Ref r, bits x, int ip, Ins *i, Fn *fn, Slot *sl)
{
	int64_t off;
	Slot *s;

	if (slot(&s, &off, r, fn, sl)) {
		if (s->l) {
			radd(&s->r, ip);
			s->l &= ~(x << off);
		} else {
			vgrow(&s->st, ++s->nst);
			s->st[s->nst-1].ip = ip;
			s->st[s->nst-1].i = i;
		}
	}
}

static int
scmp(const void *pa, const void *pb)
{
	Slot *a, *b;

	a = (Slot *)pa, b = (Slot *)pb;
	if (a->sz != b->sz)
		return b->sz - a->sz;
	return a->r.a - b->r.a;
}

static void
maxrpo(Blk *hd, Blk *b)
{
	if (hd->loop < (int)b->id)
		hd->loop = b->id;
}

void
coalesce(Fn *fn)
{
	Range r, *br;
	Slot *s, *s0, *sl;
	Blk *b, **ps, *succ[3];
	Ins *i, **bl;
	Use *u;
	Tmp *t, *ts;
	Ref *arg;
	bits x;
	int64_t off0, off1;
	int n, m, ip, sz, nsl, nbl, *stk;
	uint total, freed, fused;

	/* minimize the stack usage
	 * by coalescing slots
	 */
	nsl = 0;
	sl = vnew(0, sizeof sl[0], PHeap);
	for (n=Tmp0; n<fn->ntmp; n++) {
		t = &fn->tmp[n];
		t->visit = -1;
		if (t->alias.type == ALoc)
		if (t->alias.slot == &t->alias)
		if (t->bid == fn->start->id)
		if (t->alias.u.loc.sz != -1) {
			t->visit = nsl;
			vgrow(&sl, ++nsl);
			s = &sl[nsl-1];
			s->t = n;
			s->sz = t->alias.u.loc.sz;
			s->m = t->alias.u.loc.m;
			s->s = 0;
			s->st = vnew(0, sizeof s->st[0], PHeap);
			s->nst = 0;
		}
	}

	/* one-pass liveness analysis */
	for (b=fn->start; b; b=b->link)
		b->loop = -1;
	loopiter(fn, maxrpo);
	nbl = 0;
	bl = vnew(0, sizeof bl[0], PHeap);
	br = emalloc(fn->nblk * sizeof br[0]);
	ip = INT_MAX - 1;
	for (n=fn->nblk-1; n>=0; n--) {
		b = fn->rpo[n];
		succ[0] = b->s1;
		succ[1] = b->s2;
		succ[2] = 0;
		br[n].b = ip--;
		for (s=sl; s<&sl[nsl]; s++) {
			s->l = 0;
			for (ps=succ; *ps; ps++) {
				m = (*ps)->id;
				if (m > n && rin(s->r, br[m].a)) {
					s->l = s->m;
					radd(&s->r, ip);
				}
			}
		}
		if (b->jmp.type == Jretc)
			load(b->jmp.arg, -1, --ip, fn, sl);
		for (i=&b->ins[b->nins]; i!=b->ins;) {
			--i;
			arg = i->arg;
			if (i->op == Oargc) {
				load(arg[1], -1, --ip, fn, sl);
			}
			if (isload(i->op)) {
				x = BIT(loadsz(i)) - 1;
				load(arg[0], x, --ip, fn, sl);
			}
			if (isstore(i->op)) {
				x = BIT(storesz(i)) - 1;
				store(arg[1], x, ip--, i, fn, sl);
			}
			if (i->op == Oblit0) {
				assert((i+1)->op == Oblit1);
				assert(rtype((i+1)->arg[0]) == RInt);
				sz = abs(rsval((i+1)->arg[0]));
				x = sz >= NBit ? (bits)-1 : BIT(sz) - 1;
				store(arg[1], x, ip--, i, fn, sl);
				load(arg[0], x, ip, fn, sl);
				vgrow(&bl, ++nbl);
				bl[nbl-1] = i;
			}
		}
		for (s=sl; s<&sl[nsl]; s++)
			if (s->l) {
				radd(&s->r, ip);
				if (b->loop != -1) {
					assert(b->loop > n);
					radd(&s->r, br[b->loop].b - 1);
				}
			}
		br[n].a = ip;
	}
	free(br);

	/* kill dead stores */
	for (s=sl; s<&sl[nsl]; s++)
		for (n=0; n<s->nst; n++)
			if (!rin(s->r, s->st[n].ip)) {
				i = s->st[n].i;
				if (i->op == Oblit0)
					*(i+1) = (Ins){.op = Onop};
				*i = (Ins){.op = Onop};
			}

	/* kill slots with an empty live range */
	total = 0;
	freed = 0;
	stk = vnew(0, sizeof stk[0], PHeap);
	n = 0;
	for (s=s0=sl; s<&sl[nsl]; s++) {
		total += s->sz;
		if (!s->r.b) {
			vfree(s->st);
			vgrow(&stk, ++n);
			stk[n-1] = s->t;
			freed += s->sz;
		} else
			*s0++ = *s;
	}
	nsl = s0-sl;
	if (debug['M']) {
		fputs("\n> Slot coalescing:\n", stderr);
		if (n) {
			fputs("\tkill [", stderr);
			for (m=0; m<n; m++)
				fprintf(stderr, " %%%s",
					fn->tmp[stk[m]].name);
			fputs(" ]\n", stderr);
		}
	}
	while (n--) {
		t = &fn->tmp[stk[n]];
		assert(t->ndef == 1 && t->def);
		i = t->def;
		if (isload(i->op)) {
			i->op = Ocopy;
			i->arg[0] = UNDEF;
			continue;
		}
		*i = (Ins){.op = Onop};
		for (u=t->use; u<&t->use[t->nuse]; u++) {
			if (u->type == UJmp) {
				b = fn->rpo[u->bid];
				assert(isret(b->jmp.type));
				b->jmp.type = Jret0;
				b->jmp.arg = R;
				continue;
			}
			assert(u->type == UIns);
			i = u->u.ins;
			if (!req(i->to, R)) {
				assert(rtype(i->to) == RTmp);
				vgrow(&stk, ++n);
				stk[n-1] = i->to.val;
			} else if (isarg(i->op)) {
				assert(i->op == Oargc);
				i->arg[1] = CON_Z;  /* crash */
			} else {
				if (i->op == Oblit0)
					*(i+1) = (Ins){.op = Onop};
				*i = (Ins){.op = Onop};
			}
		}
	}
	vfree(stk);

	/* fuse slots by decreasing size */
	qsort(sl, nsl, sizeof *sl, scmp);
	fused = 0;
	for (n=0; n<nsl; n++) {
		s0 = &sl[n];
		if (s0->s)
			continue;
		s0->s = s0;
		r = s0->r;
		for (s=s0+1; s<&sl[nsl]; s++) {
			if (s->s || !s->r.b)
				goto Skip;
			if (rovlap(r, s->r))
				/* O(n); can be approximated
				 * by 'goto Skip;' if need be
				 */
				for (m=n; &sl[m]<s; m++)
					if (sl[m].s == s0)
					if (rovlap(sl[m].r, s->r))
						goto Skip;
			radd(&r, s->r.a);
			radd(&r, s->r.b - 1);
			s->s = s0;
			fused += s->sz;
		Skip:;
		}
	}

	/* substitute fused slots */
	for (s=sl; s<&sl[nsl]; s++) {
		t = &fn->tmp[s->t];
		/* the visit link is stale,
		 * reset it before the slot()
		 * calls below
		 */
		t->visit = s-sl;
		assert(t->ndef == 1 && t->def);
		if (s->s == s)
			continue;
		*t->def = (Ins){.op = Onop};
		ts = &fn->tmp[s->s->t];
		assert(t->bid == ts->bid);
		if (t->def < ts->def) {
			/* make sure the slot we
			 * selected has a def that
			 * dominates its new uses
			 */
			*t->def = *ts->def;
			*ts->def = (Ins){.op = Onop};
			ts->def = t->def;
		}
		for (u=t->use; u<&t->use[t->nuse]; u++) {
			if (u->type == UJmp) {
				b = fn->rpo[u->bid];
				b->jmp.arg = TMP(s->s->t);
				continue;
			}
			assert(u->type == UIns);
			arg = u->u.ins->arg;
			for (n=0; n<2; n++)
				if (req(arg[n], TMP(s->t)))
					arg[n] = TMP(s->s->t);
		}
	}

	/* fix newly overlapping blits */
	for (n=0; n<nbl; n++) {
		i = bl[n];
		if (i->op == Oblit0)
		if (slot(&s, &off0, i->arg[0], fn, sl))
		if (slot(&s0, &off1, i->arg[1], fn, sl))
		if (s->s == s0->s && off0 < off1) {
			sz = rsval((i+1)->arg[0]);
			assert(sz >= 0);
			(i+1)->arg[0] = INT(-sz);
		}
	}
	vfree(bl);

	if (debug['M']) {
		for (s0=sl; s0<&sl[nsl]; s0++) {
			if (s0->s != s0)
				continue;
			fprintf(stderr, "\tfuse (% 3db) [", s0->sz);
			for (s=s0; s<&sl[nsl]; s++) {
				if (s->s != s0)
					continue;
				fprintf(stderr, " %%%s", fn->tmp[s->t].name);
				if (s->r.b)
					fprintf(stderr, "[%d,%d)",
						s->r.a-ip, s->r.b-ip);
				else
					fputs("{}", stderr);
			}
			fputs(" ]\n", stderr);
		}
		fprintf(stderr, "\tsums %u/%u/%u (killed/fused/total)\n\n",
			freed, fused, total);
		printfn(fn, stderr);
	}

	for (s=sl; s<&sl[nsl]; s++)
		vfree(s->st);
	vfree(sl);
}
