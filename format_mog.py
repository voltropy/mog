#!/usr/bin/env python3

import re

def format_mog(filepath):
    with open(filepath, 'r') as f:
        lines = f.read().splitlines()

    indent_level = 0
    formatted = []

    for line in lines:
        stripped = line.strip()
        if not stripped:
            # Keep empty lines
            formatted.append('')
            continue

        # First, count braces in this line to determine indent BEFORE outputting
        open_before = 0
        close_before = 0

        # Character-by-character tracking
        chars = list(stripped)
        i = 0
        while i < len(chars):
            if chars[i] == '{':
                open_before += 1
            elif chars[i] == '}':
                close_before += 1
            i += 1

        # If we close more than we open, decrease indent before this line
        if close_before > open_before:
            indent_level = max(0, indent_level - (close_before - open_before))

        # Determine indent for this line
        line_indent = indent_level * 2

        # Output with indent
        formatted.append(' ' * line_indent + stripped)

        # Update indent level for next line based on net braces
        net_braces = open_before - close_before
        indent_level += net_braces
        indent_level = max(0, indent_level)

    # Remove trailing whitespace
    formatted = [line.rstrip() for line in formatted]

    # Remove consecutive empty lines (but keep at most one)
    cleaned = []
    prev_empty = False
    for line in formatted:
        if not line.strip():
            if not prev_empty:
                cleaned.append('')
            prev_empty = True
        else:
            cleaned.append(line)
            prev_empty = False

    # Remove leading empty lines
    while cleaned and not cleaned[0].strip():
        cleaned.pop(0)

    # Remove trailing empty lines
    while cleaned and not cleaned[-1].strip():
        cleaned.pop()

    # Write back
    with open(filepath, 'w') as f:
        f.write('\n'.join(cleaned))
        if cleaned:
            f.write('\n')

    print(f"Formatted {filepath}")

# Process all .mog files
import glob
for filepath in sorted(glob.glob('./*.mog')):
    try:
        format_mog(filepath)
    except Exception as e:
        print(f"Error formatting {filepath}: {e}")

print("All files formatted!")