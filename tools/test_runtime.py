"""Compile and run compatibility checks with the exact packaged runtime DLLs."""
import json
from pathlib import Path
import subprocess
import zipfile
from build import ROOT, visual_studio, run
from runtime import CRT_FILES, required_version, validate_directory
from build_helper import digest


def main():
    out = ROOT / 'artifacts/runtime-tests'
    out.mkdir(parents=True, exist_ok=True)
    manifest = json.loads((ROOT / 'artifacts/helper/manifest.json').read_text())
    with zipfile.ZipFile(ROOT / 'artifacts/helper/payload.zip') as archive:
        minimum = required_version(archive.read('Luma-Metaphor ReFantazio.addon'))
        for name in CRT_FILES:
            data = archive.read(name)
            if digest(data) != manifest['files'][name]:
                raise RuntimeError('Runtime payload hash mismatch: ' + name)
            (out / name).write_bytes(data)
    validate_directory(out, minimum)
    vs = visual_studio()
    batch = out / 'compile.cmd'
    batch.write_text(f'@echo off\ncall "{vs / "VC/Auxiliary/Build/vcvars64.bat"}" >nul\n'
        f'cl /nologo /std:c++20 /EHsc /O2 /MD /DNDEBUG "{ROOT / "tests/runtime_compatibility.cpp"}" '
        f'/Fe:"{out / "runtime_compatibility.exe"}" /Fo:"{out / "runtime_compatibility.obj"}"\nexit /b %errorlevel%\n')
    run(['cmd.exe', '/d', '/c', str(batch)], ROOT / 'research/build-runtime-tests.log', cwd=out)
    subprocess.run([str(out / 'runtime_compatibility.exe')], cwd=out, check=True, timeout=30)


if __name__ == '__main__':
    main()
