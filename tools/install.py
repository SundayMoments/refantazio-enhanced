"""Install/restore this task's package only, with a per-file backup and hash checks."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
GAME = Path(r'C:\Program Files (x86)\Steam\steamapps\common\METAPHOR')
STATE = ROOT / 'artifacts/install-state.json'
BACKUP = ROOT / 'artifacts/original-files'
ROOT_FILES = {'dxgi.dll', 'nvngx_dlss.dll', 'Luma-Metaphor ReFantazio.addon', 'README-FORK.txt', 'LUMA-LICENSE.txt'}

def save_state(state):
    temporary = STATE.with_suffix('.new.json')
    temporary.write_text(json.dumps(state, indent=2))
    os.replace(temporary, STATE)

def matches_entry(target, entry):
    actual = digest(target) if target.is_file() else None
    accepted = {entry['installed']}
    if 'installed_before' in entry: accepted.add(entry['installed_before'])
    return actual in accepted

def atomic_copy(source, target):
    # Same-directory staging keeps each replacement atomic, even if copying fails.
    staged = target.with_name(target.name + '.refantazio-stage')
    if staged.exists():
        raise RuntimeError('A previous staging file exists; leaving it intact: ' + str(staged))
    owned = False
    try:
        with staged.open('xb') as output:
            owned = True
            with source.open('rb') as input: shutil.copyfileobj(input, output)
        if digest(staged) != digest(source): raise RuntimeError('Staged file hash mismatch')
        os.replace(staged, target)
    finally:
        if owned and staged.exists(): staged.unlink()

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def game_file(name):
    relative = Path(name)
    if relative.is_absolute() or '..' in relative.parts or not (name in ROOT_FILES or relative.parts[0] == 'Luma'):
        raise RuntimeError('Refusing path outside mod payload: ' + name)
    target = (GAME / relative).resolve()
    if not target.is_relative_to(GAME.resolve()):
        raise RuntimeError('Refusing path outside the named game directory')
    return target

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('action', choices=['fork', 'stable', 'baseline', 'restore'])
    args = parser.parse_args()
    running = subprocess.check_output(['tasklist', '/FI', 'IMAGENAME eq METAPHOR.exe', '/FO', 'CSV'], text=True)
    if 'METAPHOR.exe' in running:
        raise RuntimeError('Close Metaphor before switching builds')
    if not (GAME / 'METAPHOR.exe').is_file():
        raise RuntimeError('Game executable missing')
    if args.action == 'restore':
        state = json.loads(STATE.read_text())
        # Preflight everything before the first mutation. Never overwrite subsequent user edits.
        for name, entry in state['files'].items():
            target = game_file(name)
            restored_original = state.get('restoring') and entry['original'] and target.is_file() and digest(target) == entry['original']
            if target.exists() and not restored_original and not matches_entry(target, entry):
                raise RuntimeError('File changed after installation; leaving it untouched: ' + name)
            if entry['original'] and digest(BACKUP / name) != entry['original']:
                raise RuntimeError('Backup digest mismatch: ' + name)
        state['restoring'] = True
        save_state(state)
        for name, entry in state['files'].items():
            target = game_file(name)
            if entry['original']:
                atomic_copy(BACKUP / name, target)
            elif target.exists():
                target.unlink() # Individual allowlisted files only; no recursive deletion.
        state['restored'] = True
        save_state(state)
        print('Original files restored. Runtime-generated logs, cache and settings retained.')
        return
    payload = ROOT / 'artifacts' / args.action
    files = {str(p.relative_to(payload)).replace('\\', '/'): p for p in payload.rglob('*') if p.is_file()}
    if args.action == 'fork':
        for directory in [GAME, GAME / 'scripts', GAME / 'plugins']:
            if directory.is_dir() and any(p.name.lower() in {'metaphorfix.asi','metaphorfix.dll'} for p in directory.iterdir()):
                raise RuntimeError('A separate MetaphorFix is installed; resolve overlapping hooks before installing this combined addon')
    if args.action in {'fork', 'stable'}:
        expected = json.loads((ROOT / f'artifacts/{args.action}-manifest.json').read_text())['files']
        if set(expected) != set(files) or any(digest(files[n]) != h for n, h in expected.items()):
            raise RuntimeError('Package no longer matches its manifest')
    state = json.loads(STATE.read_text()) if STATE.exists() else {'created': datetime.now(timezone.utc).isoformat(), 'files': {}}
    if state.get('restored'):
        raise RuntimeError('This installation was restored; keep its backups and create a new session for another install')
    if state.get('restoring'):
        raise RuntimeError('A restore was interrupted; run restore again before installing')
    for name, source in files.items():
        target = game_file(name)
        if name in state['files']:
            if not matches_entry(target, state['files'][name]):
                raise RuntimeError('Installed file changed; refusing overwrite: ' + name)
        else:
            original = digest(target) if target.is_file() else None
            if original:
                backup = BACKUP / name; backup.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy2(target, backup)
            state['files'][name] = {'original': original, 'installed': digest(source)}
    state['variant'] = args.action
    # Persist recovery information before writing to the game directory.
    for name, source in files.items():
        target = game_file(name)
        state['files'][name]['installed_before'] = digest(target) if target.is_file() else None
        state['files'][name]['installed'] = digest(source)
    save_state(state)
    for name, source in files.items():
        target = game_file(name); target.parent.mkdir(parents=True, exist_ok=True)
        atomic_copy(source, target)
        if digest(target) != digest(source):
            raise RuntimeError('Installed hash mismatch: ' + name)
    for entry in state['files'].values(): entry.pop('installed_before', None)
    save_state(state)
    print(f'Installed and verified {len(files)} {args.action} files. Recovery manifest: {STATE}')

if __name__ == '__main__':
    main()
