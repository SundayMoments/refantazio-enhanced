"""Build the local Metaphor fork using the installed VS Build Tools and SDK."""
import argparse
import json
import os
from pathlib import Path
import subprocess
import hashlib
import zipfile

ROOT = Path(__file__).resolve().parents[1]

def environment():
    # Some launcher environments contain both Path and PATH, which breaks MSBuild.
    env = {k.upper(): v for k, v in os.environ.items()}
    env['PYTHONPATH'] = str(ROOT / 'artifacts/build-libs')
    return env

def visual_studio():
    vswhere = Path(os.environ['ProgramFiles(x86)']) / 'Microsoft Visual Studio/Installer/vswhere.exe'
    installs = json.loads(subprocess.check_output([str(vswhere), '-latest', '-products', '*',
        '-requires', 'Microsoft.VisualStudio.Component.VC.Tools.x86.x64', '-format', 'json'], env=environment()))
    if not installs:
        raise RuntimeError('Visual Studio C++ Build Tools are required')
    return Path(installs[0]['installationPath'])

def run(command, log, **kwargs):
    log.parent.mkdir(parents=True, exist_ok=True)
    with log.open('w', encoding='utf-8') as output:
        result = subprocess.run(command, env=environment(), stdout=output, stderr=subprocess.STDOUT, **kwargs)
    print(log.read_text(encoding='utf-8', errors='replace')[-16000:])
    if result.returncode:
        raise SystemExit(result.returncode)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--tests', action='store_true')
    parser.add_argument('--dlss-tests', action='store_true', help='Build the real NGX runtime regression harness')
    parser.add_argument('--reshade', action='store_true', help='Build the offline ReShade loader')
    args = parser.parse_args()
    vs = visual_studio()
    if args.reshade:
        reshade = ROOT / 'upstream/Source/External/reshade'
        # Pinned source corresponding to the Luma 704 loader; keep local identity
        # deterministic even in a shallow checkout without upstream tags.
        (reshade / 'res/version.h').write_text('#pragma once\n'
            '#define VERSION_FULL 6.8.0.2\n#define VERSION_MAJOR 6\n#define VERSION_MINOR 8\n'
            '#define VERSION_REVISION 0\n#define VERSION_BUILD 2\n'
            '#define VERSION_STRING_FILE "6.8.0.2"\n'
            '#define VERSION_STRING_PRODUCT "6.8.0 UNOFFICIAL OFFLINE"\n')
        run([str(vs / 'MSBuild/Current/Bin/MSBuild.exe'), str(reshade / 'ReShade.vcxproj'),
            '/p:Configuration=Release', '/p:Platform=x64', '/p:PlatformToolset=v143',
            '/p:SolutionDir=' + str(reshade) + '\\', '/p:GladReproducible=true',
            '/p:PreBuildEventUseInBuild=false',
            '/v:minimal', '/nologo'], ROOT / 'research/build-reshade.log', cwd=reshade)
    elif args.dlss_tests:
        out = ROOT / 'artifacts/tests'
        out.mkdir(parents=True, exist_ok=True)
        ngx = ROOT / 'upstream/Source/External/NGX'
        batch = out / 'compile-dlss.cmd'
        batch.write_text(f'@echo off\ncall "{vs / "VC/Auxiliary/Build/vcvars64.bat"}" >nul\n'
            f'cl /nologo /std:c++latest /EHsc /O2 /MD /DNDEBUG /I"{ngx}" '
            f'/I"{ROOT / "upstream/Source/External/reshade"}" '
            f'"{ROOT / "tests/dlss_runtime.cpp"}" /Fe:"{out / "dlss_runtime.exe"}" '
            f'/Fo:"{out / "dlss_runtime.obj"}" "{ngx / "libs/nvsdk_ngx_d.lib"}" '
            'd3d11.lib dxgi.lib advapi32.lib user32.lib\nexit /b %errorlevel%\n')
        release = json.loads((ROOT / 'sources.lock.json').read_text())['release']
        archive = ROOT / 'downloads' / release['filename']
        if hashlib.sha256(archive.read_bytes()).hexdigest() != release['sha256']:
            raise RuntimeError('DLSS runtime archive digest mismatch')
        with zipfile.ZipFile(archive) as zipped:
            (out / 'nvngx_dlss.dll').write_bytes(zipped.read('nvngx_dlss.dll'))
        run(['cmd.exe', '/d', '/c', str(batch)], ROOT / 'research/build-dlss-tests.log', cwd=out)
    elif args.tests:
        out = ROOT / 'artifacts/tests'
        out.mkdir(parents=True, exist_ok=True)
        # This generated batch file has fixed compiler arguments and quoted absolute paths.
        batch = out / 'compile.cmd'
        batch.write_text(f'@echo off\ncall "{vs / "VC/Auxiliary/Build/vcvars64.bat"}" >nul\n'
            f'cl /nologo /std:c++20 /EHsc /O2 /W4 "{ROOT / "tests/temporal_gpu.cpp"}" '
            f'/Fe:"{out / "temporal_gpu.exe"}" /Fo:"{out / "temporal_gpu.obj"}" '
            'd3d11.lib dxgi.lib d3dcompiler.lib\nif errorlevel 1 exit /b %errorlevel%\n'
            f'cl /nologo /O2 /c /TC /I"{ROOT / "vendor/safetyhook"}" '
            f'"{ROOT / "vendor/safetyhook/Zydis.c"}" /Fo:"{out / "Zydis.obj"}"\n'
            'if errorlevel 1 exit /b %errorlevel%\n'
            f'cl /nologo /std:c++latest /EHsc /O2 "{ROOT / "tests/engine.cpp"}" '
            f'"{ROOT / "vendor/safetyhook/safetyhook.cpp"}" "{out / "Zydis.obj"}" '
            f'/Fe:"{out / "engine.exe"}"\nexit /b %errorlevel%\n')
        run(['cmd.exe', '/d', '/c', str(batch)], ROOT / 'research/build-tests.log', cwd=out)
    else:
        run([str(vs / 'MSBuild/Current/Bin/MSBuild.exe'),
            str(ROOT / 'upstream/Source/Games/Metaphor ReFantazio/Metaphor ReFantazio.vcxproj'),
            '/p:Configuration=Publishing-Release', '/p:Platform=x64', '/p:PlatformToolset=v143',
            '/p:SolutionName=Luma', '/p:SolutionDir=' + str(ROOT / 'upstream') + '\\',
            '/p:PostBuildEventUseInBuild=false', '/v:minimal', '/nologo'], ROOT / 'research/build-fork.log')

if __name__ == '__main__':
    main()
