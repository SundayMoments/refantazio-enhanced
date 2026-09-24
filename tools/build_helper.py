"""Build the portable Rust helper with a verified, embedded offline payload.

Dependency provisioning is explicit (cargo fetch); this command never downloads.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import zipfile
from runtime import CRT_FILES, required_version, system_directory, validate_directory

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'artifacts/helper'
VERSION = '0.2.4'
GAME_SHA256 = '548ddc955c176867f062c44f94c03dd9ac33caefb278a5ac388e792aedf09504'

def digest(b):
    return hashlib.sha256(b).hexdigest()

def cargo(*args, **kwargs):
    return subprocess.check_output(['cargo', *args, '--offline', '--locked', '--manifest-path', str(ROOT / 'helper/Cargo.toml')], **kwargs)

def rust_notices():
    metadata = json.loads(cargo('metadata', '--format-version', '1', '--filter-platform', 'x86_64-pc-windows-msvc'))
    ids = {n['id'] for n in metadata['resolve']['nodes']}
    packages = [p for p in metadata['packages'] if p['id'] in ids and p['name'] != 'refantazio-enhanced-helper']
    apache = next(Path(p['manifest_path']).parent / 'LICENSE-APACHE' for p in packages if (Path(p['manifest_path']).parent / 'LICENSE-APACHE').is_file()).read_text(encoding='utf-8')
    boost = (ROOT / 'licenses/SafetyHook-Boost.txt').read_text(encoding='utf-8')
    result = ['Rust helper dependency notices\n\nPackages with an Apache-2.0 alternative may be used under that alternative.\n']
    for p in sorted(packages, key=lambda p: (p['name'], p['version'])):
        result.append(f"\n{'=' * 72}\n{p['name']} {p['version']} — {p['license']}\n{p.get('repository') or ''}\n")
        crate = Path(p['manifest_path']).parent
        files = sorted(f for f in crate.rglob('*') if f.is_file() and f.name.lower().startswith(('license', 'copying', 'notice', 'ofl', 'ufl')) and f.suffix.lower() in ('', '.md', '.txt'))
        if not files:
            if 'Apache-2.0' in (p['license'] or ''): result.append(apache)
            elif p['license'] == 'BSL-1.0': result.append(boost)
            else: raise RuntimeError('Missing license text: ' + p['name'])
        for f in files:
            result.append('\n' + f.relative_to(crate).as_posix() + '\n' + f.read_text(encoding='utf-8'))
    return '\n'.join(result)

def native_notices():
    base = ROOT / 'upstream/Source/External'
    paths = [base / 'DKUtil/LICENSE', base / 'NVAPI/License.txt', base / 'FidelityFX/FidelityFX/host/backends/dx12/license.txt']
    deps = base / 'reshade/deps'
    paths.extend(f for f in deps.rglob('*') if f.is_file() and f.name.lower().startswith(('license', 'copying')) and f.suffix.lower() in ('', '.md', '.txt', '.adoc'))
    def read(path):
        try: return path.read_text(encoding='utf-8-sig')
        except UnicodeDecodeError: return path.read_text(encoding='cp1252')
    return '\n\n'.join(f'{p.relative_to(base).as_posix()}\n\n{read(p)}' for p in sorted(set(paths)))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--crt-dir', type=Path, help='Installed release x64 Microsoft.VC143.CRT directory')
    parser.add_argument('--prepare-only', action='store_true')
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    candidates = [args.crt_dir] if args.crt_dir else []
    if not candidates:
        vswhere = Path(os.environ.get('ProgramFiles(x86)', 'C:/Program Files (x86)')) / 'Microsoft Visual Studio/Installer/vswhere.exe'
        vs = subprocess.check_output([str(vswhere), '-latest', '-products', '*', '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-property', 'installationPath'], text=True).strip()
        candidates = sorted((Path(vs) / 'VC/Redist/MSVC').glob('*/x64/Microsoft.VC143.CRT'), reverse=True)
        candidates.append(system_directory())
    base = ROOT / 'artifacts/fork'
    manifest = json.loads((ROOT / 'artifacts/fork-manifest.json').read_text())
    files = {}
    for name, expected in manifest['files'].items():
        path = (base / name).resolve()
        if not path.is_relative_to(base.resolve()): raise RuntimeError('Invalid base package path')
        data = path.read_bytes()
        if digest(data) != expected: raise RuntimeError('Base package digest mismatch: ' + name)
        files[name] = data
    minimum = required_version(files['Luma-Metaphor ReFantazio.addon'])
    crt = None
    for candidate in candidates:
        try:
            versions = validate_directory(candidate, minimum)
        except (OSError, RuntimeError) as error:
            print('Skipped runtime candidate:', error)
            continue
        crt = candidate
        break
    if crt is None:
        raise RuntimeError(f'Provide a licensed, signed x64 Microsoft runtime >= {minimum} with --crt-dir')
    print('Runtime requirement:', minimum, '; bundled runtime:', versions['msvcp140.dll'])
    for name in CRT_FILES:
        files[name] = (crt / name).read_bytes()
    # Ensure the distributable DLLs are unmodified, signed Microsoft release files.
    env = os.environ.copy()
    env['RE_CRT_DIR'] = str(crt.resolve())
    subprocess.run([shutil.which('pwsh') or 'powershell', '-NoProfile', '-Command', "$ErrorActionPreference = 'Stop'; $names = 'msvcp140.dll','msvcp140_atomic_wait.dll','vcruntime140.dll','vcruntime140_1.dll'; foreach ($name in $names) { $s = Get-AuthenticodeSignature -LiteralPath (Join-Path $env:RE_CRT_DIR $name); if ($s.Status -ne 'Valid' -or $s.SignerCertificate.Subject -notmatch 'Microsoft Corporation') { throw 'Runtime signature validation failed' } }"], env=env, check=True)
    for license in (ROOT / 'licenses').glob('*.txt'):
        files['Luma/licenses/' + license.name] = license.read_bytes()
    files['Luma/licenses/Rust-dependencies.txt'] = rust_notices().encode('utf-8')
    files['Luma/licenses/Native-dependencies.txt'] = native_notices().encode('utf-8')
    for name in ('CREDITS.md', 'LICENSE.md'):
        files['Luma/' + name] = (ROOT / name).read_bytes()
    files['Luma/OFFLINE-AUDIT.md'] = (ROOT / 'research/offline-audit.md').read_bytes()
    files['README-FORK.txt'] = (f'ReFantazio Enhanced {VERSION} — experimental\n\n' + (ROOT / 'docs/HELPER.md').read_text(encoding='utf-8')).encode('utf-8')
    for required in ('NVIDIA-DLSS.txt', 'Microsoft-VC-Runtime.txt'):
        if 'Luma/licenses/' + required not in files: raise RuntimeError('Missing runtime notice: ' + required)
    payload = OUT / 'payload.zip'
    with zipfile.ZipFile(payload, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name, data in sorted(files.items()):
            info = zipfile.ZipInfo(name, (2026, 9, 23, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            z.writestr(info, data)
    manifest_path = OUT / 'manifest.json'
    manifest_path.write_text(json.dumps({'version': VERSION, 'game_sha256': GAME_SHA256, 'files': {n: digest(b) for n, b in sorted(files.items())}}, indent=2), encoding='utf-8')
    notices = OUT / 'NOTICE.txt'
    notices.write_text('\n\n'.join(files[n].decode('utf-8') for n in ['Luma/CREDITS.md', 'Luma/LICENSE.md', *sorted(n for n in files if n.startswith('Luma/licenses/') and n.endswith('.txt'))]), encoding='utf-8')
    print(f'Prepared {len(files)} verified files ({payload.stat().st_size / 1048576:.1f} MiB compressed).')
    if args.prepare_only: return
    env.update(RE_HELPER_PAYLOAD=str(payload), RE_HELPER_MANIFEST=str(manifest_path), RE_HELPER_NOTICES=str(notices))
    # Unit separator avoids treating spaces in profile/workspace paths as separate flags.
    profile = str(Path.home())
    flags = ['-C', 'target-feature=+crt-static', '--remap-path-prefix=' + str(ROOT) + '=project', '--remap-path-prefix=' + profile + '=build-user', '--remap-path-prefix=' + profile.replace('\\', '/') + '=build-user']
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    subprocess.run(['cargo', 'build', '--release', '--target', 'x86_64-pc-windows-msvc', '--offline', '--locked', '--features', 'bundled', '--manifest-path', str(ROOT / 'helper/Cargo.toml')], env=env, check=True)
    release = ROOT / 'artifacts/distribution'
    release.mkdir(exist_ok=True)
    exe = release / f'ReFantazio-Enhanced-Helper-{VERSION}.exe'
    shutil.copy2(ROOT / 'helper/target/x86_64-pc-windows-msvc/release/ReFantazio-Enhanced-Helper.exe', exe)
    subprocess.run([str(exe), '--verify-payload'], check=True)
    private = profile.lower().encode()
    for name, data in [('helper', exe.read_bytes()), *files.items()]:
        if private in data.lower() or profile.lower().encode('utf-16-le') in data.lower(): raise RuntimeError('Build profile leaked into ' + name)
    sha = exe.with_suffix('.exe.sha256')
    sha.write_text(f'{digest(exe.read_bytes())}  {exe.name}\n', encoding='ascii')
    shutil.copy2(notices, release / 'NOTICE.txt')
    print(f'Built and checked {exe.name}; SHA-256 saved alongside it.')

if __name__ == '__main__':
    main()
