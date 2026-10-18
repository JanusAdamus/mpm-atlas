"""Declared hardware for every report (PROTOCOL.md §5, latency)."""

from __future__ import annotations

import ctypes
import os
import platform
import sys


def _ram_bytes() -> int | None:
    if sys.platform != "win32":
        try:
            return os.sysconf("SC_PAGE_SIZE") * os.sysconf("SC_PHYS_PAGES")
        except (ValueError, OSError, AttributeError):
            return None

    class MemStatus(ctypes.Structure):
        _fields_ = [("dwLength", ctypes.c_ulong), ("dwMemoryLoad", ctypes.c_ulong),
                    ("ullTotalPhys", ctypes.c_ulonglong), ("ullAvailPhys", ctypes.c_ulonglong),
                    ("ullTotalPageFile", ctypes.c_ulonglong), ("ullAvailPageFile", ctypes.c_ulonglong),
                    ("ullTotalVirtual", ctypes.c_ulonglong), ("ullAvailVirtual", ctypes.c_ulonglong),
                    ("ullAvailExtendedVirtual", ctypes.c_ulonglong)]

    st = MemStatus()
    st.dwLength = ctypes.sizeof(MemStatus)
    return st.ullTotalPhys if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(st)) else None


def hardware() -> dict:
    return {"platform": platform.platform(), "processor": platform.processor(), "logical_cpus": os.cpu_count(),
            "ram_bytes": _ram_bytes(), "python": platform.python_version()}
