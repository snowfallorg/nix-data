use crate::HOME;
use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    path::Path, io::{Read, Write},
};

/// Refreshes desktop icons for applications installed with Nix
pub fn refreshicons() -> Result<()> {
    let desktoppath = &format!("{}/.local/share/applications", &*HOME);
    let iconpath = &format!("{}/.local/share/icons/nixrefresh.png", &*HOME);
    fs::create_dir_all(desktoppath)?;
    fs::create_dir_all(&format!("{}/.local/share/icons", &*HOME))?;

    // Clean up old files
    for filename in (fs::read_dir(desktoppath)?).flatten() {
        if filename.file_type()?.is_file() && fs::read_to_string(filename.path())?.lines().next() == Some("# Nix Desktop Entry") {
            fs::remove_file(filename.path())?;
        }
    }

    for filename in
        (fs::read_dir(&format!("{}/.nix-profile/share/applications", &*HOME))?).flatten()
    {
        let filepath = filename.path().to_str().context("file path")?.to_string();
        let localpath = format!(
            "{}/{}",
            desktoppath,
            filename.file_name().to_str().context("file name")?
        );
        if Path::new(&localpath).exists() {
            fs::remove_file(&localpath)?;
        }
        fs::copy(&filepath, &localpath)?;
        // Write "# Nix Desktop Entry" to the top of the file
        let mut file = File::open(&localpath)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        contents = format!("# Nix Desktop Entry\n{}", contents);
        fs::remove_file(&localpath)?;
        let mut file = File::create(&localpath)?;
        file.write_all(contents.as_bytes())?;
        let mut perms = fs::metadata(&localpath)?.permissions();
        perms.set_readonly(true);
        fs::set_permissions(&localpath, perms)?;
    }

    if Path::new(iconpath).exists() {
        fs::remove_file(iconpath)?;
    }
    File::create(iconpath)?;
    if Path::new(iconpath).exists() {
        fs::remove_file(iconpath)?;
    }

    Ok(())
}
