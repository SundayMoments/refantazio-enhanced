# DLSS correctness audit — 2026-09-23

Reviewed the Metaphor integration, shared D3D11 NGX backend, motion/depth/mask
shaders, feature lifecycle, and final output merge. Changes are included in
the version 0.2 source patch and release build, now named ReFantazio Enhanced.

## Defects corrected in this pass

1. **Release-build cleanup was compiled out.** Feature release, parameter
   destruction and NGX shutdown were expressions inside `assert`. Publishing
   builds define `NDEBUG`, which removes those calls. They now execute
   independently of assertions. Device/context references remain alive through
   feature cleanup, and a device reference survives until NGX shutdown returns.
2. **Failed feature initialization could reach evaluation.** The game ignored
   `UpdateSettings`' return value, and the backend relied on assertions for its
   feature/input preconditions. Both layers now reject invalid state. Missing
   inputs, invalid dimensions, non-finite jitter/exposure and failed motion-data
   preparation cannot reach DLSS evaluation.
3. **Failed evaluation could display stale or uninitialized output.** The merge
   previously always read the DLSS output texture. It now uses a dedicated
   shader variant to scale the current scene color when reconstruction fails.
   It preserves the scene's bloom alpha policy. This fallback is spatial and
   may show aliasing during failure; it does not claim temporal reconstruction.
4. **History was not explicitly reset after feature recreation.** The backend
   now resets the first successful frame after creation, recreation, failed
   evaluation or rejected draw input. Subsequent successful frames keep history.
5. **Compute resource/state handoffs were incomplete.** Motion/mask UAV bindings
   are cleared before NGX reads those textures. NGX's output bindings are cleared
   before the merge reads its result, and Luma constants are rebound after NGX
   feature creation/evaluation. Merge bindings are cleared before copying out.
6. **Bloom alpha was sampled half a pixel off-center.** The output pass now uses
   pixel-center UVs, preserving the source mask at native resolution. A shader
   test with alternating alpha values detects the former half-pixel filtering.
   Running the original shader returned `0.5` where the source alpha was `1.0`;
   the corrected shader returned `1.0`. No visible gameplay bloom artifact or
   causal link to ghosting was demonstrated by that test.
7. **Resolution-change detection required both dimensions to differ.** The
   upscaling flag now detects a change in either dimension.

## Input contract review

- Geometry motion is converted from the game's NDC displacement into
  previous-minus-current render-pixel displacement, with Y-down texture UVs.
  NGX motion scale is `(1, 1)` and the low-resolution-motion flag is set.
- Current jitter is also used in the previous camera projection when generating
  geometry/sky motion, excluding inter-frame jitter from those vectors. The
  backend passes pixel jitter separately and leaves `MVJittered` disabled.
- Sky reprojection uses pixel centers, the D3D Y convention, and finite
  off-screen vectors for invalid previous projections.
- The integration uses normal device depth with `DepthInverted` disabled. Scene
  color is linear floating point with the HDR flag. The existing explicit 1×1
  exposure texture contains 1; the SDK helper treats zero optional pre-exposure
  as 1. These choices were checked against the bundled NGX helper implementation.
- The particle hint uses a single-channel floating-point resource and goes to
  `pInBiasCurrentColorMask`, separate from FSR reactivity. Binary coverage at 50%
  is an experimental quality policy, not a universally correct threshold.
- Fallback output and DLSS output both restore the original scene's alpha, which
  Metaphor uses for bloom exclusion.

## Verification

The addon builds as x64 Publishing-Release. On the RTX 5090:

- **16 shader/CPU checks passed**, executing the actual shipping HLSL. These
  include reprojection direction, invalid history, depth jitter, mask policy,
  pixel-aligned merge, stale-output rejection and fallback scaling from 7×5 to
  13×9. Odd dimensions exercise partial compute thread groups.
- **10 real NGX checks passed** with bundled DLSS runtime `310.9.1.0`.
  The harness compiles the actual backend with `NDEBUG`, uses 128×128 DLAA and
  128×128 → 256×256 DLSS, checks GPU readback for finite constant linear RGB,
  and observes the submitted mask, exposure, jitter, motion scale and reset flag.
  API wrappers inject creation/evaluation failures; normal evaluations execute
  in the real NVIDIA runtime. Cleanup calls and their success codes are verified.
- The D3D11 debug layer is not installed on this machine. GPU checks used the
  hardware driver without it; absence of debug-layer warnings is not claimed.

An initial sandboxed run waited inside `NvTelemetryAPI64!UninitializeTelemetry`
while NGX shut down. A read-only debugger stack identified the driver component.
Running the same test normally completed in seconds and passed all checks.
This observation confirms that NGX loads NVIDIA telemetry code; it does not
establish whether any network request occurred. No packet capture was performed.
See [offline audit](offline-audit.md) for the proprietary-driver boundary.

## Limits

These checks establish specific API and shader correctness properties, not
superiority to Luma in moving gameplay. The inherited object/skinning motion
capture still needs scene validation. The conservative camera-cut heuristic can
miss pure translation cuts or small cinematic cuts. Particle rejection may trade
ghosting for noise. Extended play, alt-tab, battle/loading transitions, and a
controlled visual comparison at equal settings remain gameplay validation.

No network updater, online service, or download path was added to the installed
mod. Public source hosting is separate from the mod's runtime behavior.
