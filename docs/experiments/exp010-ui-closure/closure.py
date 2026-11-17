"""Static closure of Zed-workspace-internal crates reachable from given start crates.

Usage: closure.py <zed-root> <start>[,<start>...] [--upper] [--minus <base>[,<base>...]]
  --upper : also follow optional deps, build-dependencies and ALL target-specific deps
            (an upper bound: features are not resolved, so anything that *could* be enabled counts)
  --minus : subtract the closure of the base crates (computed in NORMAL mode unless --minus-upper,
            so "ui --upper --minus base" is a STATIC ESTIMATE of what ui may add over a plain base build;
            base cfgs are only approximated, so this is not a guaranteed upper bound)
Only internal (path / workspace-path) crates are expanded; external crates are only counted.
"""
import sys, tomllib, os

def load(root):
    ws = tomllib.load(open(os.path.join(root, "Cargo.toml"), "rb"))
    return ws["workspace"]["dependencies"]

def dep_tables(t, upper):
    tabs = [t.get("dependencies", {})]
    if upper:
        tabs.append(t.get("build-dependencies", {}))
    for k, v in t.get("target", {}).items():
        # normal mode: only cfgs that positively name macOS (skip any "not(" so cfg(not(target_os="macos")) is excluded)
        if upper or ("not(" not in k and any(x in k for x in ("macos", "unix", "apple"))):
            tabs.append(v.get("dependencies", {}))
            if upper:
                tabs.append(v.get("build-dependencies", {}))
    return tabs

def closure(root, starts, upper):
    wsdeps = load(root)
    def path_of(name):
        d = wsdeps.get(name)
        return os.path.join(root, d["path"]) if isinstance(d, dict) and "path" in d else None
    seen, ext = {}, set()
    def visit(name, p):
        if name in seen:
            return
        t = tomllib.load(open(os.path.join(p, "Cargo.toml"), "rb"))
        lic = t.get("package", {}).get("license", "?")
        n = 0
        for dp, _, fs in os.walk(os.path.join(p, "src")):
            for f in fs:
                if f.endswith(".rs"):
                    n += sum(1 for _ in open(os.path.join(dp, f), errors="ignore"))
        seen[name] = (lic, n)
        for tab in dep_tables(t, upper):
            for dn, dv in tab.items():
                if isinstance(dv, dict) and dv.get("optional") and not upper:
                    continue
                real = dv.get("package", dn) if isinstance(dv, dict) else dn
                pp = None
                if isinstance(dv, dict) and dv.get("workspace"):
                    pp = path_of(real)
                elif isinstance(dv, dict) and "path" in dv:
                    pp = os.path.normpath(os.path.join(p, dv["path"]))
                if pp:
                    visit(real, pp)
                else:
                    ext.add(real)
    for s in starts:
        visit(s, path_of(s))
    return seen, ext

def main():
    a = sys.argv[1:]
    root, starts = a[0], a[1].split(",")
    upper = "--upper" in a
    base = a[a.index("--minus") + 1].split(",") if "--minus" in a else []
    seen, ext = closure(root, starts, upper)
    if base:
        bseen, bext = closure(root, base, "--minus-upper" in a)
        seen = {k: v for k, v in seen.items() if k not in bseen}
        ext = ext - bext
    tot = 0
    for k, (l, n) in sorted(seen.items(), key=lambda x: -x[1][1]):
        print(f"{k:28s} {l:20s} {n}")
        tot += n
    mode = "upper" if upper else "normal"
    bmode = "upper" if "--minus-upper" in a else "normal"
    print(f"mode={mode} starts={starts} minus={base} (base mode={bmode})")
    print("internal crates:", len(seen), "lines:", tot, "external deps (not expanded):", len(ext))
    print("GPL:", sorted(k for k, (l, _) in seen.items() if "GPL" in l))
    hits = sorted(k for k in seen if k in ("settings", "editor", "workspace", "project", "language", "client", "theme_settings"))
    print("heavy-crate hits:", hits)

main()
