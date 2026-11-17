"""Check that every relative Markdown link in the given files points at an existing file."""
import os, re, sys
bad = 0; n = 0
for f in sys.argv[1:]:
    for m in re.finditer(r'\]\(([^)#\s]+)(#[^)]*)?\)', open(f, encoding='utf-8').read()):
        t = m.group(1)
        if re.match(r'[a-z]+:', t): continue
        n += 1
        p = os.path.normpath(os.path.join(os.path.dirname(f), t))
        ok = os.path.exists(p); bad += not ok
        print(('OK  ' if ok else 'MISSING ') + f + ' -> ' + t)
print(f'{n} links, {bad} missing'); sys.exit(1 if bad else 0)
