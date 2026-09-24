# Validation record — 2026-09-23

## Environment

- NVIDIA GeForce RTX 5090, driver 617.14.
- D3D11 hardware execution; the optional D3D11 debug layer was unavailable.
- MSVC v143 / 14.44.35207, Windows SDK 10.0.26100.0.
- Steam Metaphor build 18330018, x64 D3D11.
- Luma source/release pinned as recorded in the README.

## Build and synthetic checks

The Metaphor addon built successfully in `Publishing-Release` for x64. The harness compiles and executes the actual modified compute shaders, reads GPU results back, and checks mathematical expectations. Inputs are 13 × 9 pixels to exercise partially filled thread groups. These small synthetic scenes do not exercise the game's real object motion or DLSS's image reconstruction.

The current fork passes **13/13** checks: ten GPU shader checks and three CPU policy checks. See `tests-fork.log` and `tests/temporal_gpu.cpp`.

The original release shaders failed these four reprojection expectations:

| Check | Expected | Original result | Fork result |
| --- | ---: | ---: | ---: |
| Rotating sky, selected horizontal vector | -1.028266 | 1.420918 | Pass |
| Invalid sky projection, finite off-screen vector | 26 | -infinity | Pass |
| Reject off-screen depth history | 0.5 | 0.457500 | Pass |
| Align previous depth with jitter | 0.5 | 0.451250 | Pass |

The depth-jitter check demonstrates that the old shader ignores the new jitter correction input. It does not independently validate the entire game's camera extraction path. The depth motion-vector sign/scale change was also checked against the upstream motion shader's NDC-to-UV convention in source.

The original mask shader additionally failed the fork's new binary-mask expectation. That is a **deliberate policy difference**, not an established upstream defect. Its quality tradeoff needs real particle scenes. The original flat-depth test produced finite output on this GPU, so the denominator guard is preventative; an observed stock-game NaN defect is not claimed.

The earlier baseline log has 12 checks (7 pass, 5 fail); the final fork log includes the subsequently added projection-handedness policy check. CPU policy checks always compile the fork's helper, including during a run against original shaders. They are not a comparison of upstream CPU behavior.

## Runtime smoke check

The installer verified all 145 payload files. The game reached Grand Trad / Sunshade Row with the addon loaded. The overlay showed:

- DLSS enabled, preset K.
- `ReFantazio temporal quality - experimental`.
- Texture detail bias 0.00 and particle history rejection enabled.
- History reset counter at 2 in the observed scene.

The process was responsive. ReShade logged addon registration, D3D11 setup, and shader compilation; the checked log contained upstream shader warnings but no error entries. This is a load/render smoke check, not instrumentation proving every DLSS evaluation succeeds, a stability soak, or a performance benchmark. The captured overlay is not a motion-quality comparison.

## Remaining validation

- User assessment of shimmer, ghosting and blur against the original release is pending. No percentage improvement or superiority claim is justified yet.
- Compare at equal output and render resolutions, DLSS preset, frame rate, scene and camera path. Check retained detail as well as stability; texture bias can hide shimmer by reducing high-frequency detail.
- Test animated meshes, foliage, transparent layers, rain and combat effects. Existing Luma object-motion limitations may remain.
- Particle rejection may trade trails for noise. The 50% threshold is provisional; faint particles retain history.
- The cut heuristic detects rotation above 60 degrees per frame and projection-scale changes above 20%, plus missing/invalid camera data. It can miss pure translation cuts and smaller cinematic cuts. It can also reset a very fast continuous rotation.
- Exercise resolution changes, alt-tab, loading, menus, battle transitions and extended play. Resize invalidation was reviewed in source but not covered by the shader harness or a dedicated in-game resize test.
- FSR mask behavior passed a synthetic check; no full FSR gameplay validation was performed. No other game's addon was built or tested after the shared optional DLSS field/compiler portability changes.

The user requested time to analyze the running game. Further UI input and build switching were stopped to avoid interfering with that assessment.

## Borderless startup fix

The user subsequently reported that borderless mode and the Steam overlay were stuck at 1920 × 1080. Editing `pcconfig.dat` with a valid recomputed checksum made startup request 3840 × 2160, but the game then resized back to 1920 × 1080. A read-only helper measured 1920 × 1080 from DPI-virtualized desktop queries and 3840 × 2160 with DPI awareness enabled. The running game reported `PROCESS_DPI_UNAWARE` (0).

Enabling `force_ignore_dpi` specifically in the Metaphor addon uses Luma's existing `SetProcessDpiAwarenessContext(PER_MONITOR_AWARE_V2)` startup path. After rebuilding, reinstalling and relaunching at 16:46 on September 23:

- The running game reported per-monitor DPI awareness (2).
- The startup swap chain was 3840 × 2160 with `Windowed=TRUE`.
- After the transient startup resize, the final resize was **3840 × 2160**, with exclusive fullscreen disabled. The prior build's corresponding final resize was **1920 × 1080**.
- The saved screen mode remains borderless (`PC_SCREEN_MODE=1`).

This verifies the rendering-buffer resolution after startup. It does not independently verify Steam overlay visual sharpness. The ineffective compatibility-registry trial was restored to its original absent value. See `display-startup-after.log`, `tools/fix_display.py`, and the display-configuration backups under `artifacts/display-config`.

## Combined mod and final DLSS audit

The Super Mod 0.2 adds selected Lyall engine improvements and a locally compiled
offline ReShade loader. The [engine/offline audit](offline-audit.md) records the
11 verified signatures and fixes to pattern matching, rollback and installation.
The [DLSS audit](dlss-audit.md) records additional lifecycle, failure-path, state
handoff and output-alignment corrections. Tests passed: 16 temporal shader/CPU,
10 real NGX runtime, 13 engine and 5 installer checks. Both distributable source
patches were applied to pristine pinned files and reproduced the working source
(12 Luma files and 4 ReShade files).

The final package installed and hash-verified 152 files. The September 23, 17:44
startup loaded the audited addon with ReShade 6.8.0.2; default engine fixes were
reported active. The process remained responsive, reported per-monitor DPI
awareness, and ended startup with a 3840×2160 buffer and exclusive fullscreen
disabled. This is another startup smoke check, not a new gameplay quality or
performance comparison. The game FPS setting and Windows refresh rate were not
changed. The game was left running for user evaluation.
