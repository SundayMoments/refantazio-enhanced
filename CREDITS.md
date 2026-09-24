# Credits and upstream projects

ReFantazio Enhanced is an independent fork built on substantial existing work.
It would not exist without these projects and their contributors.

- **Idarion** — Luma's Metaphor: ReFantazio integration, including the game hooks,
  rendering integration and motion reconstruction on which this fork builds.
- **Filippo Tarpini / Pumbo and the Luma contributors** —
  [Luma Framework](https://github.com/Filoppi/Luma-Framework), the graphics addon
  framework, HDR work, shader tooling and super-resolution backends.
- **Lyall** — [MetaphorFix](https://codeberg.org/Lyall/MetaphorFix), from which the
  selected engine enhancements were adapted. The earlier GitHub home is
  [Lyall/MetaphorFix](https://github.com/Lyall/MetaphorFix).
- **Patrick Mours and contributors** — [ReShade](https://github.com/crosire/reshade),
  the loader and addon infrastructure. This fork uses a modified local build.
- **SafetyHook contributors** — the inline/mid-hook implementation used by the
  engine enhancements, under the Boost Software License 1.0.
- **Zydis/Zycore contributors** — instruction decoding used by SafetyHook, under
  their included MIT notices.
- **NVIDIA** — the proprietary DLSS/NGX technology used through Luma's backend.
  NVIDIA's runtime is not authored or relicensed by this project.
- **NVIDIA RTX™** technology is used for DLSS. This software contains source
  code provided by NVIDIA Corporation.
- **Microsoft** — the unmodified Visual C++ runtime redistributed with the mod.
- **egui/eframe and the Rust dependency contributors** — the offline desktop
  helper. Exact dependency and font notices are bundled in `Luma/licenses`.

ReFantazio Enhanced adds targeted correctness fixes, integration safeguards,
regression tests, reproducible source patches and selected engine improvements.
It does not claim authorship of the upstream framework, the game's original
DLSS integration, or the DLSS model. Experimental gameplay-quality claims must
be supported by comparative testing.

There is no claimed affiliation with or endorsement by the upstream authors,
NVIDIA, ATLUS or SEGA. Metaphor: ReFantazio is the game this mod targets; its
executable, assets and save data are not included in this repository.

Preserve the notices in [licenses](licenses) and follow [LICENSE.md](LICENSE.md).
Exact upstream revisions and vendored-file hashes are recorded in
[sources.lock.json](sources.lock.json).
