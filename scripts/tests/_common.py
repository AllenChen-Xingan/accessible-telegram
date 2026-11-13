"""门槛自测共用的东西（issue #60）。每条检查在注入违规的副本上必须变红，原样必须是绿。

  python3 -m unittest discover -s scripts/tests -v

只会绿的检查等于没有（记忆 20261025-silent-check-failure）。
"""
from __future__ import annotations

import os
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
SIZE = os.path.join(REPO, "scripts", "check-file-size.py")
SOURCES = os.path.join(REPO, "scripts", "check-sources.py")
REVIEW = os.path.join(REPO, ".claude", "workflows", "atg-review.js")


def run(*args: str) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", *args], capture_output=True, text=True, timeout=120)


def git(cwd: str, *args: str) -> None:
    subprocess.run(["git", "-C", cwd, "-c", "user.name=t", "-c", "user.email=t@t", *args],
                   check=True, capture_output=True)
