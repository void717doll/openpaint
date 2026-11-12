# wine reference test

tested on 2026-11-10. wine is installed, but neither the installer nor the fully extracted application reached startup in this restricted session. no paint.net window, menu, editing operation, or saved result was observed.

## observed environment

| check | observed result |
| --- | --- |
| installed packages | `wine`, `wine64`, and `wine32`, version `10.0~repack-6` |
| 64-bit loader | `/usr/lib/wine/wine64 --version` succeeds: `wine-10.0 (Debian 10.0~repack-6)` |
| default launcher | `/usr/bin/wine` selects the installed 32-bit loader; `wine --version` exits with signal 31 (`SIGSYS`) and no output |
| display | `DISPLAY=:0`; `xdpyinfo` cannot open it |
| display authorization | `XAUTHORITY=/tmp/xauth_bJVvpe`; that file is absent |
| virtual display | `Xvfb`, `xvfb-run`, and `Xephyr` are unavailable |
| local socket pair | a normal Python UNIX `socketpair()` succeeds; the wine socket-check message alone does not establish that all local sockets are prohibited |

the installed 64-bit loader was used because it matches the supplied x64 target. no system configuration, sandbox permissions, or existing wine prefix was changed. the `SIGSYS` cause was not traced further.

## target and prefix

the supplied installer still has sha-256 `9c34b285eabcc4658335ad91f9c76ccf8f96d939b9640fac08e4942e394e1e8d`.

the complete matching payload was extracted to ignored `analysis/reference/full-payload/`. it includes the bundled runtime files `hostfxr.dll`, `hostpolicy.dll`, and `coreclr.dll`, which were not all present in the earlier selective extraction. the extracted application identities are:

| file | bytes | sha-256 |
| --- | ---: | --- |
| `x64/paintdotnet.exe` | 107816 | `c4e6d5e7804769235b254cbf973861db539b0377ac4cd5fa1e761274dc875006` |
| `x64/paintdotnet.dll` | 19610920 | `313fad2fb569baf7248a69d1cb981fd713187270abaee64849a723db90013aba` |

all launch attempts set `WINEARCH=win64` and `WINEPREFIX=/home/mia/openpaint/analysis/reference/wine-runtime/prefix`. the user's `~/.wine` was not used.

## launch findings

| attempt | result |
| --- | --- |
| original installer through `wine64` | exit 1 in 0.02 seconds |
| original installer with a private `XDG_RUNTIME_DIR` | exit 1 in 0.02 seconds; wine still selected `/run/user/1000/wine` |
| fully extracted `paintdotnet.exe`, with its own directory as cwd | exit 1 in 0.01 seconds |

all attempts logged the same startup failure:

```text
sock_init: ERROR in sock_check_pollhup()
wineserver: mkdir /run/user/1000/wine: Read-only file system
```

each run had a 40-second limit, process-group termination on timeout, and a prefix-scoped `wineserver -k` cleanup attempt. none reached the timeout. cleanup also reported the read-only runtime-directory error.

the concrete blocker is wine's server initialization within the current filesystem restrictions; the unavailable display is a separate obstacle to GUI observation. these results do not establish whether paint.net 5.1.12, its .NET runtime, or its graphics features work under wine once those environment requirements are met.

## retained records

the shell-generated records are in ignored `analysis/reference/wine-runtime/`:

- `environment.json` and `display-result.json`: actual commands, exit codes, version output, display checks, and socket-pair result.
- `installer.log` and `installer-result.json`: first isolated installer launch.
- `installer-local-runtime.log` and `installer-local-runtime-result.json`: scoped runtime-directory attempt.
- `application.log` and `application-result.json`: direct fully extracted application launch.
- `full-extraction.log` and `full-payload-identities.json`: extraction result and hashes of the application/runtime files.

these are ordinary process logs, not REA runtime Evidence records. the earlier static REA evidence and inferred feature inventory remain in [reference.md](reference.md). no parity-checklist item gains runtime verification from these failed launches.

## reproduction in a working desktop environment

the following retains an isolated prefix and bounds the run. it requires a normal writable wine runtime directory and a working authenticated display; it is not a workaround for this session's restrictions.

```sh
cd /home/mia/openpaint
mkdir -p analysis/reference/wine-runtime/prefix
7z x -y -oanalysis/reference/full-payload paint.net.5.1.12.install.x64.exe
cd analysis/reference/full-payload/x64
WINEPREFIX=/home/mia/openpaint/analysis/reference/wine-runtime/prefix \
  WINEARCH=win64 timeout -k 5s 45s /usr/lib/wine/wine64 ./paintdotnet.exe
WINEPREFIX=/home/mia/openpaint/analysis/reference/wine-runtime/prefix wineserver -k
```

once startup works, the next evidence needed is a controlled launch screenshot and an open/draw/layers/undo/save/reopen sequence, followed by menu/dialog captures and `.pdn` interoperability fixtures. none of those functional checks ran in this session.
