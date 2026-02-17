#include "all.h"

/* eliminate sub-word abi op
 * variants for targets that
 * treat char/short/... as
 * words with arbitrary high
 * bits
 */
void
elimsb(Fn *fn)
{
	Blk *b;
	Ins *i;

	for (b=fn->start; b; b=b->link) {
		for (i=b->ins; i<&b->ins[b->nins]; i++) {
			if (isargbh(i->op))
				i->op = Oarg;
			if (isparbh(i->op))
				i->op = Opar;
		}
		if (isretbh(b->jmp.type))
			b->jmp.type = Jretw;
	}
}
