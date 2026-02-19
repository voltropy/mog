# Chapter 14: Capabilities — Safe I/O

Mog has no built-in I/O. No file reads, no network calls, no environment variables — nothing that touches the outside world lives in the language itself. All side effects flow through **capabilities**: named interfaces that the host application provides to the script at runtime.

This is the foundation of Mog's security model. If the host doesn't grant a capability, the script can't use it. There's no escape hatch, no FFI backdoor, no unsafe block. A Mog script can only do what the host explicitly allows.

## The Capability Model

A capability is a named collection of functions provided by the host. From the script's perspective, it looks like a module with dot-syntax method calls:

```mog
requires fs;

async fn main() -> int {
  content := await fs.read_file("config.json")?;
  print(content);
  return 0;
}
```

The key difference from a regular module: there is no Mog source code behind `fs`. The host application implements those functions in C (or whatever language hosts the VM) and registers them before the script runs.

This design has three consequences:

1. **Sandboxing is the default.** A script with no `requires` declaration has zero access to the outside world. It can compute, but it can't affect anything.
2. **The compiler enforces declarations.** If you call `fs.read_file` without declaring `requires fs`, the compiler rejects your program.
3. **The host enforces availability.** If a script declares `requires http` but the host doesn't provide `http`, the host rejects the script before it runs.

## Declaring Capabilities: `requires` and `optional`

Capability declarations go at the top of the file, before any function definitions:

```mog
requires fs, process;
optional log;
```

`requires` means the program cannot run without these capabilities. If any are missing, the host refuses to execute the script:

```mog
requires fs, process;

async fn main() -> int {
  content := await fs.read_file("data.txt")?;
  dir := process.cwd();
  print("read from {dir}");
  return 0;
}
```

`optional` means the program can function without these capabilities but will use them if available. You check at runtime whether an optional capability is present:

```mog
requires fs;
optional log;

async fn main() -> int {
  data := await fs.read_file("input.txt")?;
  // log may or may not be available
  log.info("loaded input file");
  return 0;
}
```

A program with no declarations at all is a pure computation — it takes no input and produces no output beyond its return value:

```mog
fn fibonacci(n: int) -> int {
  if n <= 1 { return n; }
  a := 0;
  b := 1;
  for i in 2..n+1 {
    temp := a + b;
    a = b;
    b = temp;
  }
  return b;
}

fn main() -> int {
  return fibonacci(10);  // 55
}
```

> **Note:** Capabilities use the same dot-syntax as package imports (Chapter 13), but they are backed by host-provided C functions, not Mog source code. The compiler knows the difference because capabilities are declared with `requires`/`optional`, not `import`.

## Built-in Capabilities

The Mog runtime ships with a POSIX host that provides two standard capabilities: `fs` and `process`. These are conventional names — the host registers them, not the language.

### `fs` — File System

The `fs` capability provides file operations. Some may be async under the hood depending on the host, but the Mog interface uses `await`:

```mog
requires fs;

async fn main() -> int {
  // Read an entire file as a string
  content := await fs.read_file("data.txt")?;
  print(content);

  // Write a string to a file (creates or overwrites)
  await fs.write_file("output.txt", "hello, world")?;

  // Append to a file
  await fs.append_file("log.txt", "new entry\n")?;

  // Check if a file exists
  if await fs.exists("config.json")? {
    print("config found");
  }

  // Get file size in bytes
  size := await fs.file_size("data.txt")?;
  print("file is {size} bytes");

  // Remove a file
  await fs.remove("temp.txt")?;

  return 0;
}
```

The full `fs` interface:

| Function | Signature | Description |
|---|---|---|
| `read_file` | `(path: string) -> string` | Read entire file contents |
| `write_file` | `(path: string, contents: string) -> int` | Write string to file |
| `append_file` | `(path: string, contents: string) -> int` | Append string to file |
| `exists` | `(path: string) -> bool` | Check if path exists |
| `remove` | `(path: string) -> int` | Delete a file |
| `file_size` | `(path: string) -> int` | Get file size in bytes |

### `process` — Process and Environment

The `process` capability provides access to the runtime environment:

```mog
requires process;

async fn main() -> int {
  // Sleep for 500 milliseconds
  await process.sleep(500);

  // Get current timestamp (milliseconds since Unix epoch)
  now := process.timestamp();
  print("current time: {now}");

  // Get the current working directory
  dir := process.cwd();
  print("working in: {dir}");

  // Read an environment variable
  home := process.getenv("HOME");
  print("home directory: {home}");

  // Exit with a specific code
  process.exit(0);

  return 0;
}
```

The full `process` interface:

| Function | Signature | Description |
|---|---|---|
| `sleep` | `async (ms: int) -> int` | Pause execution for `ms` milliseconds |
| `timestamp` | `() -> int` | Milliseconds since Unix epoch |
| `cwd` | `() -> string` | Current working directory |
| `getenv` | `(name: string) -> string` | Read environment variable |
| `exit` | `(code: int) -> int` | Terminate the program |

## Practical Examples

### Copying a File

```mog
requires fs;

async fn copy_file(src: string, dst: string) -> int {
  content := await fs.read_file(src)?;
  await fs.write_file(dst, content)?;
  return 0;
}

async fn main() -> int {
  await copy_file("original.txt", "backup.txt")?;
  print("file copied");
  return 0;
}
```

### Reading Configuration

```mog
requires fs, process;

async fn main() -> int {
  // Try multiple config locations
  home := process.getenv("HOME");
  paths := [
    ".config.json",
    "{home}/.mogrc",
    "/etc/mog/config.json",
  ];

  for path in paths {
    if await fs.exists(path)? {
      config := await fs.read_file(path)?;
      print("loaded config from {path}");
      return 0;
    }
  }

  print("no config file found");
  return 1;
}
```

### Timed Operations

```mog
requires process;

async fn main() -> int {
  start := process.timestamp();

  // Do some work
  sum := 0;
  for i in 0..1000000 {
    sum = sum + i;
  }

  elapsed := process.timestamp() - start;
  print("computed sum={sum} in {elapsed}ms");
  return 0;
}
```

### Simple Logger Using `fs`

```mog
requires fs, process;

async fn log(message: string) -> int {
  ts := process.timestamp();
  line := "[{ts}] {message}\n";
  await fs.append_file("app.log", line)?;
  return 0;
}

async fn main() -> int {
  await log("application started")?;
  await log("processing data")?;

  result := do_work();
  await log("finished with result: {result}")?;

  return 0;
}

fn do_work() -> int {
  // pure computation — no capabilities needed
  total := 0;
  for i in 1..101 {
    total = total + i * i;
  }
  return total;
}
```

> **Tip:** Keep capability-using functions separate from pure computation. This makes your code easier to test — pure functions need no host at all. Notice how `do_work` above has no `requires` and no `await`.

## Custom Host Capabilities

The built-in `fs` and `process` capabilities are just the standard ones. Any host application can define its own capabilities with whatever functions make sense for the domain.

### `.mogdecl` Files

Custom capabilities are declared in `.mogdecl` files. These files tell the compiler what functions a capability provides, so it can type-check calls at compile time:

```
capability env {
  fn get_name() -> string
  fn get_version() -> int
  fn timestamp() -> int
  fn random(min: int, max: int) -> int
  fn log(message: string)
  async fn delay_square(value: int, delay_ms: int) -> int
}
```

A `.mogdecl` file contains no implementation — it's a type declaration. The host provides the actual function bodies in C (or whatever the host language is).

Here's the declaration for the built-in `fs` capability:

```
capability fs {
  fn read_file(path: string) -> string
  fn write_file(path: string, contents: string) -> int
  fn append_file(path: string, contents: string) -> int
  fn exists(path: string) -> bool
  fn remove(path: string) -> int
  fn file_size(path: string) -> int
}
```

And `process`:

```
capability process {
  async fn sleep(ms: int) -> int
  fn getenv(name: string) -> string
  fn cwd() -> string
  fn exit(code: int) -> int
  fn timestamp() -> int
}
```

### Using a Custom Capability

Once the host registers a capability and provides a matching `.mogdecl` file, a Mog script uses it exactly like a built-in:

```mog
requires env;

async fn main() -> int {
  name := env.get_name();
  version := env.get_version();
  print("running {name} v{version}");

  // Call an async host function
  result := await env.delay_square(7, 100)?;
  print("7 squared (after 100ms delay): {result}");

  // Get a random number from the host
  roll := env.random(1, 6);
  print("dice roll: {roll}");

  return 0;
}
```

### How the Pieces Fit Together

The flow from script to host and back:

1. The host application starts and creates a `MogVM`.
2. The host registers capability implementations — C functions grouped by capability name.
3. The compiler reads `.mogdecl` files and validates that every capability call in the script matches a declared function signature.
4. At runtime, when the script calls `env.random(1, 6)`, the VM routes the call through `mog_cap_call_out()` to the host's C function.
5. The host function receives the arguments as `MogValue`s, does its work, and returns a `MogValue` result.

The script never knows or cares how the host implements the functions. The `.mogdecl` file is the contract between the two sides.

> **Note:** For details on implementing host functions in C and registering capabilities with the VM, see Chapter 15: Embedding Mog.

### Capability Validation Example

```mog
requires fs, process, http;

async fn main() -> int {
  data := await http.get("https://api.example.com/data")?;
  await fs.write_file("cached.json", data)?;
  return 0;
}
```

If the host only provides `fs` and `process` but not `http`, the script is rejected before execution. The host calls `mog_validate_capabilities()` and gets back an error indicating that `http` is missing. This is a deliberate, early failure — no partial execution, no runtime surprise.

## Summary

| Concept | Meaning |
|---|---|
| `requires fs, process;` | Script needs these capabilities to run |
| `optional log;` | Script can use these if available |
| `.mogdecl` file | Type declaration for a custom capability |
| `fs.read_file(path)` | Call a capability function |
| No declaration | Pure computation, no I/O |

Capabilities are the only way for Mog code to interact with the outside world. This constraint is what makes Mog safe to embed — the host is always in control. The next chapter shows how to set up that host: creating a VM, registering capabilities from C, and enforcing resource limits.
