"""Read-only MSVC runtime checks used while building an offline bundle."""
import ctypes
from ctypes import wintypes
from pathlib import Path
import re
import struct

CRT_FILES = ('msvcp140.dll', 'msvcp140_atomic_wait.dll', 'vcruntime140.dll', 'vcruntime140_1.dll')


def required_version(addon):
    matches = set(re.findall(rb'RE_MSVC_RUNTIME_MIN=(\d+)\.(\d+)\.(\d+)\.(\d+)', addon))
    if len(matches) != 1:
        raise RuntimeError('Rebuild the addon: a unique MSVC runtime requirement is missing')
    return tuple(map(int, matches.pop()))


def compatible(version, minimum):
    return version[0] == minimum[0] and version >= minimum


def file_version(path):
    api = ctypes.WinDLL('version', use_last_error=True)
    api.GetFileVersionInfoSizeW.argtypes = [wintypes.LPCWSTR, ctypes.POINTER(wintypes.DWORD)]
    api.GetFileVersionInfoSizeW.restype = wintypes.DWORD
    api.GetFileVersionInfoW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD, ctypes.c_void_p]
    api.GetFileVersionInfoW.restype = wintypes.BOOL
    api.VerQueryValueW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR, ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(wintypes.UINT)]
    api.VerQueryValueW.restype = wintypes.BOOL
    size = api.GetFileVersionInfoSizeW(str(path), None)
    if not size:
        raise RuntimeError('Runtime has no readable version: ' + path.name)
    buffer = ctypes.create_string_buffer(size)
    value, length = ctypes.c_void_p(), wintypes.UINT()
    if not api.GetFileVersionInfoW(str(path), 0, size, buffer) or not api.VerQueryValueW(buffer, '\\', ctypes.byref(value), ctypes.byref(length)):
        raise RuntimeError('Cannot read runtime version: ' + path.name)
    if length.value < 52 or not value.value:
        raise RuntimeError('Invalid runtime version resource: ' + path.name)
    signature, _, major_minor, build_revision = struct.unpack('<4I', ctypes.string_at(value, 16))
    if signature != 0xFEEF04BD:
        raise RuntimeError('Invalid runtime version signature: ' + path.name)
    return major_minor >> 16, major_minor & 0xffff, build_revision >> 16, build_revision & 0xffff


def validate_directory(directory, minimum):
    versions = {}
    for name in CRT_FILES:
        path = directory / name
        data = path.read_bytes()
        if len(data) < 64 or data[:2] != b'MZ':
            raise RuntimeError('Invalid runtime DLL: ' + name)
        pe = struct.unpack_from('<I', data, 0x3c)[0]
        if pe + 6 > len(data) or data[pe:pe+4] != b'PE\0\0' or struct.unpack_from('<H', data, pe+4)[0] != 0x8664:
            raise RuntimeError('Runtime must be Windows x64: ' + name)
        version = file_version(path)
        if not compatible(version, minimum):
            raise RuntimeError(f'{name} {version} is older than required {minimum}')
        versions[name] = version
    return versions


def system_directory():
    api = ctypes.WinDLL('kernel32', use_last_error=True)
    api.GetSystemDirectoryW.argtypes = [wintypes.LPWSTR, wintypes.UINT]
    api.GetSystemDirectoryW.restype = wintypes.UINT
    buffer = ctypes.create_unicode_buffer(32768)
    size = api.GetSystemDirectoryW(buffer, len(buffer))
    if not 0 < size < len(buffer):
        raise RuntimeError('Cannot locate the installed Windows runtime')
    return Path(buffer.value)
