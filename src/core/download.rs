use crate::utils::fs as cleen_fs;
use anyhow::Result;
use flate2::read::GzDecoder;
use std::fs::File;
use std::path::Path;
use tar::Archive;
use zip::ZipArchive;

pub struct Downloader;

impl Default for Downloader {
    fn default() -> Self {
        Self
    }
}

impl Downloader {
    pub fn new() -> Self {
        Self
    }

    pub fn download_file(&self, url: &str, destination: &Path) -> Result<()> {
        println!("Downloading from {url}...");

        // Ensure parent directory exists
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = std::process::Command::new("curl")
            .arg("-L") // Follow redirects
            .arg("-s") // Silent
            .arg("-H")
            .arg("User-Agent: cleen/0.1.0")
            .arg("-o")
            .arg(destination)
            .arg(url)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "Failed to download file: curl exited with status {:?}",
                output.status.code()
            ));
        }

        // curl writes inherit `com.apple.provenance` on macOS Sequoia when
        // the calling process itself carries it. Strip on the freshly-written
        // file so downstream extract/copy operations start from a clean slate.
        cleen_fs::strip_macos_xattrs(destination);

        println!("Downloaded to {destination:?}");
        Ok(())
    }

    pub fn extract_archive(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        println!("Extracting {archive_path:?} to {destination:?}");

        std::fs::create_dir_all(destination)?;

        let file_name = archive_path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid archive file name"))?;

        if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
            self.extract_tar_gz(archive_path, destination)?;
        } else if file_name.ends_with(".zip") {
            self.extract_zip(archive_path, destination)?;
        } else {
            return Err(anyhow::anyhow!("Unsupported archive format: {}", file_name));
        }

        // Files created by a provenance-tagged process inherit the xattr.
        // Strip recursively so the installed version dir is clean.
        cleen_fs::strip_macos_xattrs_recursive(destination);

        println!("Extraction completed");
        Ok(())
    }

    fn extract_tar_gz(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        let file = File::open(archive_path)?;
        let decoder = GzDecoder::new(file);
        let mut archive = Archive::new(decoder);
        archive.unpack(destination)?;
        Ok(())
    }

    fn extract_zip(&self, archive_path: &Path, destination: &Path) -> Result<()> {
        let file = File::open(archive_path)?;
        let mut archive = ZipArchive::new(file)?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let outpath = match file.enclosed_name() {
                Some(path) => destination.join(path),
                None => continue,
            };

            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        std::fs::create_dir_all(p)?;
                    }
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    #[test]
    fn extract_zip_restores_directories_files_and_permissions() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let archive_path = tmp.path().join("release.zip");

        let mut writer = ZipWriter::new(File::create(&archive_path).expect("create zip"));
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o755);
        writer.add_directory("bin/", options).expect("add dir");
        writer.start_file("bin/cln", options).expect("start file");
        writer.write_all(b"compiler-binary-bytes").expect("write file");
        writer.finish().expect("finish zip");

        let destination = tmp.path().join("out");
        Downloader::new()
            .extract_archive(&archive_path, &destination)
            .expect("extract zip");

        assert!(destination.join("bin").is_dir());
        let extracted = destination.join("bin/cln");
        assert_eq!(
            std::fs::read(&extracted).expect("read extracted file"),
            b"compiler-binary-bytes"
        );

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&extracted).expect("metadata").permissions().mode();
            assert_eq!(mode & 0o777, 0o755, "executable bit must survive extraction");
        }
    }
}
