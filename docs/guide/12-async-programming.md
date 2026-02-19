# Chapter 12: Async Programming

Mog uses `async`/`await` for asynchronous operations. Agent scripts need to wait on external operations — API calls, model inference, file I/O — and async functions let you express that waiting without blocking the entire program. The host runtime manages the event loop; Mog code never creates threads or manages concurrency primitives directly.

## Async Functions

Mark a function as `async` to indicate it returns a future. Use `await` inside to wait for other async operations:

```mog
async fn fetch(url: string) -> Result<string> {
  response := await http.get(url)?;
  return ok(response.body);
}
```

An `async fn` can be called like any other function, but its return value is a future that must be `await`ed to get the actual result:

```mog
async fn greet(name: string) -> string {
  return f"hello, {name}";
}

async fn main() -> int {
  msg := await greet("world");
  println(msg);  // hello, world
  return 0;
}
```

> **Note:** When `main` is declared `async`, the runtime creates the event loop automatically. You don't need to set up or start the loop yourself.

Async functions can call other async functions. Each `await` suspends the current function until the awaited future completes:

```mog
async fn fetch_json(url: string) -> Result<string> {
  raw := await http.get(url)?;
  parsed := parse_json(raw.body)?;
  return ok(parsed);
}

async fn get_user_name(id: int) -> Result<string> {
  data := await fetch_json(f"https://api.example.com/users/{id}")?;
  return ok(data["name"]);
}
```

## Await

The `await` keyword suspends execution until a future resolves. It works on any expression that produces a future:

```mog
async fn pipeline() -> Result<string> {
  raw := await fetch_data("https://api.example.com/data")?;
  processed := await transform(raw)?;
  return ok(processed);
}
```

Each `await` is a suspension point. The runtime can run other tasks while this function waits. Without `await`, the future is created but never resolved:

```mog
async fn example() -> Result<string> {
  // This creates a future but doesn't wait for it — probably a bug
  // fetch_data("https://api.example.com");

  // This creates the future AND waits for the result
  result := await fetch_data("https://api.example.com")?;
  return ok(result);
}
```

> **Warning:** Forgetting `await` is a common mistake. If you call an async function without `await`, you get a future object, not the actual result.

You can combine `await` with the `?` operator from Chapter 11. The `await` resolves the future, then `?` unwraps the `Result`:

```mog
async fn load_config(path: string) -> Result<Config> {
  content := await fs.read_async(path)?;    // await the I/O, then ? the Result
  config := parse_config(content)?;          // synchronous parse, just ? the Result
  return ok(config);
}
```

## Spawn

Use `spawn` to launch a task that runs in the background. The spawned task executes concurrently with the rest of your code — you don't wait for it to finish:

```mog
async fn log_event(event: string) -> Result<int> {
  await http.post("https://logs.example.com", event)?;
  return ok(0);
}

async fn handle_request(req: Request) -> Result<Response> {
  // Fire and forget — don't wait for logging to complete
  spawn log_event(f"received request: {req.path}");

  result := await process(req)?;
  return ok(result);
}
```

Spawn is useful for side effects you don't need to wait on — logging, analytics, cache warming:

```mog
async fn warm_cache(keys: [string]) {
  for key in keys {
    data := await fetch_data(key)?;
    cache.set(key, data);
  }
}

async fn main() -> int {
  // Start cache warming in the background
  spawn warm_cache(["users", "products", "settings"]);

  // Continue immediately without waiting
  println("server starting...");
  await start_server(8080);
  return 0;
}
```

> **Tip:** Spawned tasks that fail do so silently — their errors aren't propagated to the caller. If you need to know whether a background task succeeded, use `await` instead of `spawn`, or use `all()` to collect results.

```mog
async fn main() -> int {
  // Bad: error is silently lost
  spawn might_fail();

  // Good: error is handled
  try {
    await might_fail();
  } catch(e) {
    println(f"task failed: {e}");
  }
  return 0;
}
```

## all() — Wait for All

`all()` takes a list of futures and waits for all of them to complete. It returns when every future has resolved, giving you all the results:

```mog
async fn parallel_fetch() -> Result<[string]> {
  results := await all([
    fetch_data("https://api.example.com/a"),
    fetch_data("https://api.example.com/b"),
    fetch_data("https://api.example.com/c"),
  ])?;
  return ok(results);
}
```

This is significantly faster than sequential awaits when the operations are independent:

```mog
// Sequential — each waits for the previous to finish
async fn fetch_sequential() -> Result<[string]> {
  a := await fetch_data("https://api.example.com/a")?;
  b := await fetch_data("https://api.example.com/b")?;
  c := await fetch_data("https://api.example.com/c")?;
  return ok([a, b, c]);
}

// Parallel — all three run at the same time
async fn fetch_parallel() -> Result<[string]> {
  results := await all([
    fetch_data("https://api.example.com/a"),
    fetch_data("https://api.example.com/b"),
    fetch_data("https://api.example.com/c"),
  ])?;
  return ok(results);
}
```

> **Note:** If any future in `all()` fails, the entire `all()` returns that error. All futures are started concurrently, but a single failure short-circuits the result.

A practical example — loading multiple resources in parallel to build a page:

```mog
struct Page {
  user: string,
  posts: string,
  notifications: string,
}

async fn load_page(user_id: int) -> Result<Page> {
  results := await all([
    fetch_data(f"https://api.example.com/users/{user_id}"),
    fetch_data(f"https://api.example.com/users/{user_id}/posts"),
    fetch_data(f"https://api.example.com/users/{user_id}/notifications"),
  ])?;

  return ok(Page {
    user: results[0],
    posts: results[1],
    notifications: results[2],
  });
}
```

## race() — Wait for First

`race()` takes a list of futures and returns the result of whichever finishes first. The remaining futures are cancelled:

```mog
async fn fastest() -> Result<string> {
  result := await race([
    fetch_from_primary(),
    fetch_from_backup(),
  ])?;
  return ok(result);
}
```

The most common use for `race()` is implementing timeouts:

```mog
async fn timeout(ms: int) -> Result<string> {
  await sleep(ms);
  return err("operation timed out");
}

async fn fetch_with_timeout(url: string) -> Result<string> {
  result := await race([
    fetch_data(url),
    timeout(5000),
  ])?;
  return ok(result);
}
```

Another use — trying multiple strategies and going with the first to succeed:

```mog
async fn resolve_address(host: string) -> Result<string> {
  result := await race([
    dns_lookup_v4(host),
    dns_lookup_v6(host),
  ])?;
  return ok(result);
}
```

> **Warning:** `race()` returns the first future to complete, whether it succeeds or fails. If the fastest future returns an error, that error propagates — even if a slower future would have succeeded. Design your futures accordingly.

## Error Handling with Async

Async functions combine naturally with `Result<T>` and the `?` operator from Chapter 11. The `await` resolves the future, and `?` unwraps the result:

```mog
async fn create_user(name: string, email: string) -> Result<User> {
  // Validate inputs (synchronous)
  validated_name := validate_name(name)?;
  validated_email := validate_email(email)?;

  // Check for duplicates (async)
  existing := await db.find_user_by_email(validated_email)?;
  match existing {
    some(_) => return err("email already registered"),
    none => {},
  }

  // Create the user (async)
  user := await db.insert_user(validated_name, validated_email)?;
  return ok(user);
}
```

Use `try`-`catch` inside async functions the same way you would in synchronous code:

```mog
async fn sync_all_data() {
  sources := ["users", "products", "orders"];

  for source in sources {
    try {
      data := await fetch_data(f"https://api.example.com/{source}")?;
      await db.upsert(source, data)?;
      println(f"synced {source}");
    } catch(e) {
      println(f"failed to sync {source}: {e}");
    }
  }
}
```

A complete async program with error handling:

```mog
async fn fetch_weather(city: string) -> Result<string> {
  url := f"https://weather.example.com/api?city={city}";
  response := await http.get(url)?;
  data := parse_json(response.body)?;
  return ok(data["temperature"]);
}

async fn main() -> int {
  cities := ["London", "Tokyo", "New York"];

  results := await all([
    fetch_weather("London"),
    fetch_weather("Tokyo"),
    fetch_weather("New York"),
  ]);

  match results {
    ok(temps) => {
      for i, city in cities {
        println(f"{city}: {temps[i]}");
      }
    },
    err(e) => {
      println(f"weather fetch failed: {e}");
    },
  }
  return 0;
}
```

### Retry Pattern

Combine async with a loop to retry failed operations:

```mog
async fn fetch_with_retry(url: string, max_retries: int) -> Result<string> {
  attempts := 0;
  for attempts < max_retries {
    match await http.get(url) {
      ok(response) => return ok(response.body),
      err(e) => {
        attempts = attempts + 1;
        if attempts >= max_retries {
          return err(f"failed after {max_retries} attempts: {e}");
        }
        println(f"attempt {attempts} failed, retrying...");
        await sleep(1000 * attempts);  // exponential-ish backoff
      },
    }
  }
  return err("unreachable");
}
```

### Fan-out / Fan-in

Process multiple items concurrently, then aggregate the results:

```mog
async fn process_batch(urls: [string]) -> Result<[string]> {
  // Build a list of futures
  futures := urls.map(fn(url: string) -> Future<Result<string>> {
    return fetch_data(url);
  });

  // Wait for all concurrently
  results := await all(futures)?;
  return ok(results);
}

async fn main() -> int {
  urls := [
    "https://api.example.com/1",
    "https://api.example.com/2",
    "https://api.example.com/3",
    "https://api.example.com/4",
    "https://api.example.com/5",
  ];

  match await process_batch(urls) {
    ok(data) => {
      for i, d in data {
        println(f"result {i}: {d}");
      }
    },
    err(e) => println(f"batch failed: {e}"),
  }
  return 0;
}
```

## Summary

| Syntax | Meaning |
|---|---|
| `async fn f() -> T` | Declare an async function returning a future |
| `await expr` | Suspend until a future resolves |
| `spawn task()` | Launch a fire-and-forget background task |
| `all([f1, f2, f3])` | Wait for all futures to complete |
| `race([f1, f2])` | Wait for the first future to complete |

Async functions compose with `Result<T>` and `?` from Chapter 11 — `await` resolves the future, then `?` unwraps the result. Use `all()` when you have independent operations that can run in parallel. Use `race()` for timeouts and fallback strategies. Use `spawn` only for side effects you don't need to track. The runtime manages the event loop; your job is to describe what depends on what.
