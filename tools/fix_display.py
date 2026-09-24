"""Set Metaphor's next startup to 4K borderless, preserving its config checksum."""
import argparse
from datetime import datetime
import os
from pathlib import Path
import re
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def checksum(body):
    # Game build 18330018: writer at RVA 0x7e886e; reader at 0x7e753d.
    value = 0x7B3562C1
    padded = body + bytes((-len(body)) % 4)
    for (word,) in struct.iter_unpack('<I', padded):
        shift = word & 3
        value = (((value >> shift) | (value << (32 - shift))) & 0xFFFFFFFF) ^ word
    return value


def decode(raw):
    header, separator, body = raw.partition(b'\n')
    magic, size, expected = map(int, header.split(b','))
    if not separator or magic != 40815 or size != len(body) or checksum(body) != expected:
        raise RuntimeError('Configuration integrity check failed; refusing to edit')
    return body


def prepare(raw):
    body = decode(raw)
    values = dict(line.split(b'=', 1) for line in body.splitlines() if b'=' in line)
    modes = values[b'PC_RESOLUTIONS'].split(b',')
    if len(modes) != 6:
        raise RuntimeError('Unexpected per-mode resolution format')
    # Windowed, borderless, fullscreen each have a separate width/height pair.
    modes[2:4] = [b'3840', b'2160']
    display = values[b'PC_DISPLAY_RESOLUTION'].split(b',')
    if len(display) != 3:
        raise RuntimeError('Unexpected display resolution format')
    display[:2] = [b'3840', b'2160']
    changes = {
        b'PC_SCREEN_MODE': b'1',
        b'PC_RESOLUTIONS': b','.join(modes),
        b'PC_DISPLAY_RESOLUTION': b','.join(display),
    }
    for key, value in changes.items():
        body, count = re.subn(rb'^' + re.escape(key) + rb'=[^\n]*$', key + b'=' + value, body, flags=re.M)
        if count != 1:
            raise RuntimeError('Missing or duplicate setting: ' + key.decode())
    result = f'40815,{len(body)},{checksum(body)}\n'.encode() + body
    decode(result)
    return result, changes


def running():
    output = subprocess.check_output(['tasklist', '/FI', 'IMAGENAME eq METAPHOR.exe', '/FO', 'CSV'], text=True)
    return 'metaphor.exe' in output.lower()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--apply', action='store_true')
    parser.add_argument('--wait-for-exit', action='store_true')
    args = parser.parse_args()
    configs = list((Path(os.environ['APPDATA']) / 'SEGA/METAPHOR/Steam').glob('*/pcconfig.dat'))
    if len(configs) != 1:
        raise RuntimeError('Expected exactly one Steam game configuration')
    path = configs[0]
    if args.apply and args.wait_for_exit:
        print('Waiting for Metaphor to close normally before editing its saved configuration...', flush=True)
        while running():
            time.sleep(2)
    if args.apply and running():
        raise RuntimeError('Close Metaphor before applying the file edit')
    raw = path.read_bytes()
    result, changes = prepare(raw)
    out = ROOT / 'artifacts/display-config'
    out.mkdir(parents=True, exist_ok=True)
    (out / 'pcconfig-4k-borderless.dat').write_bytes(result)
    for key, value in changes.items():
        print((key + b'=' + value).decode())
    print('Original and proposed checksums verified.')
    if args.apply:
        if path.read_bytes() != raw or running():
            raise RuntimeError('Configuration or game state changed; retry after closing the game')
        backup = out / ('pcconfig-before-' + datetime.now().strftime('%Y%m%d-%H%M%S-%f') + '.dat')
        backup.write_bytes(raw)
        temporary = path.with_name('pcconfig.dat.codex-tmp')
        with temporary.open('xb') as stream:
            stream.write(result)
        os.replace(temporary, path)
        if path.read_bytes() != result:
            raise RuntimeError('Written configuration does not match')
        decode(path.read_bytes())
        print('Applied and verified. Backup: ' + str(backup))
    else:
        print('Preview only; actual configuration has not been changed.')


if __name__ == '__main__':
    main()
