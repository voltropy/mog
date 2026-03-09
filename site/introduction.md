---
layout: null
title: "Mog: A Programming Language for AI Agents"
permalink: false
---

# Mog: A Programming Language for AI Agents

> What if an AI agent could modify itself quickly, easily, and safely? Mog is a
> programming language designed for exactly this.

<a class="jump-to-examples" href="#chapter-1-your-first-mog-program">Jump to Code Examples <span aria-hidden="true">↓</span></a>

---

## Overview

Mog is a statically typed, compiled, embedded language (think statically typed Lua) designed to be written by LLMs -- the full spec fits in 3200 tokens.

- An AI agent writes a Mog program, compiles it, and dynamically loads it as a plugin, script, or hook.
- The host controls exactly which functions a Mog program can call (capability-based permissions), so permissions propagate from agent to agent-written code.
- Compiled to native code for low-latency plugin execution -- no interpreter overhead, no JIT, no process startup cost.
- The compiler is being rewritten in safe Rust so the entire toolchain can be audited for security.
- Even without a full security audit, Mog is already useful for agents extending themselves with their own code.
- MIT licensed, contributions welcome. https://github.com/voltropy/mog

## Examples

### Agent hook

An agent hook that runs after context compaction. The first two lines are the entire security story: `import` pulls in host-defined types, `optional` capabilities degrade gracefully, and structs are constructed inline.

```mog
import agent;       // Agent, Message, Role types
optional log;       // silently ignored if not provided

// post-compaction hook: re-inject key context that may have been lost
pub fn on_post_compaction(session: agent.Session) {
  log.info("post-compaction hook: injecting reminder");

  session.messages.push(agent.Message {
    role: agent.Role.system,
    content: "IMPORTANT: Always run tests before committing.",
  });
}
```

### Async HTTP with retry

An async HTTP fetcher with retry logic -- 17 lines, no boilerplate. `async`/`await` suspends without blocking, `match` destructures `Result` values, and f-strings interpolate expressions directly.

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

### FFT on tensors

A radix-2 FFT on `tensor<f32>` data -- real numeric code, not a toy. Mog has no operator precedence, so mixed arithmetic requires explicit parentheses. Type conversions like `size as float` and `cos(angle) as f32` are always explicit -- no implicit coercion.

```mog
// Fast Fourier Transform (Cooley-Tukey, radix-2, in-place)
// Returns a 2×n tensor: row 0 = real, row 1 = imaginary.

fn fft(re: tensor<f32>, im: tensor<f32>) -> tensor<f32> {
  n := re.shape[0];
  r := tensor<f32>.zeros([n]);
  im_out := tensor<f32>.zeros([n]);

  for i in 0..n {
    r[i] = re[i];
    im_out[i] = im[i];
  }

  // bit-reversal permutation
  j := 0;
  for i := 1 to (n - 1) {
    bit := n / 2;
    while j >= bit {
      j = j - bit;
      bit = bit / 2;
    }
    j = j + bit;
    if i < j {
      tmp := r[i]; r[i] = r[j]; r[j] = tmp;
      tmp = im_out[i]; im_out[i] = im_out[j]; im_out[j] = tmp;
    }
  }

  // Cooley-Tukey butterfly — mixed operators require explicit parens
  size := 2;
  while size <= n {
    half := size / 2;
    step := (0.0 - 6.283185307) / (size as float);  // -2π/size, explicit int->float cast
    k := 0;
    while k < n {
      angle := 0.0;
      for m := 0 to (half - 1) {
        cos_a := cos(angle) as f32;  // math builtins return f64; cast to match tensor
        sin_a := sin(angle) as f32;
        idx := (k + m) + half;

        tr := (r[idx] * cos_a) - (im_out[idx] * sin_a);
        ti := (r[idx] * sin_a) + (im_out[idx] * cos_a);

        r[idx] = r[(k + m)] - tr;
        im_out[idx] = im_out[(k + m)] - ti;
        r[(k + m)] = r[(k + m)] + tr;
        im_out[(k + m)] = im_out[(k + m)] + ti;

        angle = angle + step;
      }
      k = k + size;
    }
    size = size * 2;
  }

  // pack real and imaginary into 2×n result
  result := tensor<f32>.zeros([2, n]);
  for i in 0..n {
    result[i] = r[i];
    result[n + i] = im_out[i];
  }
  return result;
}
```

## Why Mog?

A general-purpose AI agent should be able to continuously extend and modify itself. Over time, an agent should grow into a personal server that manages tasks in all kinds of ways. To do that, the agent needs to write its own code -- and that code needs to be safe.

The simplest kind of program an agent writes is a one-off script to achieve some task. Examples:
- Converting a markdown file to PDF.
- Analyzing a CSV with database results.
- Sending test requests to an application's HTTP endpoint.
- Renaming all the files in a folder.
- Installing dependencies.

Coding agents typically use bash for this, and sometimes reach for an inline Python or TypeScript script. Mog is well-suited for this: it's easy to write and it compiles fast. Notably, scripting is one of the main ways an agent escapes its sandbox, and Mog closes that loophole. Even if Mog is given a capability by its host agent to call bash commands, the host still has the ability to filter those commands according to its permissions, just as if the model had called its bash tool directly.

The second kind of program agents commonly write is a hook: a piece of code that runs repeatedly at a certain point in the agentic loop. Pre- and post-tool-use hooks are common, as well as pre-compaction hooks. For a hook, it's not important for it to compile quickly, but it needs to start up quickly and execute quickly, since it can get called frequently enough that a slow implementation would meaningfully drag down the user experience. Mog compiles to native code, and it can then *load that machine code into the agent's running binary*. The key property that makes this safe: native code compiled by the Mog compiler can't do anything other than what the host explicitly lets it do -- not even exceed limits on memory or time. The agent can incorporate a Mog program into itself at runtime and call into it without inter-process communication overhead or process startup latency.

The third category is writing or rewriting parts of the agent itself. This could mean adding a new tool that the agent exposes to the LLM, a status line for the user interface, settings about agent skills or session management -- a potentially long list of agent internals that would benefit from extension or specialization.

One could imagine a microkernel agent, written as a small backbone of Rust code that calls into Mog code for almost all its features. The kernel would manage the event loops, multi-threading, root permissions, compiling and loading Mog programs, and maybe part of its upgrade functionality, but the rest of the system could be written in Mog -- running with minimal, granular permissions and upgradeable on the fly without a restart, just by prompting the agent to modify itself.

## Alternatives

Without something like Mog, every option for AI-generated agent code has a downside. One major one is enforcing permissions: tools like Jeffrey Emanuel's [`dcg`](https://github.com/Dicklesworthstone/destructive_command_guard) can interdict `rm -rf` and similarly destructive shell commands, but they can't stop an agent from emitting Python that iterates through files in a folder and calls `os.remove()` on each one.

The next step is generally to run the agent in a sandbox, like a Docker container. But then the permissions tend to apply to the whole sandbox, so letting the agent use the host computer in nontrivial ways (e.g. pull in environment variables, access CLI tools, drive a browser, make HTTP requests) requires opening up the sandbox boundary. At that point the agent has regained essentially unfettered access to that capability.

What's missing is a way to propagate the permissions granted to an agent to the programs that agent writes. Mog addresses this directly.

Another issue with external scripts is sharing them -- receiving a script from an untrusted source is a security risk. With Mog, the receiving agent compiles the Mog source itself, rather than running a pre-compiled binary that could take over the machine.

There are other approaches to LLM-driven extension systems. Jeffrey Emanuel is building something similar using JavaScript containers with permissions, which is quite close to Mog in spirit, and has an impressive level of infrastructure behind it. Mog could complement such systems, especially for higher-performance plugins -- Mog's infrastructure is also in Rust, making integration straightforward. Emanuel's [Rust version of the Pi agent](https://github.com/Dicklesworthstone/pi_agent_rust) would be a natural starting point for a Mog-based microkernel agent.

WASM is another natural option, since it's a more standard sandboxing technique. Mog takes a different approach: a hot loop over an array runs at native speed, without the unpredictability of JIT compilation, the overhead of WASM interpretation, or the process startup time of an external binary.

## The Language

Mog is a minimal language designed to be written by LLMs. Its full specification, with examples, fits in a single [3200-token markdown file](https://github.com/voltropy/mog/blob/main/docs/context.md), which an LLM can pin in context when writing Mog programs. Everything about the language is designed to optimize the latency between asking an LLM to write a program for a task and having a performant program that implements that task.

The primary goal of Mog syntax is to be familiar to LLMs. It's a mix of TypeScript, Rust, and Go, with a couple of Pythonisms. Its module system is modeled on Go modules. It's a small but usable language with no foot-guns: no implicit type coercion, no operator precedence, and no uninitialized data.

Why statically typed and compiled? Because latency matters in this domain. Consider an agent plugin that runs before every tool call -- it needs to be fast, or it's not worth using. Jeffrey Emanuel rewrote every single one of his agent plugins in Rust to reduce startup time and Python overhead. A modern computer is fast, but Python is not. Even Bun's startup time, which has been heavily optimized, is nowhere near the startup time of a Rust program, and even that is slow compared to calling into an in-process library.

### Example

Here's a Mog hook that an agent might write to monitor its own tool calls for errors:

```mog
requires fs;  // must be provided by host
optional log; // runs without it

pub fn on_tool_result(tool_name: string, stderr: string) {
  if (stderr.contains("permission denied")) {
    log.warn(f"{tool_name}: permission denied");
    fs.append_file("agent.log", f"[warn] {tool_name}: {stderr}\n");
  }
}
```

The first two lines tell the security story: this hook can append to a file and optionally log, but it cannot make HTTP requests, run shell commands, or read environment variables. The host decides what this program is allowed to do -- it can even interdict the `fs.append_file()` call if this program doesn't have that permission.

To learn more about how to use Mog, see the detailed [Mog Language Guide](/).

As agents get more complex and more of their code is bespoke, performance, correctness, and security of the combined system become harder to maintain. Mog is designed to address these issues.

## The Capability System

Mog is an embedded language. It runs inside a host program, much like Lua -- the way a Mog host provides functions to the guest Mog program is based directly on Lua's elegant and battle-tested design. By itself, a Mog program cannot do any I/O, perform syscalls, or access raw memory. It can only run functions and return values. That is the extent of Mog's sandbox.

Any I/O, FFI, syscalls, or interaction with other systems can only be done by calling a host function -- a function that the host makes available to the Mog program. This is the essence of Mog's capability system: the host decides exactly which functions it will allow the guest to call. The host is also free to filter the inputs to such a function and to filter the response delivered back to the Mog program.

Part of this isolation involves preventing a Mog program from taking over the host process in subtler ways. An upcoming feature will allow the host to control whether a Mog program can request a larger memory arena, preventing the guest from consuming all available RAM. An existing feature is cooperative interrupt polling: Mog loops all have interrupt checks added at back-edges, which allows the host to halt the guest program without killing the process. This enables timeout enforcement. There is no way for a guest program to corrupt memory or kill the process (assuming correct implementation of the compiler and host).

Since a typical agent runs an event loop, Mog programs are designed to run inside an event loop, familiar to anyone who has written JavaScript or TypeScript. Mog's support for this consists primarily of async/await syntax. Mog programs can define async functions, and importantly, the host can also provide async functions that the guest can call. This allows a guest program to fire off an HTTP request and a timer and do something different depending on which one finishes first -- internally the compiler implements this using coroutine lowering, based on LLVM's design for the same.

## The Compiler

The compiler uses [QBE](https://c9x.me/compile/) as its code generation backend, which is being rewritten in safe Rust. Once complete, the entire toolchain for compiling, loading, and running a Mog program will be in Rust -- the goal is for all of it to be safe Rust, making it much more difficult to find an exploit in the toolchain.

The first implementation of Mog used LLVM as the backend. LLVM can produce somewhat faster code due to its wide array of optimizations, but it had two major issues. First, compile times were not fast enough. The new compiler has compile times that are not quite as good as Go's, but within an order of magnitude for programs under 1000 lines -- fast enough that the start time for one-off scripts is not painful. Mog does not claim to provide zero-cost abstractions or arbitrary opportunities for low-level optimization. It compiles to native code, but an expert can still write faster C or C++.

The second issue with LLVM is that for Mog, the compiler itself is part of the trusted computing base. If the compiler has a security flaw, the agent has a security flaw. This rules out an enormous codebase like LLVM's. The compiler needs to be small enough to control and audit.

## Current Status

Mog was created entirely using the [Volt](https://github.com/Martian-Engineering/volt) coding agent, the vast majority of which used a single continuous session spanning over three weeks, using Voltropy's [Lossless Context Management](https://papers.voltropy.com/LCM) to maintain its memory after compactions. This session is still running, porting the QBE compiler to safe Rust. The models used were Claude Opus 4.6, Kimi k2.5, and GLM-4.7.

Significant work remains to standardize the functions that hosts provide to Mog programs. This should include much of what the Python standard library provides: support for JSON, CSV, SQLite, POSIX filesystem and networking operations, etc. The Mog standard library should also provide async functions for calling LLMs -- both high-level interfaces (like Simon Willison's `llm` CLI tool or `DSPy` modules) and low-level interfaces that allow fine-grained context management.

To be clear: Mog has *not* been audited, and it is presented without security guarantees. It should be possible to secure it, but that work has not yet been done. There are tests that check that a malicious Mog program cannot access host functionality that the host does not want to provide, but this design has enough security attack surface to warrant careful scrutiny.

Even without audited security properties, Mog is already useful for extending AI agents *with their own code* -- they're not trying to exploit themselves. For untrusted third-party code, treat the security model as unverified until a formal audit is completed.
