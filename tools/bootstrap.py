"""Explicit development-only source setup. Never included in the game payload.

This command uses the network for pinned Git repositories, a hash-checked base
archive, and pinned code-generation dependencies. It refuses existing checkouts.
"""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import sys
import urllib.request

ROOT=Path(__file__).resolve().parents[1]

def run(*args,cwd=None):
    subprocess.run(list(map(str,args)),cwd=cwd,check=True)

def main():
    lock=json.loads((ROOT/'sources.lock.json').read_text())
    upstream=ROOT/'upstream'
    if upstream.exists():
        raise SystemExit('Existing upstream checkout left untouched. Use bootstrap only in a fresh clone of this repository.')
    for name,expected in lock['vendored_files'].items():
        if hashlib.sha256((ROOT/name).read_bytes()).hexdigest()!=expected:
            raise RuntimeError('Vendored source hash mismatch: '+name)
    run('git','clone','--filter=blob:none','--no-checkout',lock['luma']['url'],upstream)
    run('git','sparse-checkout','set','.github','Dependencies','External','Libraries','Scripts',
        'Shaders/Common','Shaders/Generic','Shaders/Includes','Shaders/Metaphor ReFantazio',
        'Source/Core','Source/External','Source/Games/Metaphor ReFantazio',cwd=upstream)
    run('git','checkout','-b','codex/super-mod',lock['luma']['revision'],cwd=upstream)
    run('git','submodule','update','--init','--depth','1','Source/External/reshade','Source/External/DKUtil',cwd=upstream)
    reshade=upstream/'Source/External/reshade'
    run('git','submodule','update','--init','--depth','1',cwd=reshade)
    revisions=dict(lock['submodules'])
    revisions['Source/External/reshade']=lock['reshade']['revision']
    for name,expected in revisions.items():
        actual=subprocess.check_output(['git','-C',str(upstream/name),'rev-parse','HEAD'],text=True).strip()
        if actual!=expected: raise RuntimeError('Dependency revision mismatch: '+name)
    run('git','apply','--check',ROOT/'patches/temporal-quality.patch',cwd=upstream)
    run('git','apply',ROOT/'patches/temporal-quality.patch',cwd=upstream)
    run('git','apply','--check',ROOT/'patches/reshade-offline.patch',cwd=reshade)
    run('git','apply',ROOT/'patches/reshade-offline.patch',cwd=reshade)
    release=lock['release']; downloads=ROOT/'downloads'; downloads.mkdir(exist_ok=True)
    archive=downloads/release['filename']
    if not archive.exists():
        request=urllib.request.Request(release['url'],headers={'User-Agent':'ReFantazio-local-build'})
        with urllib.request.urlopen(request,timeout=60) as response: data=response.read()
        if hashlib.sha256(data).hexdigest()!=release['sha256']: raise RuntimeError('Download hash mismatch')
        archive.write_bytes(data)
    if hashlib.sha256(archive.read_bytes()).hexdigest()!=release['sha256']: raise RuntimeError('Archive hash mismatch')
    run(sys.executable,'-m','pip','install','--target',ROOT/'artifacts/build-libs',*lock['python_build_dependencies'])
    print('Pinned sources ready. Build commands perform no downloads. Nothing installed into the game.')

if __name__=='__main__': main()
