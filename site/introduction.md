---
layout: guide_page.njk
title: "Mog: A Programming Language for AI Agents"
permalink: /introduction/
---

# Mog: A Programming Language for AI Agents

> What if an AI agent could modify itself quickly, easily, and safely? Mog is a
> programming language designed for exactly this.

---

## Overview

Mog is a statically typed, compiled, embedded language (think statically typed Lua) designed to be written by LLMs -- the full spec fits in 3200 tokens.

- An AI agent writes a Mog program, compiles it, and dynamically loads it as a plugin, script, or hook.
- The host controls exactly which functions a Mog program can call (capability-based permissions), so permissions propagate from agent to agent-written code.
- Compiled to native code for low-latency plugin execution -- no interpreter overhead, no JIT, no process startup cost.
- The compiler is being rewritten in safe Rust so the entire toolchain can be audited for security.
- Even without a full security audit, Mog is already useful for agents extending themselves with their own code.
- MIT licensed, contributions welcome. https://github.com/voltropy/mog

## Why Mog?

I want to have a general-purpose agent, like OpenClaw, that I can continuously tweak to serve my needs better. (When I say 'I' can tweak it, of course I mean an LLM that can do the dirty work for me. To quote Andrej Karpathy, "what am I, a computer?"). Over time, my agent should grow itself into my own personal server that manages my life in all kinds of ways. In order to do that, the agent needs to modify itself in a few ways.

The simplest kind of program an agent needs to write is a one-off script that the LLM writes and executes to achieve some task. Examples:
- Converting a markdown file to PDF.
- Analyzing a CSV with database results.
- Sending test requests to an application's HTTP endpoint.
- Renaming all the files in a folder.
- Installing dependencies.

Coding agents typically use bash for this, but sometimes they'll reach for an inline Python or TypeScript script. Mog is well-suited for this, since it's easy to write and it compiles fast. Notably, scripting is one of the big ways an agent escapes its sandbox, and Mog closes that loophole quite nicely. Even if Mog is given a capability by its host agent to call bash commands, the host still has the ability to filter those commands according to its permissions, just as if the model had called its bash tool directly.

The second kind of program agents commonly write is a hook: a piece of code that runs repeatedly at a certain point in the agentic loop. Pre- and post-tool-use hooks are common, as well as pre-compaction hooks. For a hook, it's not important for it to compile quickly, but it needs to start up quickly and execute quickly, since it can get called frequently enough that a slow implementation would meaningfully drag down the user experience. Mog compiles to native code, and it can then *load that machine code into the agent's running binary*. Mog's claim to fame is making this safe. Because native code compiled by the Mog compiler can't do anything other than what the host explicitly lets it do (not even exceed limits on memory or time), the agent can incorporate the Mog program into itself at runtime and call into it without inter-process communication overhead or process startup latency.

The final category of agent-related program is a bit more nebulous, but it's more like writing or rewriting parts of the agent itself. This could mean adding a new tool that the agent exposes to the LLM, or a status line for the user interface, or settings about agent skills or session management... it's potentially a long list of agent internals that would benefit from extension or specialization.

One could imagine a microkernel of an agent, something like the Pi agent at the core of OpenClaw, written as a small backbone of Rust code that calls into Mog code for almost all its features. The kernel would need to manage the event loops, multi-threading, root permissions, compiling and loading Mog programs, and maybe part of its upgrade functionality, but it's not unthinkable that the rest of the system could be written in Mog, running with minimal, granular permissions and upgradeable on the fly without a restart, just by prompting the agent to modify itself. If someone doesn't build this soon, I might.

## Alternatives

Without something like Mog, every option for AI-generated agent code has a downside. One big one is enforcing permissions: I use Jeffrey Emanuel's [`dcg`](https://github.com/Dicklesworthstone/destructive_command_guard) plugin for Claude Code to interdict `rm -rf` and similarly destructive commands, but that can't stop Claude Code from emitting some Python that iterates through the files in a folder and calls `os.remove()` on each one.

If you're worried about that, the next step is generally to run your agent in a sandbox, like a Docker container. But then the permissions tend to apply to the whole sandbox, so if you want to let your agent use your computer in nontrivial ways (e.g. pull in environment variables, give access to CLI tools you have installed, drive a browser, make HTTP requests), you have to open up the boundary to that sandbox. Now you're back to the same problem you had before: the agent has regained essentially unfettered access to that capability.

What's missing is a way to propagate the permissions you want your agents to have to the programs those agents write. Mog is the only system I know of that lets you do this.

Another issue with using external scripts is sharing them -- how do I know you're not sending me malware? Mog also addresses this. My agent compiles the Mog source you send me, rather than running some binary you created that could take over my machine.

Of course, you could build such an LLM-driven extension system in a number of other ways. Jeffrey Emanuel is building something similar using JavaScript containers with permissions, which I think is quite close to Mog, and is likely better in some ways. He's certainly built an impressive level of infrastructure to run his system. My hope is that Mog could be an option in his world of Rust (Mog's infrastructure is also in Rust, partially for that reason), especially to provide higher-performance plugins. I would start from his [Rust version of the Pi agent](https://github.com/Dicklesworthstone/pi_agent_rust) to build the Mog microkernel I mentioned above.

Another natural option would be to use WASM, since that's a more standard sandboxing technique. But it seems to me that it's worth a shot to do it Mog's way, where if you want to write a hot loop over an array, you can know it'll run at native speed, without the unpredictability of JIT compilation, the overhead of WASM interpretation, or the process startup time of an external binary.

## The Language

As a language, Mog is a statically typed, compiled, embedded programming language. Mog is a minimal language designed to be written by LLMs, so its full specification, with examples, fits in a single [3200-token markdown file](https://github.com/voltropy/mog/blob/main/docs/context.md), which an LLM can pin in context when writing Mog programs. Everything about the language is designed to make it easy for LLMs to write, to optimize the latency between asking an LLM to write a program for a task and having a passably performant program that implements that task.

The primary goal of Mog syntax is to be familiar to LLMs. It's a mix of Claude's favorite languages -- TypeScript, Rust, and Go, with a couple of Pythonisms. Its module system is straight-up stolen from Go modules, which have never done me wrong. It's supposed to be a small but usable language with no foot-guns, which is also why it has no implicit type coercion, no operator precedence, and no uninitialized data.

Why statically typed and compiled? Because latency matters in this domain. Say I want my agent to have a plugin that runs before every tool call. That had better be lightning-fast, or I am turning it off. Again, Jeffrey Emanuel rewrote every single one of his plugins in Rust to reduce startup time and bogus Python overhead. A modern computer is mind-bogglingly fast, but alas, Python is not. Even Bun's startup time, which they optimized, is nowhere near as fast as the startup time of a Rust program, and even that is woefully slow compared to calling into an in-process library.

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

Agents aren't going anywhere. As they get more complex and more of their code is bespoke, performance, correctness, and security of the combined system become more difficult to maintain. Mog is an attempt to address these issues.

## The Capability System

As for the design of Mog, there are a number of things that are good to know. The big one is that it is an embedded language. It needs to run inside a host program, like Lua -- in fact, if you think of Mog as a statically typed Lua, that's not far off. The way a Mog host provides functions that the guest Mog program can run is based directly off of Lua's elegant and battle-tested design. By itself, a Mog program cannot do any I/O, perform syscalls, or access raw memory. It can just run functions and return values. To the extent that Mog has a 'sandbox', that's it.

Any I/O, FFI, syscalls, or any interaction with other systems can only be done in Mog by calling a host function, i.e. a function that the host makes available to the Mog program. This is the essence of Mog's capability system: the host decides exactly which functions it will allow the guest to call. The host is also free to filter the inputs to such a function once the Mog program has called it and to filter the response that the host will deliver back to the Mog program.

Part of this isolation involves preventing a Mog program from commandeering the host process in subtler, more insidious ways. A much-needed upcoming feature will allow the host to decide whether to acquiesce to a Mog program's request for a larger memory arena, so the guest can't run away with all the RAM. An existing feature is that Mog loops all have 'cooperative interrupt polling' added to them, which allows the host to halt the guest program, without killing the process. This allows the host to enforce a timeout on the guest program. There is no way for a guest program to corrupt memory or kill the process (assuming correct implementation of the compiler and host).

Since a typical agent runs an event loop, Mog programs are designed to run inside an event loop. This is familiar to anyone who has written JavaScript or TypeScript. Mog's support for this consists primarily of async/await syntax. Mog programs can define async functions, and importantly, the host can also provide async functions that the guest program can run. This allows the guest program to do the things you often want to do with a TypeScript program, such as fire off an HTTP request and a timer, and do something different depending on which one finishes first -- internally the compiler does this using coroutine lowering, which is based on LLVM's design for the same.

## The Compiler

Which brings me to the compiler itself. The compiler uses the existing [QBE](https://c9x.me/compile/) compiler, but I am in the process of rewriting that in safe Rust. Once that's done, the entire toolchain for compiling, loading, and running a Mog program will be in Rust. My goal is to get that all to be safe Rust. By the time that's done, it should be much more difficult to find an exploit in that toolchain.

The first implementation of Mog used LLVM as the compiler. This can produce somewhat faster code than the newer compiler, due to having a wide array of optimizations, but it had two major issues. First, the compile time wasn't great. The new compiler has compile times that are not quite as good as Go's, but within an order of magnitude for programs under 1000 lines, which is to say still quite fast, and generally fast enough that the start time for one-off scripts isn't painful. Mog does not claim to provide zero-cost abstractions, nor to provide arbitrary opportunities for low-level optimization. It does compile to native code, but a cracked low-level programmer will be able to squeeze more juice out of a C or C++ program.

The second big issue with using LLVM is that for Mog, the compiler itself is part of the trusted computing base. If the compiler has a security flaw, your agent has a security flaw. This means we can't use an enormous, sprawling codebase like LLVM's. We have to have a small codebase that we control and that can be audited.

## Current Status

Mog was created entirely using the [Volt](https://github.com/Martian-Engineering/volt) coding agent, the vast majority of which used a single continuous session spanning over three weeks, using Voltropy's [Lossless Context Management](https://papers.voltropy.com/LCM) to maintain its memory after compactions. This session is still running, porting the QBE compiler to safe Rust. The models used were Claude Opus 4.6, Kimi k2.5, and GLM-4.7.

Lots of work remains to be done standardize functions that the host provides to Mog programs, to keep things standard. That should include a lot of what the Python standard library provides, like support for JSON, CSV, Sqlite, POSIX filesystem and networking operations, etc. The Mog standard library should also provide async functions for calling LLMs. I suspect high-level interfaces, like Simon Willison's `llm` CLI tool or `DSPy` modules, would be useful, as well as low-level interfaces that allow fine-grained context management.

To be clear: Mog has *not* been audited, and it is presented without security guarantees. It should be possible to secure it, but that work has not yet been done. There are some tests that check that a malicious Mog program that tries to access host functionality that the host does not want to provide cannot in fact achieve access, but this design has enough security attack surface to warrant careful scrutiny.

Even without unimpeachable security properties, Mog should already be useful for extending your AI agents *with their own code* -- they're not trying to pwn themselves. So for now, beware of other people trying to Mog your agent, and I hope Mog is a useful tool for all you agent runners out there.
