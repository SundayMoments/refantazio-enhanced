# Technical notes — mod core 0.2

An experimental graphics and engine enhancement mod for **Metaphor: ReFantazio**, built on [Luma Framework](https://github.com/Filoppi/Luma-Framework) and [Lyall's MetaphorFix](https://codeberg.org/Lyall/MetaphorFix).

**Built on the work of Idarion, Filippo Tarpini / Pumbo and the Luma contributors, and Lyall.** This independent fork preserves their credits and licenses; it is not an official release or endorsement by those projects. See [credits](../CREDITS.md) and [licensing](../LICENSE.md).

Version **0.2 — experimental** combines temporal AA/DLSS corrections, a 4K borderless DPI fix, selected engine improvements, and a locally compiled ReShade loader with automatic update checks disabled. The Rust helper added in 0.2.1 packages this mod core. See [helper usage](HELPER.md) and [helper build instructions](BUILDING.md).

**Status:** the audited combined build starts successfully at 3840 × 2160 borderless. Startup logs confirm the default engine enhancements are active. 16 temporal GPU/CPU checks, 10 real DLSS runtime checks, 13 engine checks and 5 installer checks pass. A significant improvement over Luma in actual gameplay has **not** yet been established. Motion, combat, scene transitions and performance still need comparison.

## Combined engine controls

Open **Home → Luma → Engine enhancements**. These settings apply after restarting the game.

| Feature | Default | Notes |
| --- | --- | --- |
| Startup logos | Skip | Opening movie remains enabled |
| Menu FPS fix | Enabled | Respects the game's chosen FPS limit with V-Sync off; no forced 60/120 target |
| Shadow resolution | 4096 | Stock 2048 or optional 8192; corrected texel size, stock cascade coverage |
| Draw distance | 20 | Stock 10; larger values increase geometry/foliage range and may reduce performance |
| Gameplay FOV | 1.00 | Original framing; optional 0.75–1.50 multiplier |
| Camera shake | Original | Optional removal |
| Controller prompts | Automatic | Optional fixed controller prompts |

Borderless follows Windows' display refresh rate. The mod never writes the game's FPS setting or changes the monitor's refresh rate. The previous DPI fix remains enabled.

Engine patches are gated to the verified Steam build 18330018 and each requires a unique signature and expected original bytes. Incompatible features report that they are disabled. Do not install a separate MetaphorFix ASI alongside this addon.

The initial combination targets this 16:9 setup. Lyall's ultrawide HUD/movie layout patches and window-procedure replacement are not imported. AO downscaling and outline removal are omitted to preserve quality/art direction; custom resolution scaling would overlap Luma's rendering path. The old analog movement fix was removed upstream after an official game patch.

## Offline behavior and audit

The imported engine source has no network client, updater, telemetry code, process launcher or registry writer. Our ReShade build removes its automatic update request, browser-link launching and screenshot command execution. Luma makes no NGX update calls. See [the audit](../research/offline-audit.md) for findings, source references and verification.

NVIDIA's proprietary DLSS runtime/driver remains outside the source audit: absence of mod requests is not a guarantee about every vendor component or about Steam/the game's own traffic. No firewall or global driver settings are changed. Only the explicitly invoked development setup script downloads source dependencies.

## Changes

- Correct sky reprojection's vertical convention and pixel centers; reject invalid sky history with finite, off-screen motion vectors.
- Correct temporal depth motion-vector scale/sign, account for projection jitter, and reject off-screen or nonfinite history. Guard partial dispatch groups and flat-depth division.
- Initialize camera matrices and invalidate history after missing camera data, large rotation/projection discontinuities, and depth-buffer resize.
- Default the additional texture mip bias to `0`, instead of upstream's `-1`, to favor stability. It is adjustable because this can trade apparent sharpness for reduced texture shimmer.
- Supply DLSS's optional current-color bias mask for particles. The experimental mask rejects history at 50% coverage; it can increase particle noise and can be disabled. FSR retains its continuous reactive mask.
- Enable Luma's process DPI-awareness option for Metaphor so borderless desktop queries use physical pixels on scaled displays.
- Check the loaded Visual C++ runtime against the compiled minimum version, rather than warning about every app-local `msvcp140.dll`. Packaging validates all four signed x64 runtime DLLs against that same minimum.
- Repair release-mode DLSS cleanup, reset history after recreation/failure, guard invalid inputs, restore compute state, and use current scene color when DLSS fails. Correct half-pixel bloom-mask alignment in the final merge.

This retains Luma's game hooks, object-motion reconstruction, rendering changes, HDR support and upstream limitations. It does not replace NVIDIA's DLSS model. The ATL-to-WRL compiler change is solely to build with the installed Visual Studio tools.

## Controls and comparison

Press **Home → Luma**. The running build was observed with **DLSS**, **preset K**, **Texture detail bias 0.00**, and **Reject particle history enabled**. No quality conclusion is inferred from those settings alone.

For native-resolution DLAA, select your intended output resolution in the game's graphics settings and use **Rendering Scale 100%** with DLSS enabled. Lower rendering scales use upscaling. Start with texture detail bias `0`; `-1` reproduces upstream's additional sharpening bias at native or lower rendering resolutions. An enabled global custom mip bias overrides this control.

Compare the same save, resolution, scale, preset and camera path:

1. Slowly pan past railings, rooflines, foliage and distant textured walls. Check both edge flicker and fine detail.
2. Move the character across a contrasting background; check trails on the silhouette, hair and weapon.
3. Watch rain, smoke and spell particles with particle rejection enabled and disabled.
4. Check camera cuts, menus and battle transitions for stale images or flashes.

Static screenshots cannot establish temporal stability. Keep a record of scene, settings and whether the defect occurs while stationary, moving the camera, or moving an object.

## Developer comparison installer (legacy)

Build and package the project using the instructions below, then close the game. The installer currently targets the standard Steam location, `C:\Program Files (x86)\Steam\steamapps\common\METAPHOR`; other library locations are not automatically discovered. Writing under Program Files may require elevation:

```powershell
python tools\install.py fork      # Install the locally built package
python tools\install.py restore   # Remove this installation / restore original files
```

The installer records original and installed hashes in `artifacts/install-state.json` and saves any replaced originals under `artifacts/original-files`. It checks for unexpected edits, stages replacements atomically, and retains recovery hashes for interrupted installs. Runtime-generated shader cache, logs and settings remain after restoration. The restore operation ends this installation session; the script refuses to reuse a completed recovery manifest for a new installation. Keep the manifest and backups together. Developer-only `stable` and `baseline` comparison modes require previously prepared local artifacts, which are not shipped here; those older loaders contain upstream's updater.

## Saved display configuration

`python tools\fix_display.py --apply` sets the next startup to borderless at 3840 × 2160 while preserving unrelated settings and the selected refresh rate. Close the game first. The script validates and regenerates the game's custom checksum and saves a timestamped backup in `artifacts/display-config`.

The checksum starts at `0x7B3562C1`; for each zero-padded little-endian 32-bit word, it rotates the accumulator right by the word's low two bits, then XORs the word. This matches the configuration reader and writer in the installed game build. A plain text edit without recalculating it can cause the game to reject the configuration.

On the tested configuration, changing the saved resolution alone still produced a DPI-virtualized 1920 × 1080 borderless buffer. The addon enables Luma's existing `force_ignore_dpi` option so desktop queries use physical pixels. The legacy `fix_dpi.py` compatibility-registry experiment is not needed for this build.

## Build and test

Tested build dependencies: Python 3.13, Git, Visual Studio 2022 C++ Build Tools (v143) and Windows SDK 10.0.26100.0. A GPU supporting DLSS is required for the NGX runtime tests. The modified Luma checkout lives under `upstream`. GLAD generation uses pinned Jinja2/MarkupSafe under `artifacts/build-libs`. The downloaded base release supplies unchanged shaders/runtime files; its loader and addon are replaced by our builds.

Clone this repository, enter its directory, then provision pinned dependencies once:

```powershell
git clone https://github.com/SundayMoments/refantazio-enhanced.git
cd refantazio-enhanced
python tools\bootstrap.py
```

Bootstrap explicitly downloads and verifies the sources/dependencies in `sources.lock.json` and applies both source patches. It refuses an existing `upstream` checkout. These are development downloads; normal compilation and the installed mod do not invoke this script. Then build:

```powershell
python tools\build.py
python tools\build.py --reshade
python tools\build.py --tests
python tools\build.py --dlss-tests
& '.\artifacts\tests\temporal_gpu.exe' 'upstream\Shaders\Metaphor ReFantazio'
& '.\artifacts\tests\engine.exe'
Push-Location artifacts\tests
.\dlss_runtime.exe
Pop-Location
python tests\test_install.py
python tests\inspect_game.py 'C:\Program Files (x86)\Steam\steamapps\common\METAPHOR\METAPHOR.exe'
python tools\package.py
python tests\check_patches.py
```

The package is `artifacts/ReFantazio-Enhanced-0.2.zip`. Its per-file hashes are in `artifacts/fork-manifest.json`. Build logs are in `research`. Luma changes are in `patches/temporal-quality.patch`; ReShade changes are in `patches/reshade-offline.patch`; engine code is under `src` and the exact vendored hook sources under `vendor`. Packaging verifies both source patches against the modified checkouts.

Local packages and debug symbols can contain build-machine paths. They are ignored by Git and were not included in this source publication. Review and sanitize artifacts separately before distributing binaries. See [the publication review](../research/publication-review.md).

## Provenance

- Base revision: [`3d48c38c17a59136be068e1f33bd25a92be69089`](https://github.com/Filoppi/Luma-Framework/commit/3d48c38c17a59136be068e1f33bd25a92be69089).
- Base release: [latest-704, Metaphor package](https://github.com/Filoppi/Luma-Framework/releases/tag/latest-704).
- Archive: `downloads/Luma-Metaphor_ReFantazio-latest-704.zip`.
- Verified archive SHA-256: `b03268582488c1edf6e539b1a3e25d0d6c92b251be2bcd05bbd2c47725070373`.
- Included NVIDIA DLSS runtime file version: `310.9.1.0`.
- MetaphorFix revision: `a74b056d76065f069953c27eb474d29bcdf017b0` (MIT, Lyall).
- ReShade source revision: `eeb2c76aea8e00200d88b479c9036c5ea4d06d5e`; local runtime version `6.8.0.2 UNOFFICIAL OFFLINE`.
- Vendored hook files and all submodule revisions: `sources.lock.json`.

Metaphor integration by **Idarion**; Luma Framework by **Filippo Tarpini / Pumbo** and contributors; selected engine fixes by **Lyall**; ReShade by **Patrick Mours** and contributors. Preserve all upstream credits. Luma uses a custom MIT license requiring author/project attribution and author permission for commercial use; see `licenses` and the package's `Luma/licenses`. SafetyHook uses Boost 1.0 and Zydis/Zycore use MIT. This is an independent experimental fork, not an official upstream release.

See [validation notes](../research/validation.md) and the [final DLSS audit](../research/dlss-audit.md) for the evidence and remaining limitations. The audit checks passed: 16 shader/CPU, 10 real NGX runtime, 13 engine and 5 installer tests. The NGX harness needs ordinary driver access; running it in a restricted sandbox can stall NVIDIA's telemetry cleanup.
