"""Apply or restore Metaphor's per-user, per-executable DPI compatibility setting."""
import argparse
import json
from pathlib import Path
import winreg

ROOT = Path(__file__).resolve().parents[1]
EXE = r'C:\Program Files (x86)\Steam\steamapps\common\METAPHOR\METAPHOR.exe'
KEY = r'Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers'
BACKUP = ROOT / 'artifacts/display-config/dpi-override-backup.json'


def read(key):
    try:
        return winreg.QueryValueEx(key, EXE)
    except FileNotFoundError:
        return None, winreg.REG_SZ


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--restore', action='store_true')
    args = parser.parse_args()
    if not Path(EXE).is_file():
        raise RuntimeError('Named game executable missing')
    with winreg.CreateKeyEx(winreg.HKEY_CURRENT_USER, KEY, 0, winreg.KEY_READ | winreg.KEY_WRITE) as key:
        current, value_type = read(key)
        if args.restore:
            saved = json.loads(BACKUP.read_text())
            if saved['exe'] != EXE or saved['key'] != KEY or current != saved['applied']:
                raise RuntimeError('Compatibility setting changed since installation; refusing overwrite')
            if saved['previous'] is None:
                winreg.DeleteValue(key, EXE)
            else:
                winreg.SetValueEx(key, EXE, 0, saved['type'], saved['previous'])
            print('Restored original Metaphor DPI compatibility setting.')
            return
        if value_type != winreg.REG_SZ:
            raise RuntimeError('Unexpected compatibility value type')
        tokens = (current or '').split()
        if any(t in tokens for t in ['DPIUNAWARE', 'GDIDPISCALING']):
            raise RuntimeError('Conflicting DPI override requires review')
        if 'HIGHDPIAWARE' in tokens:
            print('Metaphor already has the application DPI override.')
            return
        if not tokens:
            tokens = ['~']
        tokens.append('HIGHDPIAWARE')
        applied = ' '.join(tokens)
        if BACKUP.exists():
            raise RuntimeError('Existing backup found; refusing to overwrite recovery information')
        BACKUP.parent.mkdir(parents=True, exist_ok=True)
        BACKUP.write_text(json.dumps({'exe': EXE, 'key': KEY, 'previous': current,
            'type': value_type, 'applied': applied}, indent=2))
        winreg.SetValueEx(key, EXE, 0, winreg.REG_SZ, applied)
        if read(key)[0] != applied:
            raise RuntimeError('Registry read-back did not match')
        print('Applied and verified for METAPHOR.exe only: ' + applied)
        print('Backup: ' + str(BACKUP))


if __name__ == '__main__':
    main()
