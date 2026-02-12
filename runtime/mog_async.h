#ifndef MOG_ASYNC_H
#define MOG_ASYNC_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/* ---- Forward declarations ---- */
typedef struct MogFuture    MogFuture;
typedef struct MogEventLoop MogEventLoop;
typedef struct MogTimer     MogTimer;

/* ---- Future states ---- */
typedef enum {
    MOG_FUTURE_PENDING = 0,
    MOG_FUTURE_READY   = 1,
    MOG_FUTURE_ERROR   = 2
} MogFutureState;

/* ---- Future ----
 * Represents an eventual i64 value.
 * Allocated with malloc (NOT gc_alloc) to avoid GC interaction
 * with LLVM coroutine frames.
 */
struct MogFuture {
    MogFutureState state;
    int64_t        result;       /* The completed value */
    void          *coro_handle;  /* Coroutine handle waiting on this future */
    void          *coro_frame;   /* Coroutine frame memory (freed when future is freed) */
    MogFuture     *next;         /* Intrusive linked list for ready queue */
    /* For all() combinator */
    MogFuture    **sub_futures;  /* Array of sub-futures */
    int            sub_count;    /* Number of sub-futures */
    int            sub_done;     /* How many sub-futures are done */
    int64_t       *sub_results;  /* Results array for all() */
    MogFuture     *parent;       /* Parent future (for all/race children) */
};

/* ---- Timer ---- */
struct MogTimer {
    uint64_t    deadline_ns;  /* Absolute monotonic time */
    MogFuture  *future;       /* Future to complete when timer fires */
    MogTimer   *next;         /* Sorted linked list */
};

/* ---- FD Watcher ---- */
typedef struct MogFdWatcher MogFdWatcher;
struct MogFdWatcher {
    int          fd;          /* File descriptor to watch */
    int          events;      /* MOG_FD_READ or MOG_FD_WRITE */
    MogFuture   *future;      /* Future to complete when fd is ready */
    MogFdWatcher *next;       /* Linked list */
};

#define MOG_FD_READ  1
#define MOG_FD_WRITE 2

/* ---- Event Loop ---- */
struct MogEventLoop {
    MogFuture    *ready_head;    /* Queue of coroutines to resume */
    MogFuture    *ready_tail;
    MogTimer     *timers;        /* Sorted timer list */
    MogFdWatcher *watchers;      /* FD watchers list */
    int           running;       /* Whether loop is active */
    int           pending_count; /* Number of outstanding futures */
};

/* ---- Event Loop lifecycle ---- */
MogEventLoop *mog_loop_new(void);
void          mog_loop_free(MogEventLoop *loop);
void          mog_loop_run(MogEventLoop *loop);

/* ---- Future operations (called from generated LLVM IR) ---- */
MogFuture *mog_future_new(void);
void       mog_future_complete(MogFuture *future, int64_t value);
void       mog_future_set_error(MogFuture *future, int64_t err);
int        mog_future_is_ready(MogFuture *future);
int64_t    mog_future_get_result(MogFuture *future);
void       mog_future_set_waiter(MogFuture *future, void *coro_handle);
void       mog_future_free(MogFuture *future);

/* ---- Scheduling ---- */
void mog_loop_enqueue_ready(MogEventLoop *loop, MogFuture *future);
void mog_loop_schedule(MogEventLoop *loop, void *coro_handle);
void mog_loop_add_timer(MogEventLoop *loop, uint64_t delay_ms, MogFuture *future);

/* ---- Global event loop ---- */
void          mog_loop_set_global(MogEventLoop *loop);
MogEventLoop *mog_loop_get_global(void);

/* ---- Await helper (called from LLVM IR) ----
 * If future is ready, returns result immediately.
 * If future is pending, sets coro_handle as waiter and returns 0.
 */
int64_t mog_await(MogFuture *future, void *coro_handle);

/* ---- Coroutine helpers (called from LLVM IR) ---- */
void  mog_coro_resume(void *handle);
void *mog_future_get_coro_handle(MogFuture *future);
void  mog_future_set_coro_frame(MogFuture *future, void *frame);

/* ---- FD Watchers ---- */
void mog_loop_add_fd_watcher(MogEventLoop *loop, int fd, int events, MogFuture *future);

/* ---- Async I/O ---- */
MogFuture *async_read_line(void);

/* ---- Combinators ---- */
MogFuture *mog_all(MogFuture **futures, int count);
MogFuture *mog_race(MogFuture **futures, int count);

#endif /* MOG_ASYNC_H */
