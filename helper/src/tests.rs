use super::{platform, settings::Document, storage::*};
use std::{
    collections::BTreeMap,
    fs,
    io::{Cursor, Write},
};

fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("METAPHOR.exe"), b"fixture-game").unwrap();
    dir
}
fn package(version: &str, files: &[(&str, &[u8])]) -> Package {
    Package {
        manifest: Manifest {
            version: version.into(),
            game_sha256: hash(b"fixture-game"),
            files: files
                .iter()
                .map(|(n, b)| (n.to_string(), hash(b)))
                .collect(),
        },
        files: files
            .iter()
            .map(|(n, b)| (n.to_string(), b.to_vec()))
            .collect(),
    }
}
fn bundle(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (n, b) in files {
        writer
            .start_file(*n, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
#[test]
fn fresh_install_remove_and_reinstall_preserve_user_files() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    fs::write(d.path().join("ReShade.ini"), b"user-config").unwrap();
    let p = package(
        "1",
        &[
            ("dxgi.dll", b"loader"),
            ("Luma/Includes/test.hlsl", b"shader"),
        ],
    );
    assert_eq!(p.install(&s, false).unwrap(), 2);
    assert_eq!(uninstall(&s).unwrap(), 2);
    assert!(!d.path().join("dxgi.dll").exists());
    assert_eq!(
        fs::read(d.path().join("ReShade.ini")).unwrap(),
        b"user-config"
    );
    assert!(p.install(&s, false).is_ok());
}
#[test]
fn replacement_needs_consent_and_original_survives_versions() {
    let d = fixture();
    fs::write(d.path().join("dxgi.dll"), b"other-loader").unwrap();
    let s = Store::open(d.path()).unwrap();
    let p = package("1", &[("dxgi.dll", b"one"), ("Luma/old.hlsl", b"old")]);
    assert!(p.install(&s, false).is_err());
    assert_eq!(
        fs::read(d.path().join("dxgi.dll")).unwrap(),
        b"other-loader"
    );
    p.install(&s, true).unwrap();
    package("2", &[("dxgi.dll", b"two"), ("Luma/new.hlsl", b"new")])
        .install(&s, false)
        .unwrap();
    assert!(!d.path().join("Luma/old.hlsl").exists());
    uninstall(&s).unwrap();
    assert_eq!(
        fs::read(d.path().join("dxgi.dll")).unwrap(),
        b"other-loader"
    );
    assert!(!d.path().join("Luma/new.hlsl").exists());
}
#[test]
fn changed_installed_file_blocks_all_writes() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    let p = package("1", &[("dxgi.dll", b"a"), ("Luma/z", b"b")]);
    p.install(&s, false).unwrap();
    fs::write(d.path().join("Luma/z"), b"user edit").unwrap();
    assert!(uninstall(&s).is_err());
    assert!(p.install(&s, true).is_err());
    assert_eq!(fs::read(d.path().join("dxgi.dll")).unwrap(), b"a");
}
#[test]
fn wrong_game_and_hook_conflict_are_rejected() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    let p = package("1", &[("dxgi.dll", b"a")]);
    fs::write(d.path().join("METAPHOR.exe"), b"updated-game").unwrap();
    assert!(p.install(&s, false).is_err());
    fs::write(d.path().join("METAPHOR.exe"), b"fixture-game").unwrap();
    fs::create_dir(d.path().join("plugins")).unwrap();
    fs::write(d.path().join("plugins/MetaphorFix.asi"), b"x").unwrap();
    assert!(p.install(&s, false).is_err());
    assert!(!d.path().join("dxgi.dll").exists());
}
#[test]
fn backup_corruption_blocks_uninstall() {
    let d = fixture();
    fs::write(d.path().join("dxgi.dll"), b"old").unwrap();
    let s = Store::open(d.path()).unwrap();
    package("1", &[("dxgi.dll", b"new")])
        .install(&s, true)
        .unwrap();
    fs::write(
        d.path()
            .join(format!("{STATE_DIR}/backups/{}", hash(b"old"))),
        b"bad",
    )
    .unwrap();
    assert!(uninstall(&s).is_err());
    assert_eq!(fs::read(d.path().join("dxgi.dll")).unwrap(), b"new");
}
#[test]
fn interrupted_transaction_rolls_back_and_protects_edits() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    let old = s.put(b"old").unwrap();
    let new = s.put(b"new").unwrap();
    let journal = serde_json::json!({"before":{"version":"","files":{}},"after":{"version":"","files":{}},"changes":[{"name":"dxgi.dll","before":old,"after":new},{"name":"Luma/new.hlsl","before":null,"after":new}]});
    atomic_write(
        s.root(),
        &format!("{STATE_DIR}/journal.json"),
        &serde_json::to_vec(&journal).unwrap(),
    )
    .unwrap();
    fs::write(d.path().join("dxgi.dll"), b"user edit").unwrap();
    assert!(s.recover().is_err());
    assert!(s.pending().unwrap());
    fs::write(d.path().join("dxgi.dll"), b"new").unwrap();
    s.recover().unwrap();
    assert_eq!(fs::read(d.path().join("dxgi.dll")).unwrap(), b"old");
    assert!(!s.pending().unwrap());
}
#[test]
fn locks_prevent_concurrent_helper_writes() {
    let d = fixture();
    let _s = Store::open(d.path()).unwrap();
    assert!(Store::open(d.path()).is_err());
}
#[test]
fn archive_checks_hashes_names_and_exact_file_list() {
    let files = &[("dxgi.dll", b"loader".as_slice())];
    let manifest=serde_json::json!({"version":"1","game_sha256":hash(b"game"),"files":{"dxgi.dll":hash(b"loader")}}).to_string();
    assert!(Package::read(&bundle(files), &manifest).is_ok());
    assert!(Package::read(&bundle(&[("dxgi.dll", b"wrong")]), &manifest).is_err());
    assert!(
        Package::read(
            &bundle(&[("dxgi.dll", b"loader"), ("Luma/extra", b"x")]),
            &manifest
        )
        .is_err()
    );
    for name in [
        "../dxgi.dll",
        "C:/dxgi.dll",
        "Luma/../x",
        "/dxgi.dll",
        "Luma/NUL.txt",
        "Luma/x.",
        "Luma/a:b",
        "Luma\\x",
        "METAPHOR.exe",
    ] {
        assert!(managed(name).is_err(), "{name}");
    }
    let mut manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    manifest["files"]["Luma/a"] = hash(b"a").into();
    manifest["files"]["Luma/A"] = hash(b"a").into();
    assert!(
        Package::read(
            &bundle(&[("dxgi.dll", b"loader"), ("Luma/a", b"a"), ("Luma/A", b"a")]),
            &manifest.to_string()
        )
        .is_err()
    );
}
#[test]
fn ini_preserves_unknown_sections_comments_bom_and_crlf() {
    let original=b"\xef\xbb\xbf; user note\r\n[Luma]\r\nHDR=keep\r\nEngineFOV=1.100000\r\n[GENERAL]\r\nFoo=bar\r\n[STYLE]\r\nFontScale=1.000000\r\n".to_vec();
    let mut d = Document::parse(Some(original)).unwrap();
    let scale = super::settings::SETTINGS
        .iter()
        .find(|s| s.key == "FontScale")
        .unwrap();
    d.set(scale, "2".into());
    let text = String::from_utf8(d.render().unwrap()).unwrap();
    assert!(text.starts_with('\u{feff}'));
    assert!(text.contains("; user note\r\n"));
    assert!(text.contains("HDR=keep\r\n"));
    assert!(text.contains("[GENERAL]\r\nFoo=bar\r\n"));
    assert!(text.contains("EngineFOV=1.100000\r\n"));
    assert_eq!(text.matches("FontScale=").count(), 1);
    assert!(text.contains("FontScale=2\r\n"));
}
#[test]
fn ambiguous_ini_and_invalid_values_are_rejected() {
    assert!(Document::parse(Some(b"[Luma]\nEngineFOV=1\nEngineFOV=2\n".to_vec())).is_err());
    assert!(Document::parse(Some(b"[Luma]\n[Luma]\n".to_vec())).is_err());
    assert!(Document::parse(Some(vec![255])).is_err());
    let d = Document::parse(Some(b"[Luma]\nEngineFOV=NaN\n".to_vec())).unwrap();
    assert!(d.render().is_err());
}
#[test]
fn settings_save_detects_external_edits_and_leaves_receipt_unchanged() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    package("1", &[("dxgi.dll", b"new")])
        .install(&s, false)
        .unwrap();
    let receipt = s.receipt().unwrap();
    let config = Document::load(s.root()).unwrap();
    fs::write(d.path().join("ReShade.ini"), b"; external edit\n").unwrap();
    assert!(config.save(&s).is_err());
    let config = Document::load(s.root()).unwrap();
    config.save(&s).unwrap();
    assert_eq!(s.receipt().unwrap(), receipt);
    assert!(
        fs::read_to_string(d.path().join("ReShade.ini"))
            .unwrap()
            .starts_with("; external edit\n")
    );
}
#[test]
fn vdf_lexer_handles_spaces_backslashes_and_comments() {
    let t = platform::quoted_tokens(
        r#"// "ignored"
"path" "D:\\Steam Library" "installdir" "METAPHOR" "url" "https://example.invalid""#,
    );
    assert_eq!(
        t,
        vec![
            "path",
            "D:\\Steam Library",
            "installdir",
            "METAPHOR",
            "url",
            "https://example.invalid"
        ]
    );
}
#[cfg(windows)]
#[test]
fn failed_replace_restores_already_written_files() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    fs::write(d.path().join("dxgi.dll"), b"old").unwrap();
    fs::create_dir(d.path().join("Luma")).unwrap();
    fs::write(d.path().join("Luma/z"), b"locked").unwrap();
    let path = d.path().join("Luma/z");
    let original_permissions = fs::metadata(&path).unwrap().permissions();
    let mut perm = original_permissions.clone();
    perm.set_readonly(true);
    fs::set_permissions(&path, perm).unwrap();
    // BTree order puts Luma first, so explicitly order the transaction to fail second.
    let changes = vec![
        Change {
            name: "dxgi.dll".into(),
            before: s.backup("dxgi.dll").unwrap(),
            after: Some(s.put(b"new").unwrap()),
        },
        Change {
            name: "Luma/z".into(),
            before: s.backup("Luma/z").unwrap(),
            after: Some(s.put(b"new").unwrap()),
        },
    ];
    let result = s.transact(
        Receipt {
            version: "test".into(),
            files: BTreeMap::new(),
        },
        changes,
    );
    fs::set_permissions(&path, original_permissions).unwrap();
    assert!(result.is_err());
    assert_eq!(fs::read(d.path().join("dxgi.dll")).unwrap(), b"old");
    assert!(!s.pending().unwrap());
}

#[test]
fn missing_managed_files_can_be_repaired_or_removed() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    fs::write(d.path().join("dxgi.dll"), b"original").unwrap();
    let p = package("1", &[("dxgi.dll", b"loader")]);
    p.install(&s, true).unwrap();
    fs::remove_file(d.path().join("dxgi.dll")).unwrap();
    p.install(&s, false).unwrap();
    assert_eq!(fs::read(d.path().join("dxgi.dll")).unwrap(), b"loader");
    fs::remove_file(d.path().join("dxgi.dll")).unwrap();
    uninstall(&s).unwrap();
    assert_eq!(fs::read(d.path().join("dxgi.dll")).unwrap(), b"original");
}

#[test]
fn preserves_legacy_upscaling_until_user_changes_it() {
    let d = Document::parse(Some(b"[Luma]\nUseSRForUpscaling=1\n".to_vec())).unwrap();
    assert!(
        String::from_utf8(d.render().unwrap())
            .unwrap()
            .contains("UpscalingMode=1\n")
    );
    let d = Document::parse(Some(
        b"[Luma]\nUseSRForUpscaling=1\nUpscalingMode=2\n".to_vec(),
    ))
    .unwrap();
    assert!(
        String::from_utf8(d.render().unwrap())
            .unwrap()
            .contains("UpscalingMode=2\n")
    );
}

#[cfg(windows)]
#[test]
fn junctions_cannot_redirect_game_or_backup_writes() {
    use std::os::windows::process::CommandExt;
    let d = fixture();
    let outside = tempfile::tempdir().unwrap();
    let junction = d.path().join("Luma");
    let status=std::process::Command::new("powershell.exe").args(["-NoProfile","-Command","New-Item -ItemType Junction -Path $env:RE_TEST_JUNCTION -Target $env:RE_TEST_TARGET -ErrorAction Stop | Out-Null"]).env("RE_TEST_JUNCTION",&junction).env("RE_TEST_TARGET",outside.path()).creation_flags(0x08000000).status().unwrap();
    assert!(status.success());
    let s = Store::open(d.path()).unwrap();
    let result = package("1", &[("Luma/test", b"must stay inside")]).install(&s, false);
    // Remove only the junction itself, never recursively traverse its target.
    fs::remove_dir(&junction).unwrap();
    assert!(result.is_err());
    assert!(!outside.path().join("test").exists());
}

#[cfg(windows)]
#[test]
#[ignore = "Launched only as a child fixture by the process-guard test"]
fn process_fixture_child() {
    if std::env::var_os("RE_HELPER_PROCESS_FIXTURE").is_some() {
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}

#[cfg(windows)]
#[test]
fn running_game_guard_detects_named_process() {
    use std::os::windows::process::CommandExt;
    let d = tempfile::tempdir().unwrap();
    let exe = d.path().join("METAPHOR.exe");
    fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
    let mut child = std::process::Command::new(&exe)
        .args(["--exact", "tests::process_fixture_child", "--ignored"])
        .env("RE_HELPER_PROCESS_FIXTURE", "1")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .creation_flags(0x08000000)
        .spawn()
        .unwrap();
    let result = platform::closed_game();
    let _ = child.kill();
    child.wait().unwrap();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Close Metaphor"));
}

#[cfg(feature = "bundled")]
#[test]
fn full_embedded_bundle_installs_and_removes_in_sandbox_fixture() {
    let d = fixture();
    let s = Store::open(d.path()).unwrap();
    let mut p = super::bundle::load().unwrap();
    assert!(p.files.len() > 150);
    p.manifest.game_sha256 = hash(b"fixture-game");
    p.install(&s, false).unwrap();
    for (name, bytes) in &p.files {
        assert_eq!(current(s.root(), name).unwrap(), Some(hash(bytes)));
    }
    uninstall(&s).unwrap();
    for name in p.files.keys() {
        assert!(!d.path().join(name).exists());
    }
}
