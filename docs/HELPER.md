# Offline helper

Close Metaphor, run **ReFantazio Enhanced Helper**, and choose the folder containing
`METAPHOR.exe`. Steam libraries are detected automatically; Browse handles custom locations.
Read the bundled runtime terms under **Credits & licenses**, then select **Install mod**.
If an older Luma/ReShade installation is present, enable **Back up and replace existing mod files**.
Remove a separate MetaphorFix installation first because its hooks overlap.

The bundle supports Windows x64 and Steam build **18330018**. DLSS requires an NVIDIA RTX GPU.
The helper validates the game and bundled files before replacing anything. The required
Visual C++ runtime DLLs are included beside the mod; there is no runtime download or system installer.
If Windows denies write access to the game folder, use **Relaunch as administrator** and select it again.

**Settings** edits the DLSS preset, image quality, engine options, and ReShade overlay text scale.
Reload reads existing settings; Recommended settings only changes the form until you press Save.
Changes apply on the next game launch. Other ReShade keys and sections are preserved.
The helper does not edit saves, the game's resolution/FPS settings, Windows refresh rate, or driver settings.
For DLAA, choose DLSS and set the game's Rendering Scale to 100%. Press Home for further Luma controls.

**Remove mod** restores files that existed before this helper's first installation and removes
the files it added. If you installed over an older mod, that older mod returns.
Keep `.refantazio-enhanced` in the game folder: it holds original backups and the installation record.
Settings and shader caches remain after removal. Changed managed files cause a conflict
instead of an overwrite. Restore the expected files before retrying; do not delete the backup folder.
Missing files can be repaired with Install mod and do not prevent removal.
**Recover interrupted operation** rolls back an unfinished install, removal, or settings save.

The helper has no network updater, downloads, telemetry, or account sign-in. To change versions,
download a new helper manually and install its bundled version. The modified ReShade loader has no
update check. NVIDIA's proprietary runtime/driver is outside this project's source audit.

The mod remains experimental. Comparative gameplay improvement over upstream Luma is not yet established.
See CREDITS.md, LICENSE.md, and the bundled license notices for upstream attribution and component terms.
