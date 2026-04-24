#!/usr/bin/env python3
"""
Extract files from Ollama / agent output that uses `// path:` or `# path:` headers.
Supports: .rs (// path:), .toml/.yaml/.yml/.sh/.Dockerfile/.gitignore/.dockerignore (# path:),
         .md (<!-- path: -->)
Usage:
  python3 scripts/extract_files.py --input run_artifacts/phase1/hermes_output.md --outdir ./
"""
import argparse
import re
import os
import sys


def main():
    parser = argparse.ArgumentParser(description="Extract files from generated output")
    parser.add_argument("--input", "-i", required=True, help="Markdown input file")
    parser.add_argument("--outdir", "-o", default=".", help="Output directory prefix")
    args = parser.parse_args()

    with open(args.input, "r", encoding="utf-8") as f:
        content = f.read()

    # Split by code fence blocks
    blocks = re.split(r"\n```(?:[a-zA-Z0-9]*)\n|```\n", content)

    extracted = 0
    for block in blocks:
        block = block.strip()
        if not block:
            continue

        # Try each path header pattern
        path = None
        # Rust: // path: arb_bot/crates/arb-core/src/lib.rs
        m = re.search(r"//\s*path:\s*(\S+)", block)
        if m:
            path = m.group(1)
        else:
            # Toml/Yaml/Shell/Dockerfile/Gitignore: # path: Cargo.toml
            m = re.search(r"#\s*path:\s*(\S+)", block)
            if m:
                path = m.group(1)
            else:
                # Markdown: <!-- path: relative/path/to/file.md -->
                m = re.search(r"<!--\s*path:\s*(\S+)\s*-->", block)
                if m:
                    path = m.group(1)

        if not path:
            continue

        # Remove the header line from the file content
        lines = block.splitlines()
        if lines and any(h in lines[0] for h in ("path:",)):
            # More robust: remove any initial lines that match header patterns
            cleaned_lines = []
            header_found = False
            for line in lines:
                if not header_found and (re.match(r"^\s*//\s*path:", line) or
                                          re.match(r"^\s*#\s*path:", line) or
                                          re.match(r"^\s*<!--\s*path:", line)):
                    header_found = True
                    continue
                cleaned_lines.append(line)
            body = "\n".join(cleaned_lines).strip("\n") + "\n"
        else:
            body = block

        out_path = os.path.join(args.outdir, path)
        os.makedirs(os.path.dirname(out_path), exist_ok=True)
        with open(out_path, "w", encoding="utf-8") as f:
            f.write(body)
        extracted += 1
        print(f"Extracted: {path}")

    print(f"\nTotal files extracted: {extracted}")
    return 0 if extracted else 1


if __name__ == "__main__":
    sys.exit(main())
