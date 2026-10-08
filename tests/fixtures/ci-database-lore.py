#!/usr/bin/env python3
"""Isolated read/clone-only Lore fixture. Any commit or push is a test failure."""
import json
import pathlib
import sys

root = pathlib.Path(__file__).parent
args = sys.argv[1:]
for flag in ("--identity-token", "--access-token"):
    while flag in args:
        i = args.index(flag)
        del args[i:i + 2]
args = [arg for arg in args if arg not in ("--json", "--non-interactive", "--no-pager")]
with (root / "commands").open("a") as log:
    log.write(" ".join(args) + "\n")

def event(tag, data):
    print(json.dumps({"tagName": tag, "data": data}))

if args[:2] == ["repository", "list"]:
    event("repositoryListEntry", {"id": "11111111111111111111111111111111", "name": "db-test"})
elif "clone" in args:
    pathlib.Path(args[-1]).mkdir(parents=True, exist_ok=True)
elif args[-2:] == ["branch", "list"]:
    revision = (root / "head").read_text() if (root / "head").exists() else "a" * 64
    for branch in ("main", "release"):
        event("branchListEntry", {"id": branch + "-id", "name": branch, "location": "remote", "latest": revision})
elif "file" in args and "write" in args:
    source = root / "source.toml"
    if not source.exists():
        event("complete", {"status": 3})
        sys.exit(3)
    pathlib.Path(args[args.index("--output") + 1]).write_text(source.read_text())
elif args != ["--version"]:
    sys.exit(98)
event("complete", {"status": 0})
