"""Apply distributable patches to pristine pinned files, without fetching sources."""
from pathlib import Path
import json
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]

def main():
    lock = json.loads((ROOT / 'sources.lock.json').read_text())
    cases = [
        (ROOT / 'upstream', lock['luma']['revision'], 'temporal-quality.patch', ['.', ':!Source/External/reshade']),
        (ROOT / 'upstream/Source/External/reshade', lock['reshade']['revision'], 'reshade-offline.patch', ['.', ':!deps/glad']),
    ]
    base = (ROOT / 'artifacts').resolve()
    base.mkdir(exist_ok=True)
    temporary = tempfile.TemporaryDirectory(prefix='patch-check-', dir=base)
    scratch = Path(temporary.name).resolve()
    assert scratch.is_relative_to(base) and scratch != base
    try:
        for repo, pin, patch, paths in cases:
            target = scratch / patch.removesuffix('.patch')
            target.mkdir()
            names = subprocess.check_output(['git', '-C', str(repo), 'diff', '--name-only', pin, '--', *paths], text=True).splitlines()
            for name in names:
                old = subprocess.run(['git', '-C', str(repo), 'show', f'{pin}:{name}'], capture_output=True)
                if old.returncode: continue  # New files are supplied by the patch.
                file = (target / name).resolve()
                assert file.is_relative_to(target)
                file.parent.mkdir(parents=True, exist_ok=True)
                file.write_bytes(old.stdout)
            env = dict(os.environ, GIT_CEILING_DIRECTORIES=str(base))
            subprocess.run(['git', 'apply', str(ROOT / 'patches' / patch)], cwd=target, env=env, check=True)
            count = 0
            for file in target.rglob('*'):
                if not file.is_file(): continue
                current = repo / file.relative_to(target)
                assert file.read_bytes().replace(b'\r\n', b'\n') == current.read_bytes().replace(b'\r\n', b'\n'), str(current)
                count += 1
            print(f'PASS {patch}: applies to pinned source; {count} files reproduce working source')
    finally:
        assert scratch.is_relative_to(base) and scratch != base
        temporary.cleanup()

if __name__ == '__main__':
    main()
