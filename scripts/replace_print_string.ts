#!/usr/bin/env bun
/**
 * Replace print_string() calls whose string argument ends with \n
 * with println() calls (without the trailing \n).
 *
 * Handles:
 *   print_string("text\n")     → println("text")
 *   print_string(f"text\n")    → println(f"text")
 *   print_string("\n")         → println("")
 *
 * Does NOT touch:
 *   print_string("text")       — no trailing \n
 *   print_string(variable)     — not a string literal
 */

import { readFileSync, writeFileSync } from "fs";
import { globSync } from "fs";

const args = process.argv.slice(2);
const dryRun = args.includes("--dry-run");

// Match print_string( optional-f "..." ) where the string ends with \n"
// Group 1: the f prefix (if any)
// Group 2: the string content before \n
const pattern = /print_string\((f?)"((?:[^"\\]|\\.)*)\\n"\)/g;

function processFile(path: string): number {
  const original = readFileSync(path, "utf-8");
  let count = 0;

  const replaced = original.replace(pattern, (_match, fPrefix, content) => {
    count++;
    return `println(${fPrefix}"${content}")`;
  });

  if (count > 0) {
    if (!dryRun) {
      writeFileSync(path, replaced);
    }
    console.log(`  ${path}: ${count} replacement${count > 1 ? "s" : ""}`);
  }
  return count;
}

// Find all .mog and .ts files
const files = [
  ...new Bun.Glob("**/*.mog").scanSync({ cwd: ".", absolute: true }),
  ...new Bun.Glob("src/**/*.ts").scanSync({ cwd: ".", absolute: true }),
];

if (dryRun) {
  console.log("DRY RUN — no files will be modified\n");
}

let totalReplacements = 0;
let filesModified = 0;

for (const file of files) {
  // Skip this script itself and node_modules
  if (file.includes("node_modules") || file.includes("replace_print_string")) continue;
  const count = processFile(file);
  if (count > 0) {
    filesModified++;
    totalReplacements += count;
  }
}

console.log(`\n${totalReplacements} replacements in ${filesModified} files${dryRun ? " (dry run)" : ""}`);
