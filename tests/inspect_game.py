"""Read-only validation of engine signatures against the supported retail executable.

Does not modify or redistribute game data. Reports addresses and verifies originals
used by each patch; runtime scanning additionally checks uniqueness after loading.
"""
from pathlib import Path
import argparse
import hashlib
import json
import re
import struct

ROOT = Path(__file__).resolve().parents[1]

def inspect(path):
    data = path.read_bytes()
    nt = struct.unpack_from('<I', data, 0x3c)[0]
    assert data[:2] == b'MZ' and data[nt:nt+4] == b'PE\0\0'
    count, stamp = struct.unpack_from('<HI', data, nt+6)
    optsize = struct.unpack_from('<H', data, nt+20)[0]
    image_size = struct.unpack_from('<I', data, nt+24+56)[0]
    assert (stamp, image_size) == (0x67ebd812, 0x14d49000), 'Unsupported executable'
    sections = []
    for i in range(count):
        pos = nt+24+optsize+40*i
        vsize, rva, rawsize, offset = struct.unpack_from('<IIII', data, pos+8)
        flags = struct.unpack_from('<I', data, pos+36)[0]
        sections.append((rva, offset, min(vsize, rawsize), flags))

    def offset_of(rva):
        for start, off, size, _ in sections:
            if start <= rva < start+size: return off+rva-start
        raise AssertionError('Target not in file-backed image')

    found = {}
    source = (ROOT/'src/engine_patterns.h').read_text()
    for name, pattern in re.findall(r'char (\w+)\[\] = "([^"]+)";', source):
        regex = b''.join(b'.' if '?' in b else re.escape(bytes.fromhex(b)) for b in pattern.split())
        matches = []
        for rva, off, size, flags in sections:
            if flags & 0x20000000:
                matches += [rva+m.start() for m in re.finditer(regex, data[off:off+size], re.S)]
        assert len(matches) == 1, f'{name}: expected one match, got {len(matches)}'
        found[name] = matches[0]

    def read(name, displacement, fmt):
        return struct.unpack_from(fmt, data, offset_of(found[name])+displacement)[0]

    for displacement in (3, 10): assert read('ShadowSize', displacement, '<i') == 2048
    for name, displacement, expected in [('KeyboardIcons',10,2), ('MouseIcons1',21,1), ('MouseIcons2',6,1), ('CameraShake',3,5)]:
        assert read(name, displacement, 'B') == expected, name
    lod_constant = found['LOD']+8+read('LOD',4,'<i')
    assert struct.unpack_from('<f',data,offset_of(lod_constant))[0] == 10000.0
    shadow_constant = found['ShadowTexel']+8+read('ShadowTexel',4,'<i')
    assert struct.unpack_from('<f',data,offset_of(shadow_constant))[0] == 1.0/2048
    assert read('GameplayFOV',11,'B') == 0xe8
    fov_target = found['GameplayFOV']+16+read('GameplayFOV',12,'<i')
    assert any(r <= fov_target < r+s and flags & 0x20000000 for r,_,s,flags in sections)
    return {'sha256':hashlib.sha256(data).hexdigest(), 'timestamp':hex(stamp),
        'size_of_image':hex(image_size), 'signatures':{n:hex(a) for n,a in found.items()},
        'lod_constant':hex(lod_constant), 'fov_target':hex(fov_target),
        'result':'All 11 signatures unique; original patch bytes and relative targets verified.'}

if __name__ == '__main__':
    p=argparse.ArgumentParser(); p.add_argument('exe',type=Path); args=p.parse_args()
    print(json.dumps(inspect(args.exe),indent=2))
