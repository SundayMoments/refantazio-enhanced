use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
};

pub const STATE_DIR: &str = ".refantazio-enhanced";
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn valid_hash(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

pub fn valid_relative(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && !name.contains(['\\', ':', '\0']),
        "Invalid package path"
    );
    for part in name.split('/') {
        ensure!(
            !part.is_empty() && part != "." && part != ".." && !part.ends_with(['.', ' ']),
            "Invalid package path: {name}"
        );
        ensure!(
            !part
                .chars()
                .any(|c| c.is_control() || "<>\"|?*".contains(c)),
            "Invalid package path: {name}"
        );
        let stem = part.split('.').next().unwrap().to_ascii_uppercase();
        ensure!(
            !matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
                && !(stem.len() == 4
                    && (stem.starts_with("COM") || stem.starts_with("LPT"))
                    && matches!(stem.as_bytes()[3], b'1'..=b'9')),
            "Reserved Windows filename"
        );
    }
    Ok(())
}
pub fn managed(name: &str) -> Result<()> {
    valid_relative(name)?;
    ensure!(
        name.starts_with("Luma/")
            || matches!(
                name,
                "dxgi.dll"
                    | "nvngx_dlss.dll"
                    | "Luma-Metaphor ReFantazio.addon"
                    | "README-FORK.txt"
                    | "LUMA-LICENSE.txt"
                    | "msvcp140.dll"
                    | "msvcp140_atomic_wait.dll"
                    | "vcruntime140.dll"
                    | "vcruntime140_1.dll"
            ),
        "Unmanaged package path: {name}"
    );
    Ok(())
}

// Reject reparse points in every descendant, including the target itself.
// The user-selected root is canonicalized once before entering this layer.
pub fn safe_path(root: &Path, name: &str) -> Result<PathBuf> {
    valid_relative(name)?;
    let mut path = root.to_path_buf();
    for part in name.split('/') {
        path.push(part);
        match fs::symlink_metadata(&path) {
            Ok(m) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    ensure!(
                        m.file_attributes() & 0x400 == 0,
                        "A junction or symbolic link blocks this operation: {name}"
                    );
                }
                ensure!(
                    !m.is_symlink(),
                    "A symbolic link blocks this operation: {name}"
                );
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(path)
}
pub fn read_optional(path: &Path) -> Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}
pub fn current(root: &Path, name: &str) -> Result<Option<String>> {
    Ok(read_optional(&safe_path(root, name)?)?.as_deref().map(hash))
}
pub fn atomic_write(root: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    let path = safe_path(root, name)?;
    let parent = path.parent().context("Missing parent directory")?;
    fs::create_dir_all(parent)?;
    safe_path(root, name)?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)
        .context("Cannot write here. Try running the helper as administrator.")?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    temp.persist(&path)
        .map_err(|e| e.error)
        .context("Could not replace file; close the game and other mod tools")?;
    Ok(())
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Receipt {
    pub version: String,
    pub files: BTreeMap<String, Entry>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub original: Option<String>,
    pub installed: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub name: String,
    pub before: Option<String>,
    pub after: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Journal {
    before: Receipt,
    after: Receipt,
    changes: Vec<Change>,
}

pub struct Store {
    root: PathBuf,
    _lock: File,
}
impl Store {
    pub fn open(root: &Path) -> Result<Self> {
        let root = root
            .canonicalize()
            .context("Select an existing game folder")?;
        let dir = safe_path(&root, STATE_DIR)?;
        fs::create_dir_all(dir)?;
        let lock = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(safe_path(&root, &format!("{STATE_DIR}/lock"))?)?;
        lock.try_lock()
            .context("Another helper operation is already running")?;
        Ok(Self { root, _lock: lock })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn pending(&self) -> Result<bool> {
        Ok(safe_path(&self.root, &format!("{STATE_DIR}/journal.json"))?.exists())
    }
    fn validate_receipt(receipt: &Receipt) -> Result<()> {
        let mut names = BTreeSet::new();
        for (name, entry) in &receipt.files {
            managed(name)?;
            ensure!(
                names.insert(name.to_ascii_lowercase()),
                "Duplicate installed path"
            );
            ensure!(
                valid_hash(&entry.installed) && entry.original.as_deref().is_none_or(valid_hash),
                "Invalid backup hash"
            );
        }
        Ok(())
    }
    pub fn receipt(&self) -> Result<Receipt> {
        let path = safe_path(&self.root, &format!("{STATE_DIR}/installed.json"))?;
        let receipt: Receipt = match read_optional(&path)? {
            Some(b) => serde_json::from_slice(&b)
                .context("Installation record is damaged; keep the backup folder")?,
            None => Receipt::default(),
        };
        Self::validate_receipt(&receipt)?;
        Ok(receipt)
    }
    fn save_receipt(&self, receipt: &Receipt) -> Result<()> {
        atomic_write(
            &self.root,
            &format!("{STATE_DIR}/installed.json"),
            &serde_json::to_vec_pretty(receipt)?,
        )
    }
    pub fn blob(&self, hash: &str) -> Result<Vec<u8>> {
        ensure!(valid_hash(hash), "Invalid backup hash");
        let b = fs::read(safe_path(
            &self.root,
            &format!("{STATE_DIR}/backups/{hash}"),
        )?)
        .context("A required backup is missing")?;
        ensure!(
            self::hash(&b) == hash,
            "Backup checksum mismatch; no files changed"
        );
        Ok(b)
    }
    pub fn put(&self, bytes: &[u8]) -> Result<String> {
        let h = hash(bytes);
        let name = format!("{STATE_DIR}/backups/{h}");
        if safe_path(&self.root, &name)?.exists() {
            self.blob(&h)?;
        } else {
            atomic_write(&self.root, &name, bytes)?;
        }
        Ok(h)
    }
    pub fn backup(&self, name: &str) -> Result<Option<String>> {
        read_optional(&safe_path(&self.root, name)?)?
            .map(|b| self.put(&b))
            .transpose()
    }
    fn restore(&self, name: &str, value: &Option<String>) -> Result<()> {
        if let Some(h) = value {
            atomic_write(&self.root, name, &self.blob(h)?)?;
        } else {
            let p = safe_path(&self.root, name)?;
            if p.exists() {
                fs::remove_file(p)?;
            }
        }
        Ok(())
    }
    fn validate_journal(&self, j: &Journal) -> Result<()> {
        Self::validate_receipt(&j.before)?;
        Self::validate_receipt(&j.after)?;
        let mut names = BTreeSet::new();
        for c in &j.changes {
            if c.name != "ReShade.ini" {
                managed(&c.name)?;
            }
            ensure!(
                names.insert(c.name.to_ascii_lowercase()),
                "Duplicate recovery path"
            );
            for h in [&c.before, &c.after].into_iter().flatten() {
                self.blob(h)?;
            }
            let actual = current(&self.root, &c.name)?;
            ensure!(
                actual == c.before || actual == c.after,
                "File changed since the interrupted operation: {}. Keep backups and resolve this conflict first.",
                c.name
            );
        }
        Ok(())
    }
    pub fn recover(&self) -> Result<()> {
        let path = safe_path(&self.root, &format!("{STATE_DIR}/journal.json"))?;
        let j: Journal = serde_json::from_slice(&fs::read(&path)?)?;
        self.validate_journal(&j)?;
        for c in j.changes.iter().rev() {
            if current(&self.root, &c.name)? != c.before {
                self.restore(&c.name, &c.before)?;
            }
        }
        self.save_receipt(&j.before)?;
        fs::remove_file(path)?;
        Ok(())
    }
    pub fn transact(&self, after: Receipt, changes: Vec<Change>) -> Result<()> {
        ensure!(
            !self.pending()?,
            "An interrupted operation needs recovery first"
        );
        let journal = Journal {
            before: self.receipt()?,
            after,
            changes,
        };
        self.validate_journal(&journal)?;
        for c in &journal.changes {
            ensure!(
                current(&self.root, &c.name)? == c.before,
                "File changed during preparation: {}",
                c.name
            );
        }
        atomic_write(
            &self.root,
            &format!("{STATE_DIR}/journal.json"),
            &serde_json::to_vec_pretty(&journal)?,
        )?;
        let result = (|| {
            for c in &journal.changes {
                ensure!(
                    current(&self.root, &c.name)? == c.before,
                    "File changed during installation: {}",
                    c.name
                );
                self.restore(&c.name, &c.after)?;
                ensure!(
                    current(&self.root, &c.name)? == c.after,
                    "File verification failed: {}",
                    c.name
                );
            }
            self.save_receipt(&journal.after)?;
            fs::remove_file(safe_path(&self.root, &format!("{STATE_DIR}/journal.json"))?)?;
            Ok(())
        })();
        if let Err(error) = result {
            return match self.recover() {
                Ok(()) => Err(error).context("Operation failed; previous files restored"),
                Err(recovery) => Err(error).context(format!("Recovery is needed: {recovery:#}")),
            };
        }
        Ok(())
    }
}

#[derive(Deserialize)]
pub struct Manifest {
    pub version: String,
    pub game_sha256: String,
    #[cfg_attr(not(any(test, feature = "bundled")), allow(dead_code))]
    pub files: BTreeMap<String, String>,
}
pub struct Package {
    pub manifest: Manifest,
    pub files: BTreeMap<String, Vec<u8>>,
}
impl Package {
    #[cfg_attr(not(any(test, feature = "bundled")), allow(dead_code))]
    pub fn read(zip: &[u8], manifest: &str) -> Result<Self> {
        let manifest: Manifest = serde_json::from_str(manifest)?;
        ensure!(
            valid_hash(&manifest.game_sha256) && !manifest.files.is_empty(),
            "Invalid package manifest"
        );
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip))?;
        ensure!(
            archive.len() == manifest.files.len(),
            "Package file list differs from manifest"
        );
        let mut files = BTreeMap::new();
        let mut names = BTreeSet::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let mut f = archive.by_index(i)?;
            let name = f.name().to_owned();
            managed(&name)?;
            ensure!(!f.is_dir() && !f.is_symlink(), "Unsupported package entry");
            ensure!(
                names.insert(name.to_ascii_lowercase()),
                "Duplicate package filename"
            );
            total = total.checked_add(f.size()).context("Package too large")?;
            ensure!(total <= 512 * 1024 * 1024, "Package too large");
            let expected = manifest
                .files
                .get(&name)
                .context("Unexpected package file")?;
            let mut bytes = Vec::new();
            f.read_to_end(&mut bytes)?;
            ensure!(
                &hash(&bytes) == expected,
                "Package checksum mismatch: {name}"
            );
            files.insert(name, bytes);
        }
        Ok(Self { manifest, files })
    }
    pub fn install(&self, store: &Store, replace: bool) -> Result<usize> {
        ensure!(
            !store.pending()?,
            "An interrupted operation needs recovery first"
        );
        let exe = safe_path(store.root(), "METAPHOR.exe")?;
        ensure!(
            hash(&fs::read(exe).context("Choose the folder containing METAPHOR.exe")?)
                == self.manifest.game_sha256,
            "Unsupported game version. This bundle supports Steam build 18330018 only."
        );
        for sub in ["", "scripts/", "plugins/"] {
            for file in ["MetaphorFix.asi", "MetaphorFix.dll"] {
                ensure!(
                    !safe_path(store.root(), &format!("{sub}{file}"))?.exists(),
                    "Remove the separate MetaphorFix installation first; its hooks conflict with this combined mod"
                );
            }
        }
        for file in fs::read_dir(store.root())? {
            let name = file?.file_name().to_string_lossy().to_ascii_lowercase();
            if (name.starts_with("luma")
                && (name.ends_with(".addon") || name.ends_with(".addon64"))
                && name != "luma-metaphor refantazio.addon")
                || name == "d3d11.dll"
            {
                bail!(
                    "Another graphics loader/addon is installed ({name}); resolve that conflict first"
                );
            }
        }
        let before = store.receipt()?;
        let mut after = Receipt {
            version: self.manifest.version.clone(),
            files: BTreeMap::new(),
        };
        let names: BTreeSet<_> = before
            .files
            .keys()
            .chain(self.files.keys())
            .cloned()
            .collect();
        // Complete all conflict checks before creating any backup or changing game files.
        for name in &names {
            let actual = current(store.root(), name)?;
            if let Some(e) = before.files.get(name) {
                ensure!(
                    actual.is_none() || actual.as_ref() == Some(&e.installed),
                    "Installed file changed: {name}. Restore it before replacing/removing the mod."
                );
                if let Some(h) = &e.original {
                    store.blob(h)?;
                }
            } else {
                ensure!(
                    actual.is_none() || replace,
                    "Existing mod files found. Enable 'Back up and replace existing files' to continue."
                );
            }
        }
        let mut changes = Vec::new();
        for name in names {
            let actual = store.backup(&name)?;
            let original = before
                .files
                .get(&name)
                .map_or_else(|| actual.clone(), |e| e.original.clone());
            let next = if let Some(bytes) = self.files.get(&name) {
                let installed = store.put(bytes)?;
                after.files.insert(
                    name.clone(),
                    Entry {
                        original,
                        installed: installed.clone(),
                    },
                );
                Some(installed)
            } else {
                original
            };
            changes.push(Change {
                name,
                before: actual,
                after: next,
            });
        }
        store.transact(after, changes)?;
        Ok(self.files.len())
    }
}
pub fn uninstall(store: &Store) -> Result<usize> {
    ensure!(
        !store.pending()?,
        "An interrupted operation needs recovery first"
    );
    let receipt = store.receipt()?;
    ensure!(
        !receipt.files.is_empty(),
        "No helper-managed installation found in this folder"
    );
    let mut changes = Vec::new();
    for (name, e) in receipt.files {
        let actual = current(store.root(), &name)?;
        ensure!(
            actual.is_none() || actual.as_ref() == Some(&e.installed),
            "Installed file changed: {name}. Nothing removed."
        );
        if let Some(h) = &e.original {
            store.blob(h)?;
        }
        let before = store.backup(&name)?;
        changes.push(Change {
            name,
            before,
            after: e.original,
        });
    }
    let count = changes.len();
    store.transact(Receipt::default(), changes)?;
    Ok(count)
}
