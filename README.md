# ReFantazio Enhanced

An experimental **Metaphor: ReFantazio** mod combining [Luma](https://github.com/Filoppi/Luma-Framework) and selected [Lyall / MetaphorFix](https://codeberg.org/Lyall/MetaphorFix) improvements.

- DLSS / DLAA with temporal rendering and history-handling fixes.
- Correct 4K borderless rendering on scaled Windows displays.
- Higher-resolution shadows, draw distance, FOV, startup skips, and menu FPS fixes.
- A Rust helper for installation, removal, and settings — entirely local, with no updater, downloads, or telemetry.

## Install

1. Download the helper from [Releases](https://github.com/SundayMoments/refantazio-enhanced/releases).
2. Close the game, select its folder, and click **Install bundled version**.
3. Use **Settings** to customize the mod, including ReShade's text size. Press **Home** in-game for more controls.

Windows x64; Steam build **18330018**. DLSS requires an NVIDIA RTX GPU. For native-resolution DLAA, use DLSS with the game's Rendering Scale at **100%**. Your chosen FPS limit and resolution remain yours.

The helper backs up replaced files and can restore them. Keep its `.refantazio-enhanced` folder. Remove a separate MetaphorFix installation first. [Helper details](docs/HELPER.md).

**Experimental:** comparative gameplay improvement over upstream Luma is still being evaluated. NVIDIA's proprietary runtime/driver is outside our source audit.

## Credits

**Idarion**, **Filippo Tarpini / Pumbo and Luma contributors**, **Lyall**, and **ReShade contributors** made this possible. This is an independent fork.

[Full credits](CREDITS.md) · [Licenses](LICENSE.md) · [Build instructions](docs/BUILDING.md) · [Technical notes](docs/TECHNICAL.md) · [DLSS audit](research/dlss-audit.md)
