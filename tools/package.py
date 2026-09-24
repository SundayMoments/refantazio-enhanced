"""Create a reproducible local comparison package from the pinned upstream release."""
from pathlib import Path
import difflib
import hashlib
import json
import shutil
import subprocess
import zipfile

ROOT = Path(__file__).resolve().parents[1]
PIN = '3d48c38c17a59136be068e1f33bd25a92be69089'
SHA256 = 'b03268582488c1edf6e539b1a3e25d0d6c92b251be2bcd05bbd2c47725070373'

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    archive = ROOT / 'downloads/Luma-Metaphor_ReFantazio-latest-704.zip'
    if digest(archive) != SHA256:
        raise RuntimeError('Baseline package digest mismatch')
    upstream = ROOT / 'upstream'
    subprocess.run(['git', '-C', str(upstream), 'merge-base', '--is-ancestor', PIN, 'HEAD'], check=True)
    dest = ROOT / 'artifacts/fork'
    dest.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(archive) as zipped:
        for item in zipped.infolist():
            if not (dest / item.filename).resolve().is_relative_to(dest.resolve()):
                raise RuntimeError('Unsafe archive path')
        zipped.extractall(dest)
    shutil.copy2(upstream / 'Binaries/x64-Publishing-Release/Luma-Metaphor ReFantazio.addon', dest)
    reshade = upstream / 'Source/External/reshade'
    offline_loader = reshade / 'bin/x64/Release/ReShade64.dll'
    if not offline_loader.is_file():
        raise RuntimeError('Build the offline loader with tools/build.py --reshade first')
    shutil.copy2(offline_loader, dest / 'dxgi.dll')
    for name in ['Luma_PrepareMotionVector.hlsl', 'Luma_CreateBiasMask.hlsl', 'Luma_TemporalAADepth.hlsl', 'Luma_CopyDsrResult.hlsl']:
        shutil.copy2(upstream / 'Shaders/Metaphor ReFantazio' / name, dest / 'Luma/Metaphor ReFantazio' / name)
    (dest / 'README-FORK.txt').write_text(
        'ReFantazio Enhanced - experimental 0.2\n'
        'Based on Luma build 704, by Idarion and Filippo Tarpini/Pumbo.\n'
        'Engine enhancements adapted from Lyall / MetaphorFix (MIT).\n'
        'Preserves upstream credits and licensing. See Luma/licenses.\n'
        'Local ReShade build: no update checks, browser links, or screenshot command execution.\n'
        'No NGX update requests; proprietary NVIDIA runtime/driver remains a separate trust boundary.\n'
        'Home > Luma > Engine enhancements: changes apply after restarting.\n'
        'Menu cap fix respects the game FPS setting; borderless follows Windows refresh rate.\n'
        'Use DLSS at 100% Rendering Scale for native-resolution DLAA.\n'
        'Home > Luma: texture detail bias 0 favors stability; -1 matches upstream.\n'
        'Particle history rejection is experimental and can be disabled there.\n'
        'Synthetic GPU correctness tests passed; comparative gameplay quality is not yet established.\n')
    shutil.copy2(upstream / 'LICENSE.md', dest / 'LUMA-LICENSE.txt')
    license_dir = dest / 'Luma/licenses'
    license_dir.mkdir(parents=True, exist_ok=True)
    for license in (ROOT / 'licenses').glob('*'):
        if license.is_file(): shutil.copy2(license, license_dir / license.name)
    shutil.copy2(ROOT / 'research/offline-audit.md', dest / 'Luma/OFFLINE-AUDIT.md')
    shutil.copy2(ROOT / 'research/dlss-audit.md', dest / 'Luma/dlss-audit.md')
    shutil.copy2(ROOT / 'CREDITS.md', dest / 'Luma/CREDITS.md')
    shutil.copy2(ROOT / 'LICENSE.md', dest / 'Luma/LICENSE.md')
    payload = {str(p.relative_to(dest)).replace('\\', '/'): digest(p) for p in sorted(dest.rglob('*')) if p.is_file()}
    (ROOT / 'artifacts/fork-manifest.json').write_text(json.dumps({'upstream': PIN, 'files': payload}, indent=2))
    patch_dir = ROOT / 'patches'; patch_dir.mkdir(exist_ok=True)
    added = subprocess.check_output(['git', '-C', str(upstream), 'ls-files', '--others', '--exclude-standard',
        'Source/Games/Metaphor ReFantazio'], text=True, encoding='utf-8').splitlines()
    # Let Git encode missing final newlines and paths for existing files.
    changes = [subprocess.check_output(['git', '-C', str(upstream), 'diff', '--binary', PIN, '--', '.', ':!Source/External/reshade'], text=True, encoding='utf-8')]
    for name in added:
        new = (upstream / name).read_text(encoding='utf-8')
        changes.extend(difflib.unified_diff([], new.splitlines(True),
            fromfile='/dev/null', tofile='b/' + name + '\t'))
    patch = patch_dir / 'temporal-quality.patch'
    patch.write_text(''.join(changes), encoding='utf-8', newline='\n')
    subprocess.run(['git', '-C', str(upstream), 'apply', '--reverse', '--check', str(patch)], check=True)
    reshade_patch = patch_dir / 'reshade-offline.patch'
    reshade_patch.write_bytes(subprocess.check_output(['git', '-C', str(reshade), 'diff', '--binary',
        'eeb2c76aea8e00200d88b479c9036c5ea4d06d5e', '--', '.', ':!deps/glad']))
    subprocess.run(['git', '-C', str(reshade), 'apply', '--reverse', '--check', str(reshade_patch)], check=True)
    shutil.make_archive(str(ROOT / 'artifacts/ReFantazio-Enhanced-0.2'), 'zip', dest)
    print(f'Packaged {len(payload)} files; upstream {PIN}; source patch saved.')

if __name__ == '__main__':
    main()
