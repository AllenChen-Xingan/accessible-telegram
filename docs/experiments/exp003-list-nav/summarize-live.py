#!/usr/bin/env python3
"""汇总人工试用的现场记录：按键到没到应用、有没有被认出、列表动没动。
用法：python3 summarize-live.py [out/live-*.jsonl]（不给就取最新一份）"""
import glob, json, os, sys
from collections import Counter

path = sys.argv[1] if len(sys.argv) > 1 else max(glob.glob("out/live-*.jsonl"), key=os.path.getmtime)
evs = [json.loads(l) for l in open(path, encoding="utf-8") if l.strip()]
print(f"记录：{path}，{len(evs)} 条")
for e in evs:
    if e.get("ev") == "start":
        print("模式：", e["mode"])
raw = [e for e in evs if e.get("ev") == "raw_key"]
named = Counter(e["name"] for e in raw if e.get("name"))
print("AppKit 收到的按键（本实验关心的）：", dict(named) or "无")
print("  其中绕过输入法交给 GPUI：", sum(1 for e in raw if e.get("bypass_ime")))
print("  收到时窗口不是 key window：", sum(1 for e in raw if e.get("name") and e.get("window_is_key") is not True))
gk = Counter((e["key"], e.get("action")) for e in evs if e.get("ev") == "gpui_keystroke")
print("GPUI 认出的按键 → 动作：", {f"{k}→{a}": n for (k, a), n in gk.items()} or "无")
print("列表实际移动次数（app_key）：", sum(1 for e in evs if e.get("ev") == "app_key"))
print("焦点进出列表：", Counter(e["ev"] for e in evs if e.get("ev") in ("focus", "blur")))
print("读屏发来的动作：", Counter(e["action"] for e in evs if e.get("ev") == "a11y_action") or "无")
