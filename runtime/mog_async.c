/*
 * mog_async.c — Mog async runtime: futures, event loop, coroutine scheduling.
 *
 * All allocations use malloc/free, NOT gc_alloc, to stay independent of
 * the GC and avoid problems with LLVM coroutine frames.
 */

#include "mog_async.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <time.h>
#include <sys/select.h>
#include <unistd.h>
#include <errno.h>

#ifdef __APPLE__
#include <mach/mach_time.h>
#endif

/* ---- Monotonic clock helper ---- */
static uint64_t now_ns(void) {
#ifdef __APPLE__
    static mach_timebase_info_data_t tb;
    if (tb.denom == 0) mach_timebase_info(&tb);
    return mach_absolute_time() * tb.numer / tb.denom;
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
#endif
}

static void sleep_ns(uint64_t ns) {
    struct timespec ts;
    ts.tv_sec  = (time_t)(ns / 1000000000ULL);
    ts.tv_nsec = (long)(ns % 1000000000ULL);
    nanosleep(&ts, NULL);
}

/* ---- Coroutine resume via function pointer at frame start ---- */
typedef void (*CoroResumeFn)(void *);

void mog_coro_resume(void *handle) {
    if (!handle) return;
    CoroResumeFn fn = *(CoroResumeFn *)handle;
    fn(handle);
}

/* ---- Global event loop ---- */
static MogEventLoop *g_loop = NULL;

void mog_loop_set_global(MogEventLoop *loop) { g_loop = loop; }
MogEventLoop *mog_loop_get_global(void)       { return g_loop; }

/* ---- Future ---- */

MogFuture *mog_future_new(void) {
    MogFuture *f = (MogFuture *)calloc(1, sizeof(MogFuture));
    if (!f) { fprintf(stderr, "mog: out of memory (future)\n"); exit(1); }
    f->state = MOG_FUTURE_PENDING;
    f->coro_frame = NULL;
    if (g_loop) g_loop->pending_count++;
    return f;
}

void mog_future_complete(MogFuture *future, int64_t value) {
    if (!future || future->state != MOG_FUTURE_PENDING) return;
    future->state  = MOG_FUTURE_READY;
    future->result = value;
    if (g_loop) g_loop->pending_count--;

    /* If a coroutine is waiting, schedule it for resumption */
    if (future->coro_handle) {
        mog_loop_enqueue_ready(g_loop ? g_loop : NULL, future);
    }

    /* If this future belongs to a parent combinator (all/race) */
    if (future->parent) {
        MogFuture *p = future->parent;
        if (p->state == MOG_FUTURE_PENDING) {
            if (p->sub_futures) {
                /* all() combinator */
                p->sub_done++;
                /* Store result at the index of this sub-future */
                for (int i = 0; i < p->sub_count; i++) {
                    if (p->sub_futures[i] == future) {
                        if (p->sub_results) p->sub_results[i] = value;
                        break;
                    }
                }
                if (p->sub_done >= p->sub_count) {
                    /* All sub-futures done — complete parent with pointer to results array */
                    mog_future_complete(p, (int64_t)(intptr_t)p->sub_results);
                }
            } else {
                /* race() combinator — first to finish wins */
                mog_future_complete(p, value);
            }
        }
    }
}

void mog_future_set_error(MogFuture *future, int64_t err) {
    if (!future || future->state != MOG_FUTURE_PENDING) return;
    future->state  = MOG_FUTURE_ERROR;
    future->result = err;
    if (g_loop) g_loop->pending_count--;
    if (future->coro_handle) {
        mog_loop_enqueue_ready(g_loop, future);
    }
}

int mog_future_is_ready(MogFuture *future) {
    return future && future->state != MOG_FUTURE_PENDING;
}

int64_t mog_future_get_result(MogFuture *future) {
    if (!future) return 0;
    return future->result;
}

void mog_future_set_waiter(MogFuture *future, void *coro_handle) {
    if (future) future->coro_handle = coro_handle;
}

void mog_future_free(MogFuture *future) {
    if (!future) return;
    if (future->coro_frame) { free(future->coro_frame); future->coro_frame = NULL; }
    if (future->sub_futures) free(future->sub_futures);
    if (future->sub_results) free(future->sub_results);
    free(future);
}

/* ---- Event Loop ---- */

MogEventLoop *mog_loop_new(void) {
    MogEventLoop *loop = (MogEventLoop *)calloc(1, sizeof(MogEventLoop));
    if (!loop) { fprintf(stderr, "mog: out of memory (event loop)\n"); exit(1); }
    return loop;
}

void mog_loop_free(MogEventLoop *loop) {
    if (!loop) return;
    /* Free remaining timers */
    MogTimer *t = loop->timers;
    while (t) {
        MogTimer *next = t->next;
        free(t);
        t = next;
    }
    /* Free remaining fd watchers */
    MogFdWatcher *w = loop->watchers;
    while (w) {
        MogFdWatcher *next = w->next;
        free(w);
        w = next;
    }
    free(loop);
}

void mog_loop_enqueue_ready(MogEventLoop *loop, MogFuture *future) {
    if (!loop || !future) return;
    future->next = NULL;
    if (loop->ready_tail) {
        loop->ready_tail->next = future;
    } else {
        loop->ready_head = future;
    }
    loop->ready_tail = future;
}

void mog_loop_add_timer(MogEventLoop *loop, uint64_t delay_ms, MogFuture *future) {
    if (!loop || !future) return;
    MogTimer *timer = (MogTimer *)calloc(1, sizeof(MogTimer));
    if (!timer) { fprintf(stderr, "mog: out of memory (timer)\n"); exit(1); }
    timer->deadline_ns = now_ns() + delay_ms * 1000000ULL;
    timer->future = future;
    timer->result_value = 0;

    /* Insert sorted by deadline */
    if (!loop->timers || timer->deadline_ns < loop->timers->deadline_ns) {
        timer->next = loop->timers;
        loop->timers = timer;
    } else {
        MogTimer *cur = loop->timers;
        while (cur->next && cur->next->deadline_ns <= timer->deadline_ns) {
            cur = cur->next;
        }
        timer->next = cur->next;
        cur->next = timer;
    }
}

void mog_loop_add_timer_with_value(MogEventLoop *loop, uint64_t delay_ms, MogFuture *future, int64_t value) {
    if (!loop || !future) return;
    MogTimer *timer = (MogTimer *)calloc(1, sizeof(MogTimer));
    if (!timer) { fprintf(stderr, "mog: out of memory (timer)\n"); exit(1); }
    timer->deadline_ns = now_ns() + delay_ms * 1000000ULL;
    timer->future = future;
    timer->result_value = value;

    /* Insert sorted by deadline */
    if (!loop->timers || timer->deadline_ns < loop->timers->deadline_ns) {
        timer->next = loop->timers;
        loop->timers = timer;
    } else {
        MogTimer *cur = loop->timers;
        while (cur->next && cur->next->deadline_ns <= timer->deadline_ns) {
            cur = cur->next;
        }
        timer->next = cur->next;
        cur->next = timer;
    }
}

/* ---- FD Watcher ---- */

void mog_loop_add_fd_watcher(MogEventLoop *loop, int fd, int events, MogFuture *future) {
    if (!loop || !future) return;
    MogFdWatcher *w = (MogFdWatcher *)calloc(1, sizeof(MogFdWatcher));
    if (!w) { fprintf(stderr, "mog: out of memory (fd watcher)\n"); exit(1); }
    w->fd = fd;
    w->events = events;
    w->future = future;
    w->next = loop->watchers;
    loop->watchers = w;
}

void mog_loop_run(MogEventLoop *loop) {
    if (!loop) return;
    loop->running = 1;

    while (loop->running) {
        /* 1. Fire expired timers */
        uint64_t t = now_ns();
        while (loop->timers && loop->timers->deadline_ns <= t) {
            MogTimer *timer = loop->timers;
            loop->timers = timer->next;
            mog_future_complete(timer->future, timer->result_value);
            free(timer);
        }

        /* 2. Process ready queue */
        int processed = 0;
        while (loop->ready_head) {
            MogFuture *f = loop->ready_head;
            loop->ready_head = f->next;
            if (!loop->ready_head) loop->ready_tail = NULL;
            f->next = NULL;

            if (f->coro_handle) {
                void *hdl = f->coro_handle;
                f->coro_handle = NULL;
                mog_coro_resume(hdl);
                processed++;
            }
        }

        /* 3. Check if anything remains */
        if (!loop->timers && !loop->ready_head && !loop->watchers && loop->pending_count <= 0) {
            break;  /* All work done */
        }

        /* 4. Use select() to wait for fd events AND/OR timer deadlines */
        if (!loop->ready_head && (loop->timers || loop->watchers)) {
            fd_set read_fds, write_fds;
            FD_ZERO(&read_fds);
            FD_ZERO(&write_fds);
            int max_fd = -1;

            /* Add all fd watchers */
            for (MogFdWatcher *w = loop->watchers; w; w = w->next) {
                if (w->events & MOG_FD_READ) FD_SET(w->fd, &read_fds);
                if (w->events & MOG_FD_WRITE) FD_SET(w->fd, &write_fds);
                if (w->fd > max_fd) max_fd = w->fd;
            }

            /* Calculate timeout from next timer */
            struct timeval tv;
            struct timeval *tv_ptr = NULL;
            if (loop->timers) {
                uint64_t tnow = now_ns();
                if (loop->timers->deadline_ns > tnow) {
                    uint64_t wait_ns = loop->timers->deadline_ns - tnow;
                    tv.tv_sec = (long)(wait_ns / 1000000000ULL);
                    tv.tv_usec = (long)((wait_ns % 1000000000ULL) / 1000ULL);
                } else {
                    tv.tv_sec = 0;
                    tv.tv_usec = 0;
                }
                tv_ptr = &tv;
            } else if (loop->watchers && max_fd >= 0) {
                /* No timers but have fd watchers — block indefinitely on select */
                tv_ptr = NULL;
            } else {
                /* Nothing to wait on — poll */
                tv.tv_sec = 0;
                tv.tv_usec = 10000; /* 10ms poll */
                tv_ptr = &tv;
            }

            if (max_fd >= 0) {
                int nready = select(max_fd + 1, &read_fds, &write_fds, NULL, tv_ptr);
                if (nready > 0) {
                    /* Check which fd watchers fired */
                    MogFdWatcher **prev_ptr = &loop->watchers;
                    MogFdWatcher *w = loop->watchers;
                    while (w) {
                        MogFdWatcher *next = w->next;
                        int fired = 0;
                        if ((w->events & MOG_FD_READ) && FD_ISSET(w->fd, &read_fds)) fired = 1;
                        if ((w->events & MOG_FD_WRITE) && FD_ISSET(w->fd, &write_fds)) fired = 1;

                        if (fired) {
                            /* Remove from list and complete future */
                            *prev_ptr = next;
                            /* Read the data for stdin watchers */
                            if (w->fd == STDIN_FILENO && (w->events & MOG_FD_READ)) {
                                /* Read a line from stdin */
                                char buf[4096];
                                if (fgets(buf, sizeof(buf), stdin)) {
                                    /* Remove trailing newline */
                                    size_t len = strlen(buf);
                                    if (len > 0 && buf[len-1] == '\n') buf[--len] = '\0';
                                    /* Allocate result string via gc_alloc */
                                    extern int64_t gc_alloc(int64_t);
                                    char *result = (char *)(intptr_t)gc_alloc((int64_t)(len + 1));
                                    memcpy(result, buf, len + 1);
                                    mog_future_complete(w->future, (int64_t)(intptr_t)result);
                                } else {
                                    /* EOF or error */
                                    mog_future_complete(w->future, 0);
                                }
                            } else {
                                /* Generic fd ready — complete with fd number */
                                mog_future_complete(w->future, (int64_t)w->fd);
                            }
                            free(w);
                        } else {
                            prev_ptr = &w->next;
                        }
                        w = next;
                    }
                }
            } else if (tv_ptr) {
                /* No fd watchers, just sleep for the timeout */
                sleep_ns((uint64_t)tv_ptr->tv_sec * 1000000000ULL +
                         (uint64_t)tv_ptr->tv_usec * 1000ULL);
            }
        }

        /* Safety: if nothing happened and nothing is pending, exit */
        if (!processed && !loop->timers && !loop->ready_head && !loop->watchers) {
            break;
        }
    }

    loop->running = 0;
}

/* ---- Async I/O ---- */

MogFuture *async_read_line(void) {
    MogFuture *future = mog_future_new();
    MogEventLoop *loop = mog_loop_get_global();
    if (!loop) {
        /* Fallback: synchronous read */
        char buf[4096];
        if (fgets(buf, sizeof(buf), stdin)) {
            size_t len = strlen(buf);
            if (len > 0 && buf[len-1] == '\n') buf[--len] = '\0';
            extern int64_t gc_alloc(int64_t);
            char *result = (char *)(intptr_t)gc_alloc((int64_t)(len + 1));
            memcpy(result, buf, len + 1);
            mog_future_complete(future, (int64_t)(intptr_t)result);
        } else {
            mog_future_complete(future, 0);
        }
    } else {
        /* Register stdin fd watcher — future will be completed when stdin is ready */
        mog_loop_add_fd_watcher(loop, STDIN_FILENO, MOG_FD_READ, future);
    }
    return future;
}

/* ---- Await helper ---- */

int64_t mog_await(MogFuture *future, void *coro_handle) {
    if (!future) return 0;
    /* Always register the coroutine handle so the event loop can resume us */
    future->coro_handle = coro_handle;
    if (future->state != MOG_FUTURE_PENDING) {
        /* Already ready — enqueue for immediate resumption by the event loop.
         * The coroutine will suspend (coro.suspend) and the event loop will
         * pick this up from the ready queue and resume immediately. */
        mog_loop_enqueue_ready(g_loop, future);
    }
    return 0;
}

/* ---- Coroutine helpers ---- */

void *mog_future_get_coro_handle(MogFuture *future) {
    if (!future) return NULL;
    return future->coro_handle;
}

void mog_future_set_coro_frame(MogFuture *future, void *frame) {
    if (!future) return;
    future->coro_frame = frame;
}

/* Schedule a coroutine handle for resumption via the event loop.
 * Creates a temporary future to carry the handle through the ready queue. */
void mog_loop_schedule(MogEventLoop *loop, void *coro_handle) {
    if (!loop || !coro_handle) return;
    /* Re-use the ready queue by creating a transient future */
    MogFuture sentinel;
    memset(&sentinel, 0, sizeof(sentinel));
    sentinel.coro_handle = coro_handle;
    sentinel.state = MOG_FUTURE_READY;
    /* We need a heap-allocated future for the linked list */
    MogFuture *f = (MogFuture *)calloc(1, sizeof(MogFuture));
    if (!f) return;
    f->coro_handle = coro_handle;
    f->state = MOG_FUTURE_READY;
    mog_loop_enqueue_ready(loop, f);
}

/* ---- Combinators ---- */

MogFuture *mog_all(MogFuture **futures, int count) {
    MogFuture *parent = mog_future_new();
    if (count == 0) {
        mog_future_complete(parent, 0);
        return parent;
    }

    parent->sub_futures = (MogFuture **)malloc(sizeof(MogFuture *) * count);
    parent->sub_results = (int64_t *)calloc(count, sizeof(int64_t));
    parent->sub_count = count;
    parent->sub_done  = 0;

    int already_done = 0;
    for (int i = 0; i < count; i++) {
        parent->sub_futures[i] = futures[i];
        futures[i]->parent = parent;
        if (futures[i]->state != MOG_FUTURE_PENDING) {
            parent->sub_results[i] = futures[i]->result;
            already_done++;
        }
    }
    parent->sub_done = already_done;

    if (already_done >= count) {
        parent->state = MOG_FUTURE_READY;
        parent->result = (int64_t)(intptr_t)parent->sub_results;
        if (g_loop) g_loop->pending_count--; /* was incremented in mog_future_new */
    }

    return parent;
}

MogFuture *mog_race(MogFuture **futures, int count) {
    MogFuture *parent = mog_future_new();
    if (count == 0) {
        mog_future_complete(parent, 0);
        return parent;
    }

    /* race: no sub_futures array, just set parent on children */
    for (int i = 0; i < count; i++) {
        futures[i]->parent = parent;
        if (futures[i]->state != MOG_FUTURE_PENDING && parent->state == MOG_FUTURE_PENDING) {
            /* Already done — complete parent immediately */
            parent->state = MOG_FUTURE_READY;
            parent->result = futures[i]->result;
            if (g_loop) g_loop->pending_count--;
        }
    }

    return parent;
}
