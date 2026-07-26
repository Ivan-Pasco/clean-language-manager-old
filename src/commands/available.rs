use crate::core::{github::GitHubClient, version::normalize};
use anyhow::Result;

pub fn list_available_versions() -> Result<()> {
    let github_client = GitHubClient::new(None);

    match github_client.get_releases("Ivan-Pasco", "clean-language-compiler-old") {
        Ok(releases) => {
            if releases.is_empty() {
                println!("No releases available yet.");
                println!(
                    "Check: https://github.com/Ivan-Pasco/clean-language-compiler-old/releases"
                );
            } else {
                println!("Available versions:");

                // Reverse the order so newest version appears at the bottom (next to cursor)
                for (i, release) in releases.iter().rev().enumerate() {
                    let clean_version = normalize::to_clean_version(&release.tag_name);
                    // Latest is now at the end (last index)
                    let status = if i == releases.len() - 1 {
                        " (latest)"
                    } else {
                        ""
                    };
                    let prerelease = if release.prerelease {
                        " [prerelease]"
                    } else {
                        ""
                    };

                    print!("  {}{}{}", clean_version, status, prerelease);
                    if !release.name.is_empty() && release.name != release.tag_name {
                        println!(" - {}", release.name);
                    } else {
                        println!();
                    }
                }

                println!();
                println!("Install: cleen install <version>");
            }
        }
        Err(e) => {
            println!("Unable to fetch releases: {e}");
            println!("Check: https://github.com/Ivan-Pasco/clean-language-compiler-old/releases");
        }
    }

    Ok(())
}
