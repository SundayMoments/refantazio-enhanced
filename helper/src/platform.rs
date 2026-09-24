use anyhow::{Context, Result, ensure};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub fn quoted_tokens(text: &str) -> Vec<String> {
    let mut chars = text.chars().peekable();
    let mut tokens = Vec::new();
    while let Some(c) = chars.next() {
        if c == '/' && chars.peek() == Some(&'/') {
            for c in chars.by_ref() {
                if c == '\n' {
                    break;
                }
            }
        } else if c == '"' {
            let mut s = String::new();
            while let Some(c) = chars.next() {
                if c == '"' {
                    break;
                }
                if c == '\\' && matches!(chars.peek(), Some('\\' | '"')) {
                    s.push(chars.next().unwrap());
                } else {
                    s.push(c);
                }
            }
            tokens.push(s);
        }
    }
    tokens
}
pub fn discover() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    #[cfg(windows)]
    {
        use winreg::{RegKey, enums::*};
        for (hive, key, name) in [
            (HKEY_CURRENT_USER, "Software\\Valve\\Steam", "SteamPath"),
            (HKEY_LOCAL_MACHINE, "SOFTWARE\\Valve\\Steam", "InstallPath"),
        ] {
            for view in [KEY_WOW64_32KEY, KEY_WOW64_64KEY] {
                if let Ok(k) = RegKey::predef(hive).open_subkey_with_flags(key, KEY_READ | view)
                    && let Ok(v) = k.get_value::<String, _>(name)
                {
                    roots.push(PathBuf::from(v));
                }
            }
        }
    }
    if let Some(p) = std::env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(p).join("Steam"));
    }
    let mut libraries = roots.clone();
    for root in roots {
        if let Ok(vdf) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for pair in quoted_tokens(&vdf).windows(2) {
                if pair[0] == "path" {
                    libraries.push(PathBuf::from(&pair[1]));
                }
            }
        }
    }
    let mut games = Vec::new();
    for library in libraries {
        let mut dir = "METAPHOR".to_owned();
        if let Ok(vdf) = fs::read_to_string(library.join("steamapps/appmanifest_2679460.acf"))
            && let Some(pair) = quoted_tokens(&vdf)
                .windows(2)
                .find(|v| v[0] == "installdir")
        {
            dir = pair[1].clone();
        }
        if crate::storage::valid_relative(&dir).is_err() || dir.contains('/') {
            continue;
        }
        let path = library.join("steamapps/common").join(dir);
        if path.join("METAPHOR.exe").is_file()
            && let Ok(p) = path.canonicalize()
            && !games.contains(&p)
        {
            games.push(p);
        }
    }
    games
}
pub fn closed_game() -> Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::{
            Foundation::{CloseHandle, ERROR_NO_MORE_FILES, GetLastError, INVALID_HANDLE_VALUE},
            System::Diagnostics::ToolHelp::*,
        };
        let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        ensure!(
            snap != INVALID_HANDLE_VALUE,
            "Could not check whether the game is running"
        );
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut ok = Process32FirstW(snap, &mut entry);
        let mut running = false;
        while ok != 0 {
            let len = entry
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(entry.szExeFile.len());
            if String::from_utf16_lossy(&entry.szExeFile[..len])
                .eq_ignore_ascii_case("METAPHOR.exe")
            {
                running = true;
                break;
            }
            ok = Process32NextW(snap, &mut entry);
        }
        let last = GetLastError();
        CloseHandle(snap);
        ensure!(
            !running,
            "Close Metaphor normally before installing, removing, recovering, or saving settings"
        );
        ensure!(
            last == ERROR_NO_MORE_FILES,
            "Could not finish checking running processes"
        );
    }
    Ok(())
}
pub fn game_root(path: &Path) -> Result<PathBuf> {
    let root = path
        .canonicalize()
        .context("Select the Metaphor game folder")?;
    ensure!(
        crate::storage::safe_path(&root, "METAPHOR.exe")?.is_file(),
        "Choose the folder containing METAPHOR.exe"
    );
    Ok(root)
}
pub fn elevate() -> Result<()> {
    #[cfg(windows)]
    unsafe {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
        let file: Vec<u16> = std::env::current_exe()?
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect();
        let verb: Vec<u16> = "runas\0".encode_utf16().collect();
        let result = ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            file.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        );
        ensure!(
            result as isize > 32,
            "Administrator launch was canceled or failed"
        );
    }
    Ok(())
}
