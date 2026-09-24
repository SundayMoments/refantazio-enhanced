# Building the offline helper

First build and package the mod using the [technical notes](TECHNICAL.md#build-and-test).
Install Rust stable for `x86_64-pc-windows-msvc` and Visual Studio 2022 C++ Build Tools.
The tested Rust version is 1.97.1; dependency versions are locked in `helper/Cargo.lock`.

Fetch development dependencies once (this is the only helper step that needs the network):

```powershell
$env:CARGO_NET_OFFLINE = 'false'
cargo fetch --locked --manifest-path helper/Cargo.toml
```

Then build locally:

```powershell
cargo test --offline --locked --manifest-path helper/Cargo.toml
cargo clippy --offline --locked --manifest-path helper/Cargo.toml --all-targets -- -D warnings
python tools/build_helper.py
python tools/test_runtime.py
```

The builder verifies `artifacts/fork` against its package manifest, collects dependency
notices, includes the four required signed x64 Visual C++ runtime DLLs, and embeds the
compressed bundle and per-file SHA-256 manifest in one executable. It uses the installed
VS release redist directory, or the installed system runtime if that is newer;
`--crt-dir` can select another licensed release copy. All four DLLs must be signed
Microsoft x64 binaries meeting the minimum version compiled into the addon.
An old runtime is rejected before packaging. `test_runtime.py` exercises real
mutex, condition-variable, and semaphore operations with the exact bundled DLLs.
It does not fetch dependencies or run any installer. Rust links its CRT statically.

Outputs are under `artifacts/distribution`: the helper EXE, SHA-256 file, and `NOTICE.txt`.
The helper supports `--verify-payload` for a read-only package check and
`--preview <output.png>` to render a screenshot with a fictitious path and file mutations
disabled. A normal Cargo build without `bundled` builds a UI development version with installation disabled.

The builder remaps local profile/workspace paths and checks for embedded profile paths.
Before publishing, also review the complete source diff, commit metadata, notices,
uncompressed embedded files, PE debug records, and rendered previews for private data.
Do not upload PDBs, game files, user settings, local logs, or original-file backups.

Runtime notice provenance:

- NVIDIA license: `NVIDIA/DLSS` revision `374959484e79a640feaba44c93ac8cfb0a03f5b5`, `LICENSE.txt`;
  SHA-256 `d4216e39ebef5f9b50a6712ebb37beeb5379862a67733a9999c651f21592aaf0`.
- Microsoft runtime terms: [official terms](https://visualstudio.microsoft.com/license-terms/vs2022-cruntime/),
  text extracted from the linked English license document; the unmodified signed runtime files
  remain subject to Microsoft's terms and Visual Studio's redistribution rights.
- Rust notices are collected from the exact Cargo packages used for the Windows build;
  Apache-2.0 alternatives are used for crates that omit a license file from their package.
  The helper embeds these notices and also installs them under `Luma/licenses`.
