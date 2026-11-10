// SPDX-License-Identifier: MIT
use reqwest::{redirect, Client, StatusCode};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    os::windows::process::CommandExt,
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};
use tempfile::TempDir;

const REPO: &str = "https://github.com/chahatesh/Hushline";
const LATEST: &str = "https://api.github.com/repos/chahatesh/Hushline/releases/latest";
const CURRENT: &str = env!("CARGO_PKG_VERSION");
const MAX_INSTALLER: u64 = 64 * 1024 * 1024;
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const SCRIPT: &str = include_str!("../windows/update.ps1");

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
struct Asset {
    name: String,
    state: String,
    size: u64,
    digest: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    installed_version: &'static str,
    available_version: Option<String>,
    published_release: bool,
    can_install: bool,
}

fn version(text: &str) -> Option<[u32; 3]> {
    let mut parts = text.split('.');
    let mut parsed = [0; 3];
    for item in &mut parsed {
        let part = parts.next()?;
        *item = part.parse::<u32>().ok()?;
        if item.to_string() != part {
            return None;
        }
    }
    parts.next().is_none().then_some(parsed)
}

fn candidate<'a>(
    release: &'a Release,
    current: &str,
) -> Result<Option<(&'a str, &'a Asset)>, String> {
    if release.draft || release.prerelease {
        return Ok(None);
    }
    let tag = release.tag_name.strip_prefix('v').unwrap_or_default();
    let parsed = version(tag).ok_or("GitHub returned an unsupported Hush release version.")?;
    if parsed <= version(current).ok_or("Hush has an unsupported installed version.")? {
        return Ok(None);
    }
    let installer = format!("Hush_{tag}_x64-setup.exe");
    let mut matches = release
        .assets
        .iter()
        .filter(|asset| asset.name == installer);
    let asset = matches
        .next()
        .filter(|asset| {
            asset.state == "uploaded"
                && (512..=MAX_INSTALLER).contains(&asset.size)
                && asset
                    .digest
                    .as_deref()
                    .and_then(|digest| digest.strip_prefix("sha256:"))
                    .is_some_and(|digest| {
                        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
                    })
        })
        .ok_or("The latest Hush release is missing a complete Windows installer and checksum.")?;
    if matches.next().is_some() {
        return Err("The latest Hush release has duplicate Windows installers.".into());
    }
    Ok(Some((tag, asset)))
}

fn client() -> Result<Client, String> {
    Client::builder()
        .user_agent(concat!("Hush/", env!("CARGO_PKG_VERSION")))
        .https_only(true)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(35))
        .redirect(redirect::Policy::custom(|attempt| {
            if attempt.previous().len() < 4
                && attempt.url().port().is_none()
                && matches!(
                    attempt.url().host_str(),
                    Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
                )
            {
                attempt.follow()
            } else {
                attempt.error("Unexpected GitHub redirect")
            }
        }))
        .build()
        .map_err(|_| "Hush could not initialize the GitHub update connection.".into())
}

async fn bounded_body(mut response: reqwest::Response, limit: u64) -> Result<Vec<u8>, String> {
    if response.content_length().is_some_and(|size| size > limit) {
        return Err("GitHub returned a larger update response than expected.".into());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The GitHub update download was interrupted. Try again.")?
    {
        if (bytes.len() + chunk.len()) as u64 > limit {
            return Err("GitHub returned a larger update response than expected.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

async fn latest(client: &Client) -> Result<Option<Release>, String> {
    let response = client
        .get(LATEST)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(12))
        .send()
        .await
        .map_err(|_| "Could not reach GitHub. Check your connection and try again.")?;
    match response.status() {
        StatusCode::NOT_FOUND => Ok(None),
        StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
            Err("GitHub temporarily limited update checks. Try again later.".into())
        }
        StatusCode::OK => serde_json::from_slice(&bounded_body(response, 1024 * 1024).await?)
            .map(Some)
            .map_err(|_| "GitHub returned unreadable release information.".into()),
        _ => Err("GitHub could not return the latest release. Try again later.".into()),
    }
}

fn installed_exe() -> Option<PathBuf> {
    // The standard per-user NSIS installation has a sibling uninstaller.
    // Avoid placing an installer, registry entry or shortcuts in a portable folder.
    let expected = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
        .join("Hush")
        .join("hushline.exe");
    let current = std::env::current_exe().ok()?;
    (expected.canonicalize().ok()? == current.canonicalize().ok()?
        && current.with_file_name("uninstall.exe").is_file())
    .then_some(current)
}

pub async fn check() -> Result<UpdateStatus, String> {
    let release = latest(&client()?).await?;
    let available_version = match &release {
        Some(release) => candidate(release, CURRENT)?.map(|(tag, _)| tag.to_owned()),
        None => None,
    };
    Ok(UpdateStatus {
        installed_version: CURRENT,
        available_version,
        published_release: release.is_some(),
        can_install: installed_exe().is_some(),
    })
}

fn powershell() -> Result<Command, String> {
    let system = std::env::var_os("SystemRoot").ok_or("Windows PowerShell was not found.")?;
    let path = PathBuf::from(system).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
    if !path.is_absolute() || !path.is_file() {
        return Err("Windows PowerShell was not found.".into());
    }
    let mut command = Command::new(path);
    command
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            SCRIPT,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW);
    Ok(command)
}

pub struct PreparedUpdate {
    stage: TempDir,
    exe: PathBuf,
    version: String,
    digest: String,
}

impl PreparedUpdate {
    fn command(&self, mode: &str) -> Result<Command, String> {
        let mut command = powershell()?;
        command
            .env("HUSH_UPDATE_MODE", mode)
            .env("HUSH_UPDATE_STAGE", self.stage.path())
            .env("HUSH_UPDATE_EXE", &self.exe)
            .env("HUSH_UPDATE_VERSION", &self.version)
            .env("HUSH_UPDATE_SHA256", &self.digest)
            .env("HUSH_UPDATE_PARENT", std::process::id().to_string());
        Ok(command)
    }

    pub fn handoff(self) -> Result<(), String> {
        self.command("install")?
            .spawn()
            .map_err(|_| "Windows could not start the Hush updater.")?;
        // The helper owns this directory after it starts. It removes the files
        // after success or rollback, and retains a backup if rollback fails.
        let _ = self.stage.keep();
        Ok(())
    }
}

fn hash_matches(bytes: &[u8], size: u64, digest: &str) -> bool {
    bytes.len() as u64 == size
        && format!("{:x}", Sha256::digest(bytes)).eq_ignore_ascii_case(digest)
}

pub async fn prepare(requested: &str) -> Result<PreparedUpdate, String> {
    let exe = installed_exe().ok_or("Use the Windows installer to update this copy of Hush.")?;
    let client = client()?;
    let release = latest(&client)
        .await?
        .ok_or("There is no published Hush release to install.")?;
    let (tag, asset) = candidate(&release, CURRENT)?
        .filter(|(tag, _)| *tag == requested)
        .ok_or("The available Hush version changed. Check for updates again.")?;
    let digest = asset.digest.as_deref().unwrap_or_default();
    let digest = digest.strip_prefix("sha256:").unwrap_or_default();
    let url = format!("{REPO}/releases/download/v{tag}/{}", asset.name);
    let response = client
        .get(url)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|_| "The Hush installer could not be downloaded. Try again.")?;
    let bytes = bounded_body(response, asset.size).await?;
    if !hash_matches(&bytes, asset.size, digest) {
        return Err("The downloaded installer did not match GitHub's checksum.".into());
    }
    let stage = tempfile::Builder::new()
        .prefix("hush-update-")
        .tempdir()
        .map_err(|_| "Windows could not create a temporary update folder.")?;
    std::fs::write(stage.path().join("setup.exe"), bytes)
        .map_err(|_| "Windows could not save the downloaded installer.")?;
    let update = PreparedUpdate {
        stage,
        exe,
        version: tag.to_owned(),
        digest: digest.to_owned(),
    };
    let mut command = tokio::process::Command::from(update.command("verify")?);
    let status = tokio::time::timeout(Duration::from_secs(25), command.kill_on_drop(true).status())
        .await
        .map_err(|_| "Windows took too long to verify the installer signature.")?
        .map_err(|_| "Windows could not verify the installer signature.")?;
    if !status.success() {
        return Err(
            "Windows rejected the installer's signature or version. Hush was not changed.".into(),
        );
    }
    Ok(update)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str) -> Release {
        Release {
            tag_name: format!("v{tag}"),
            draft: false,
            prerelease: false,
            assets: vec![Asset {
                name: format!("Hush_{tag}_x64-setup.exe"),
                state: "uploaded".into(),
                size: 4000,
                digest: Some(format!("sha256:{}", "ab".repeat(32))),
            }],
        }
    }

    #[test]
    fn offers_only_newer_stable_releases() {
        assert!(candidate(&release("0.7.4"), "0.7.5").unwrap().is_none());
        assert!(candidate(&release("0.7.5"), "0.7.5").unwrap().is_none());
        assert_eq!(
            candidate(&release("0.10.0"), "0.9.9").unwrap().unwrap().0,
            "0.10.0"
        );
        let mut prerelease = release("0.9.0-beta");
        prerelease.prerelease = true;
        assert!(candidate(&prerelease, "0.7.5").unwrap().is_none());
        prerelease.draft = true;
        prerelease.prerelease = false;
        assert!(candidate(&prerelease, "0.7.5").unwrap().is_none());
        assert!(candidate(&release("0.08.0"), "0.7.5").is_err());
    }

    #[test]
    fn rejects_missing_duplicate_or_unverifiable_installers() {
        let mut missing = release("1.0.0");
        missing.assets[0].digest = None;
        assert!(candidate(&missing, "0.7.5").is_err());
        let mut duplicate = release("1.0.0");
        duplicate.assets.push(release("1.0.0").assets.remove(0));
        assert!(candidate(&duplicate, "0.7.5").is_err());
    }

    #[test]
    fn detects_changed_or_partial_downloads() {
        let expected = format!("{:x}", Sha256::digest(b"installer"));
        assert!(hash_matches(b"installer", 9, &expected));
        assert!(!hash_matches(b"Installer", 9, &expected));
        assert!(!hash_matches(b"installer", 10, &expected));
    }
}
