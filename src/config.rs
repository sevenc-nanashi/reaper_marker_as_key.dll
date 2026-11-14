use anyhow::{Context, ensure};
use regex::Regex;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsString,
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    process::Command,
    sync::LazyLock,
};

const DEFAULT: &str = include_str!("../reaper_marker_as_key.toml");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    scales: BTreeMap<String, Scale>,
}

#[derive(Deserialize)]
#[serde(untagged, deny_unknown_fields)]
enum Scale {
    Mask(i32),
    Alias(String),
    File {
        path: PathBuf,
        scale: String,
    },
}

pub unsafe fn path(instance: reaper_low::raw::HINSTANCE) -> anyhow::Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    let length = unsafe {
        winapi::um::libloaderapi::GetModuleFileNameW(
            instance.cast(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
        )
    } as usize;
    if length == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    ensure!(length < buffer.len(), "DLL path is too long");
    Ok(PathBuf::from(OsString::from_wide(&buffer[..length])).with_extension("toml"))
}

pub fn ensure_file(path: &Path) -> anyhow::Result<()> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file.write_all(DEFAULT.as_bytes())?,
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).with_context(|| format!("Create {}", path.display())),
    }
    Ok(())
}

pub fn open(path: &Path) -> anyhow::Result<()> {
    ensure_file(path)?;
    Command::new("notepad.exe")
        .arg(path)
        .spawn()
        .context("Open configuration in Notepad")?;
    Ok(())
}

pub fn load(path: &Path) -> anyhow::Result<BTreeMap<String, i32>> {
    ensure_file(path)?;
    let source = fs::read_to_string(path).with_context(|| format!("Read {}", path.display()))?;
    let definitions: Config =
        toml::from_str(&source).with_context(|| format!("Parse {}", path.display()))?;
    let directory = path
        .parent()
        .context("Configuration has no parent directory")?;
    definitions
        .scales
        .keys()
        .map(|name| {
            ensure!(
                !name.is_empty() && !name.contains([']', '\r', '\n', '\0']),
                "Invalid scale name: {name:?}"
            );
            let mut current = name;
            let mut visited = BTreeSet::new();
            let mask = loop {
                ensure!(
                    visited.insert(current),
                    "Circular scale alias: {name:?} -> {current:?}"
                );
                let definition = definitions
                    .scales
                    .get(current)
                    .with_context(|| format!("Unknown scale alias: {name:?} -> {current:?}"))?;
                match definition {
                    Scale::Mask(mask) => break *mask,
                    Scale::Alias(target) => current = target,
                    Scale::File { path, scale } => {
                        break read_reascale(&directory.join(path), &scale)?;
                    }
                }
            };
            ensure!(
                (1..=0xFFF).contains(&mask) && mask & 1 != 0,
                "Scale {name:?} must be a 12-bit mask containing the root"
            );
            Ok((name.clone(), mask))
        })
        .collect()
}

fn read_reascale(path: &Path, name: &str) -> anyhow::Result<i32> {
    static DEFINITION: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r#"^0[ \t]+"([^"]+)"[ \t]+([0-9A-Za-z]{12})[ \t]*(?:#.*)?$"#)
            .expect("valid reascale pattern")
    });
    let source = fs::read_to_string(path).with_context(|| format!("Read {}", path.display()))?;
    for (index, line) in source.trim_start_matches('\u{feff}').lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let kind = line.split_whitespace().next().expect("nonempty line");
        match kind {
            "-2" | "-1" | "1" | "2" | "3" => continue,
            "0" => {}
            _ => anyhow::bail!("Invalid reascale entry at {}:{}", path.display(), index + 1),
        }
        let captures = DEFINITION
            .captures(line)
            .with_context(|| format!("Invalid scale at {}:{}", path.display(), index + 1))?;
        if captures[1] != *name {
            continue;
        }
        return Ok(captures[2]
            .bytes()
            .enumerate()
            .filter(|(_, digit)| *digit != b'0')
            .map(|(index, _)| 1 << index)
            .sum());
    }
    anyhow::bail!("Scale {name:?} not found in {}", path.display())
}
