//! Platform-specific replacement of portable packages.
use std::{
    fs, io,
    io::Read,
    path::{Path, PathBuf},
};

pub fn launch_path() -> io::Result<PathBuf> {
    #[cfg(target_os = "linux")]
    if let Some(path) = std::env::var_os("APPIMAGE") {
        return fs::canonicalize(path);
    }
    std::env::current_exe()
}

pub fn restart(path: &Path) -> io::Result<std::process::Child> {
    let mut command = std::process::Command::new(path);
    command.args(std::env::args_os().skip(1));
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_some() {
        // Don't make the new image resolve helpers/libraries in the old mount.
        for name in [
            "APPDIR",
            "APPIMAGE",
            "ARGV0",
            "LD_LIBRARY_PATH",
            "GTK_PATH",
            "GTK_EXE_PREFIX",
            "GTK_DATA_PREFIX",
            "GTK_IM_MODULE_FILE",
            "GDK_PIXBUF_MODULE_FILE",
            "GIO_MODULE_DIR",
            "GI_TYPELIB_PATH",
            "WEBKIT_EXEC_PATH",
            "GSETTINGS_SCHEMA_DIR",
        ] {
            command.env_remove(name);
        }
    }
    command.spawn()
}

fn check_header(path: &Path, signature: &[u8], offset: u64) -> io::Result<()> {
    use std::io::{Seek, SeekFrom};
    let mut file = fs::File::open(path)?;
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; signature.len()];
    file.read_exact(&mut bytes)?;
    if bytes != signature {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Downloaded package has an invalid executable header",
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn install(download: &Path) -> Result<(), Box<dyn std::error::Error>> {
    check_header(download, b"MZ", 0)?;
    self_replace::self_replace(download)?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub fn install(download: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let image = std::env::var_os("APPIMAGE")
        .ok_or("Automatic Linux updates require running the AppImage download.")?;
    replace_appimage(download, &fs::canonicalize(image)?)?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn replace_appimage(download: &Path, destination: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    check_header(download, b"\x7fELF", 0)?;
    check_header(download, b"AI\x02", 8)?;
    let parent = destination.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "AppImage has no parent directory",
        )
    })?;
    // Stage beside the outer image. Rename is atomic even while the old image is mounted.
    let staged = tempfile::NamedTempFile::new_in(parent)?;
    fs::copy(download, staged.path())?;
    let mode = fs::metadata(destination)?.permissions().mode() | 0o100;
    fs::set_permissions(staged.path(), fs::Permissions::from_mode(mode))?;
    staged.as_file().sync_all()?;
    staged.persist(destination).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn install(download: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let executable = std::env::current_exe()?;
    if !executable.ends_with("Contents/MacOS/prntscrape") {
        return Err("Automatic macOS updates require running Prntscrape.app.".into());
    }
    let bundle = executable
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or("Cannot locate app bundle")?;
    if bundle.extension().is_none_or(|ext| ext != "app") {
        return Err("Cannot locate app bundle".into());
    }
    let staging = tempfile::Builder::new()
        .prefix(".prntscrape-update-")
        .tempdir_in(bundle.parent().ok_or("App has no parent directory")?)?;
    let output = std::process::Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(download)
        .arg(staging.path())
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "Cannot unpack app: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let updated = staging.path().join("Prntscrape.app");
    if !updated.join("Contents/MacOS/prntscrape").is_file() {
        return Err("Release is missing the app executable".into());
    }
    let output = std::process::Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict"])
        .arg(&updated)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "App signature verification failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let backup = staging.path().join("previous.app");
    fs::rename(bundle, &backup)?;
    if let Err(error) = fs::rename(&updated, bundle) {
        if let Err(rollback) = fs::rename(&backup, bundle) {
            let recovery = staging.keep();
            return Err(format!("Update failed ({error}); restore failed ({rollback}). Original app preserved at {}", recovery.join("previous.app").display()).into());
        }
        return Err(error.into());
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn replaces_outer_image_and_preserves_executable_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("App with spaces.AppImage");
        let downloaded = dir.path().join("download");
        let content = b"\x7fELF\x02\x01\x01\x00AI\x02\x00new image";
        fs::write(&installed, b"previous image").unwrap();
        fs::set_permissions(&installed, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(&downloaded, content).unwrap();
        replace_appimage(&downloaded, &installed).unwrap();
        assert_eq!(fs::read(&installed).unwrap(), content);
        assert_eq!(
            fs::metadata(installed).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[test]
    fn invalid_download_leaves_existing_image_intact() {
        let dir = tempfile::tempdir().unwrap();
        let installed = dir.path().join("Prntscrape.AppImage");
        let downloaded = dir.path().join("download");
        fs::write(&installed, b"previous image").unwrap();
        fs::write(&downloaded, b"<html>download failed</html>").unwrap();
        assert!(replace_appimage(&downloaded, &installed).is_err());
        assert_eq!(fs::read(installed).unwrap(), b"previous image");
    }
}
