#!/usr/bin/env python3
"""
expand_all_parts.py: Generates comprehensive, rigorous chapters for the NumLang book monograph.
"""

import os

ROOT = os.path.dirname(os.path.abspath(__file__))
CHAPTERS = os.path.join(ROOT, "chapters")

def write_chapter(filename, content):
    path = os.path.join(CHAPTERS, filename)
    with open(path, "w", encoding="utf-8") as f:
        f.write(content.strip() + "\n")
    print(f"Wrote {filename} ({len(content.splitlines())} lines)")

def main():
    print("Writing expanded chapters...")

if __name__ == "__main__":
    main()
