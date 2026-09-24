# Offline and correctness audit — 2026-09-23

## Scope and conclusion

Reviewed the selected source imported from Lyall/MetaphorFix at
`a74b056d76065f069953c27eb474d29bcdf017b0`, its bundled SafetyHook/Zydis code,
our engine integration, Luma's Windows runtime and NGX call sites, and the
pinned ReShade runtime. No outbound requests, updater, telemetry client,
process launcher, or registry writer was found in the imported engine code.
This is a source review, not a formal proof about every third-party binary.

Only selected engine fixes are compiled into the Luma addon. No downloaded
MetaphorFix ASI, proxy loader, installer executable, or separate update service
is installed. Source downloads and Git operations are development-time actions;
the installed mod contains no Git/Python scripts and never runs them.

## Changes for offline operation

- ReShade's original `runtime_update_check.cpp` automatically requested GitHub's
  tags API. Replaced the entire implementation with a no-op and removed its
  WinInet link dependency. The packaged dxgi.dll is built from this source.
- Disabled ReShade's browser-opening callback and arbitrary screenshot
  post-save command execution. Local log/config/shader Explorer actions remain.
- No `NVSDK_NGX_UpdateFeature` calls exist in Luma's runtime sources. We retain
  the bundled local DLSS runtime and do not opt into the SDK's update API.
  The bundled NGX interface has no public disable-all-network flag; no invented
  flag or global NVIDIA registry setting was added.
- Built binaries' normal and delayed imports were inspected: the addon and
  offline dxgi.dll have no WinInet, WinHTTP, URLMon or Winsock imports. The
  bundled nvngx_dlss.dll also has no such direct imports. Import inspection
  alone cannot exclude dynamic loading or driver activity.
- ReShade contains socket-hook forwarding code used by its restricted addon
  variant; this build uses `RESHADE_ADDON=2`. It does not create a connection or
  add mod traffic. The game and Steam retain their own networking behavior.

NVIDIA's closed-source runtime, NGX driver components, and Windows services are
outside this source audit. This mod does not request online features, but it
cannot guarantee that proprietary driver internals or the entire game process
never communicate. No system-wide network/security settings were changed.

The real NGX regression harness confirmed that NVIDIA's driver-side NGX loader
loads `NvTelemetryAPI64.dll`. A sandboxed shutdown waited in that component;
the same test completed normally outside the sandbox. This is concrete evidence
of a proprietary telemetry component, not evidence of a transmitted request.
It is not part of Lyall's imported code and was not added by this integration.

SDK reference: [NVIDIA DLSS API](https://github.com/NVIDIA/DLSS/blob/main/include/nvsdk_ngx.h).
Source origins: [Luma](https://github.com/Filoppi/Luma-Framework),
[ReShade](https://github.com/crosire/reshade),
[MetaphorFix](https://codeberg.org/Lyall/MetaphorFix).

## Correctness findings addressed

1. **Ambiguous FOV signature:** upstream matched two callers of the same
   function. Narrowed to one verified caller and validated its call opcode and
   relative target before installing the hook.
2. **Incorrect shadow signature on this build:** the upstream pattern matched
   six unrelated regions and missed the real setter. Replaced it with the
   verified reciprocal-load/property-setter sequence at RVA `0x109fdc4`, checked
   its original reciprocal `1/2048`, and hook after the load. Both dimension
   immediates are separately checked before changing them. Preserved cascade
   coverage instead of copying upstream's explicitly uncertain distance change.
3. **Unchecked title-state writes:** the hook now verifies the pointer is both
   accessible and writable before updating it.
4. **Unsupported builds and partial features:** executable identity, unique
   signatures, bounds and expected bytes gate changes. A failed feature rolls
   back its own changes. Other mods' subsequent byte edits are not overwritten
   during rollback. Separate MetaphorFix installations are refused.
5. **Installer interruption:** stage, hash-check and atomically replace each
   file. Persist both old and new hashes until completion so interrupted
   installations can be resumed or restored. Preserve unexpected user edits.
6. **Build reproducibility:** fixed shallow-checkout version metadata and GLAD
   generation to use the pinned XML specifications without fetching schemas
   or installing packages during compilation.
7. **DLSS lifecycle and failure paths:** restored cleanup calls removed by
   release-mode assertions, guarded initialization/evaluation, added current-frame
   fallback and history resets, fixed compute bindings and bloom-mask alignment.
   See [the final DLSS audit](dlss-audit.md) for the individual findings and tests.

## Verification and limits

- Both x64 release binaries compiled successfully.
- 11/11 game signatures unique on executable SHA-256
  `548ddc955c176867f062c44f94c03dd9ac33caefb278a5ac388e792aedf09504`.
- 13 engine tests passed, including actual SafetyHook installation/removal on
  executable test code, rollback, duplicate signatures, inaccessible memory,
  setting sanitization and rejection of an unsupported executable.
- 16 GPU/CPU temporal and output-fallback tests passed on the RTX 5090.
- 10 real NGX runtime tests passed, including native DLAA, 2× upscaling, injected
  failures, history recovery and successful release-mode cleanup.
- 5 installer tests passed, including injected partial-copy failure.

The menu fix changes only the engine's separate menu limit. It does not change
`PC_FRAMERATE_MAX`, set a fixed FPS target, or select a display refresh rate.
Borderless mode follows Windows' display refresh rate (120 Hz on this machine).
No claim of locked 120 FPS performance or superior gameplay image quality is
made from these tests. FOV/controller/shake options default to original behavior
and still require scene testing when enabled.
