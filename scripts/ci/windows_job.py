"""Bound an entire Windows child tree before its first instruction executes."""
import ctypes as C
from ctypes import wintypes as W
import json
import os
from pathlib import Path
import subprocess


class IO_COUNTERS(C.Structure):
    _fields_ = [(name, C.c_ulonglong) for name in (
        'ReadOperationCount', 'WriteOperationCount', 'OtherOperationCount',
        'ReadTransferCount', 'WriteTransferCount', 'OtherTransferCount')]


class BASIC_LIMITS(C.Structure):
    _fields_ = [('PerProcessUserTimeLimit', C.c_longlong), ('PerJobUserTimeLimit', C.c_longlong),
                ('LimitFlags', W.DWORD), ('MinimumWorkingSetSize', C.c_size_t),
                ('MaximumWorkingSetSize', C.c_size_t), ('ActiveProcessLimit', W.DWORD),
                ('Affinity', C.c_size_t), ('PriorityClass', W.DWORD), ('SchedulingClass', W.DWORD)]


class EXTENDED_LIMITS(C.Structure):
    _fields_ = [('BasicLimitInformation', BASIC_LIMITS), ('IoInfo', IO_COUNTERS),
                ('ProcessMemoryLimit', C.c_size_t), ('JobMemoryLimit', C.c_size_t),
                ('PeakProcessMemoryUsed', C.c_size_t), ('PeakJobMemoryUsed', C.c_size_t)]


class CPU_RATE(C.Structure):
    _fields_ = [('ControlFlags', W.DWORD), ('CpuRate', W.DWORD)]


class STARTUPINFO(C.Structure):
    _fields_ = [('cb', W.DWORD), ('lpReserved', W.LPWSTR), ('lpDesktop', W.LPWSTR),
                ('lpTitle', W.LPWSTR), ('dwX', W.DWORD), ('dwY', W.DWORD),
                ('dwXSize', W.DWORD), ('dwYSize', W.DWORD), ('dwXCountChars', W.DWORD),
                ('dwYCountChars', W.DWORD), ('dwFillAttribute', W.DWORD),
                ('dwFlags', W.DWORD), ('wShowWindow', W.WORD), ('cbReserved2', W.WORD),
                ('lpReserved2', C.POINTER(C.c_byte)), ('hStdInput', W.HANDLE),
                ('hStdOutput', W.HANDLE), ('hStdError', W.HANDLE)]


class PROCESS_INFORMATION(C.Structure):
    _fields_ = [('hProcess', W.HANDLE), ('hThread', W.HANDLE),
                ('dwProcessId', W.DWORD), ('dwThreadId', W.DWORD)]


def kernel():
    k = C.WinDLL('kernel32', use_last_error=True)
    signatures = {
        'CreateJobObjectW': ([C.c_void_p, W.LPCWSTR], W.HANDLE),
        'SetInformationJobObject': ([W.HANDLE, C.c_int, C.c_void_p, W.DWORD], W.BOOL),
        'QueryInformationJobObject': ([W.HANDLE, C.c_int, C.c_void_p, W.DWORD, C.c_void_p], W.BOOL),
        'AssignProcessToJobObject': ([W.HANDLE, W.HANDLE], W.BOOL),
        'CreateProcessW': ([W.LPCWSTR, W.LPWSTR, C.c_void_p, C.c_void_p, W.BOOL,
                            W.DWORD, C.c_void_p, W.LPCWSTR,
                            C.POINTER(STARTUPINFO), C.POINTER(PROCESS_INFORMATION)], W.BOOL),
        'ResumeThread': ([W.HANDLE], W.DWORD),
        'WaitForSingleObject': ([W.HANDLE, W.DWORD], W.DWORD),
        'GetExitCodeProcess': ([W.HANDLE, C.POINTER(W.DWORD)], W.BOOL),
        'TerminateProcess': ([W.HANDLE, W.UINT], W.BOOL),
        'TerminateJobObject': ([W.HANDLE, W.UINT], W.BOOL),
        'CloseHandle': ([W.HANDLE], W.BOOL),
    }
    for name, (args, result) in signatures.items():
        fn = getattr(k, name)
        fn.argtypes, fn.restype = args, result
    return k


def checked(value):
    if not value:
        raise C.WinError(C.get_last_error())
    return value


def memory_metrics(k, job):
    """Read aggregate committed-memory peaks before closing this owned job."""
    usage = EXTENDED_LIMITS()
    if not k.QueryInformationJobObject(job, 9, C.byref(usage), C.sizeof(usage), None):
        return dict(schema=1, status='unavailable', windows_error=C.get_last_error())
    return dict(schema=1, status='observed', measurement='Windows job committed memory; not RSS',
                job_memory_limit_bytes=usage.JobMemoryLimit,
                peak_job_memory_bytes=usage.PeakJobMemoryUsed,
                peak_process_memory_bytes=usage.PeakProcessMemoryUsed,
                limit_flags=usage.BasicLimitInformation.LimitFlags)


def run(command, cwd, env, log: Path, timeout=1800):
    """One logical CPU equivalent, 2 GiB aggregate, below normal priority; fail closed."""
    if os.name != 'nt':
        raise RuntimeError('Windows Job Object limits require native Windows')
    import msvcrt
    k = kernel()
    job = checked(k.CreateJobObjectW(None, None))
    process = PROCESS_INFORMATION()
    try:
        limits = EXTENDED_LIMITS()
        limits.BasicLimitInformation.LimitFlags = 0x2000 | 0x200  # kill-on-close, job memory
        limits.JobMemoryLimit = 2 * 1024 ** 3
        checked(k.SetInformationJobObject(job, 9, C.byref(limits), C.sizeof(limits)))
        rate = CPU_RATE(1 | 4, max(1, 10000 // (os.cpu_count() or 1)))
        checked(k.SetInformationJobObject(job, 15, C.byref(rate), C.sizeof(rate)))
        log.parent.mkdir(parents=True, exist_ok=True)
        with log.open('wb') as output, open(os.devnull, 'rb') as stdin:
            handles = [msvcrt.get_osfhandle(file.fileno()) for file in (stdin, output)]
            for handle in handles:
                os.set_handle_inheritable(handle, True)
            startup = STARTUPINFO()
            startup.cb = C.sizeof(startup)
            startup.dwFlags = 0x100  # STARTF_USESTDHANDLES
            startup.hStdInput, startup.hStdOutput = handles
            startup.hStdError = handles[1]
            block = C.create_unicode_buffer('\0'.join(f'{key}={value}' for key, value in
                                                       sorted(env.items(), key=lambda pair: pair[0].upper())) + '\0\0')
            line = C.create_unicode_buffer(subprocess.list2cmdline([str(arg) for arg in command]))
            try:
                checked(k.CreateProcessW(None, line, None, None, True,
                                        0x4 | 0x400 | 0x4000,  # suspended, Unicode env, below normal
                                        block, str(cwd), C.byref(startup), C.byref(process)))
            finally:
                for handle in handles:
                    os.set_handle_inheritable(handle, False)
            checked(k.AssignProcessToJobObject(job, process.hProcess))
            if k.ResumeThread(process.hThread) == 0xffffffff:
                raise C.WinError(C.get_last_error())
            result = k.WaitForSingleObject(process.hProcess, int(timeout * 1000))
            if result == 0x102:
                checked(k.TerminateJobObject(job, 124))
                k.WaitForSingleObject(process.hProcess, 10000)
                return 124
            if result != 0:
                raise C.WinError(C.get_last_error())
            code = W.DWORD()
            checked(k.GetExitCodeProcess(process.hProcess, C.byref(code)))
            return code.value
    finally:
        # Also covers failed assignment before ResumeThread, and leftover descendants.
        if process.hProcess:
            k.TerminateProcess(process.hProcess, 125)
        try:
            metrics = memory_metrics(k, job)
            log.with_suffix(log.suffix + '.memory.json').write_text(
                json.dumps(metrics, indent=2) + '\n', encoding='utf-8')
        except OSError:
            # Telemetry failure must not hide the original child result/error.
            print('Unable to retain Windows job memory metrics', flush=True)
        finally:
            k.CloseHandle(job)
            for handle in (process.hThread, process.hProcess):
                if handle:
                    k.CloseHandle(handle)
