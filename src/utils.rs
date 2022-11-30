use crate::HOME;
use anyhow::{Context, Result};
use std::{
    fs::{self, File},
    path::Path,
};

/// Refreshes desktop icons for applications installed with Nix
pub fn refreshicons() -> Result<()> {
    let desktoppath = &format!("{}/.local/share/applications", &*HOME);
    let iconpath = &format!("{}/.local/share/icons/nixrefresh.png", &*HOME);
    fs::create_dir_all(desktoppath)?;
    fs::create_dir_all(&format!("{}/.local/share/icons", &*HOME))?;
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
        std::os::unix::fs::symlink(filepath, localpath)?;
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
