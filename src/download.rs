//! Downloading the pieces the app can run for the user: a Java runtime from
//! Adoptium and `slimevr.jar` from the SlimeVR Server GitHub releases.
//!
//! Downloads run on a worker thread and report progress through a channel so
//! the UI can stay responsive.

use std::fs::{self, File};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

/// Where downloaded pieces live: `<data dir>/slimevr-native-ui/`.
pub fn install_root() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("slimevr-native-ui")
}

pub fn installed_jar() -> PathBuf {
    install_root().join("slimevr.jar")
}

pub fn installed_java() -> PathBuf {
    install_root().join("jre")
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    /// A JRE 17 for the current platform, unpacked into `jre/`.
    Java,
    /// `slimevr.jar` from the newest SlimeVR Server release.
    Server,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Java => "Java 17 (JRE)",
            Kind::Server => "SlimeVR Server (slimevr.jar)",
        }
    }
}

/// Adoptium's "latest JRE 17 for this platform" endpoint.
fn java_url() -> String {
    let os = match std::env::consts::OS {
        "windows" => "windows",
        "macos" => "mac",
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "aarch64",
        other => other,
    };
    format!(
        "https://api.adoptium.net/v3/binary/latest/17/ga/{os}/{arch}/jre/hotspot/normal/eclipse?project=jdk"
    )
}

/// GitHub redirects `latest/download/<asset>` to the newest release asset.
fn server_url() -> &'static str {
    "https://github.com/SlimeVR/SlimeVR-Server/releases/latest/download/slimevr.jar"
}

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    /// Nothing started yet.
    Idle,
    Downloading {
        received: u64,
        total: Option<u64>,
    },
    Unpacking,
    Done(PathBuf),
    Failed(String),
}

impl Phase {
    pub fn is_busy(&self) -> bool {
        matches!(self, Phase::Downloading { .. } | Phase::Unpacking)
    }

    pub fn progress_fraction(&self) -> Option<f32> {
        match self {
            Phase::Downloading {
                received,
                total: Some(total),
            } if *total > 0 => Some(*received as f32 / *total as f32),
            _ => None,
        }
    }
}

enum Message {
    Progress { received: u64, total: Option<u64> },
    Unpacking,
    Done(Result<PathBuf, String>),
}

/// One download in flight. Dropping it leaves the worker thread to finish
/// harmlessly; the file simply is not reported.
pub struct Job {
    pub phase: Phase,
    rx: Option<Receiver<Message>>,
}

impl Job {
    pub fn is_running(&self) -> bool {
        self.rx.is_some()
    }

    pub fn start(kind: Kind) -> Job {
        let (tx, rx) = mpsc::channel();
        let handle = std::thread::Builder::new()
            .name(format!("download-{:?}", kind))
            .spawn(move || {
                let result = match kind {
                    Kind::Java => download_java(&tx),
                    Kind::Server => download_server(&tx),
                };
                let _ = tx.send(Message::Done(result));
            });

        match handle {
            Ok(_) => Job {
                phase: Phase::Idle,
                rx: Some(rx),
            },
            Err(error) => Job {
                phase: Phase::Failed(format!("スレッドを開始できません: {error}")),
                rx: None,
            },
        }
    }

    /// Apply whatever the worker reported since the last frame.
    pub fn poll(&mut self) {
        let Some(rx) = &self.rx else {
            return;
        };
        loop {
            match rx.try_recv() {
                Ok(Message::Progress { received, total }) => {
                    self.phase = Phase::Downloading { received, total };
                }
                Ok(Message::Unpacking) => self.phase = Phase::Unpacking,
                Ok(Message::Done(result)) => {
                    self.phase = match result {
                        Ok(path) => Phase::Done(path),
                        Err(error) => Phase::Failed(error),
                    };
                    self.rx = None;
                    break;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !matches!(self.phase, Phase::Done(_) | Phase::Failed(_)) {
                        self.phase = Phase::Failed("ダウンロードが中断されました".to_string());
                    }
                    self.rx = None;
                    break;
                }
            }
        }
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .user_agent(concat!("slimevr-native-ui/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn download(url: &str, destination: &Path, tx: &Sender<Message>) -> Result<u64, String> {
    let response = agent()
        .get(url)
        .call()
        .map_err(|error| format!("取得に失敗しました: {error}"))?;
    let total = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok());

    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| format!("フォルダを作成できません: {error}"))?;
    }
    let mut file = File::create(destination).map_err(|error| format!("保存できません: {error}"))?;
    let mut reader = BufReader::new(response.into_reader());
    let mut buffer = vec![0u8; 256 * 1024];
    let mut received = 0u64;
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("受信中にエラー: {error}"))?;
        if read == 0 {
            break;
        }
        file.write_all(&buffer[..read])
            .map_err(|error| format!("書き込みに失敗しました: {error}"))?;
        received += read as u64;
        let _ = tx.send(Message::Progress { received, total });
    }
    file.flush()
        .map_err(|error| format!("書き込みに失敗しました: {error}"))?;
    Ok(received)
}

fn download_server(tx: &Sender<Message>) -> Result<PathBuf, String> {
    let root = install_root();
    fs::create_dir_all(&root).map_err(|error| format!("フォルダを作成できません: {error}"))?;
    let temporary = root.join("slimevr.jar.part");
    let target = installed_jar();
    download(server_url(), &temporary, tx)?;
    if temporary.metadata().map(|meta| meta.len()).unwrap_or(0) < 1_000_000 {
        let _ = fs::remove_file(&temporary);
        return Err("slimevr.jar のサイズが不正です (配布元が変わった可能性)".to_string());
    }
    fs::rename(&temporary, &target).map_err(|error| format!("配置できません: {error}"))?;
    Ok(target)
}

fn download_java(tx: &Sender<Message>) -> Result<PathBuf, String> {
    let root = install_root();
    fs::create_dir_all(&root).map_err(|error| format!("フォルダを作成できません: {error}"))?;
    let archive = if cfg!(target_os = "windows") {
        root.join("jre.zip")
    } else {
        root.join("jre.tar.gz")
    };
    download(&java_url(), &archive, tx)?;

    let _ = tx.send(Message::Unpacking);
    let staging = root.join("jre-staging");
    let _ = fs::remove_dir_all(&staging);
    let unpacked = if archive.extension().and_then(|ext| ext.to_str()) == Some("zip") {
        unpack_zip(&archive, &staging)
    } else {
        unpack_tar_gz(&archive, &staging)
    };
    unpacked?;
    let _ = fs::remove_file(&archive);

    // Adoptium archives contain a single top-level folder (`jdk-17...-jre`).
    let inner = single_directory(&staging).unwrap_or_else(|| staging.clone());
    let target = installed_java();
    let _ = fs::remove_dir_all(&target);
    fs::rename(&inner, &target).map_err(|error| format!("配置できません: {error}"))?;
    let _ = fs::remove_dir_all(&staging);
    Ok(target)
}

fn single_directory(root: &Path) -> Option<PathBuf> {
    let mut entries = fs::read_dir(root).ok()?.flatten();
    let first = entries.next()?;
    if entries.next().is_some() || !first.path().is_dir() {
        return None;
    }
    Some(first.path())
}

fn unpack_zip(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| format!("開けません: {error}"))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|error| format!("zip を読めません: {error}"))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| format!("zip を読めません: {error}"))?;
        let Some(path) = entry.enclosed_name() else {
            continue;
        };
        let path = destination.join(path);
        if entry.is_dir() {
            fs::create_dir_all(&path).map_err(|error| format!("作成できません: {error}"))?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| format!("作成できません: {error}"))?;
        }
        let mut out = File::create(&path).map_err(|error| format!("作成できません: {error}"))?;
        std::io::copy(&mut entry, &mut out).map_err(|error| format!("展開できません: {error}"))?;

        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&path, fs::Permissions::from_mode(mode));
        }
    }
    Ok(())
}

fn unpack_tar_gz(archive: &Path, destination: &Path) -> Result<(), String> {
    let file = File::open(archive).map_err(|error| format!("開けません: {error}"))?;
    let decoder = flate2::read::GzDecoder::new(BufReader::new(file));
    let mut tar = tar::Archive::new(decoder);
    tar.unpack(destination)
        .map_err(|error| format!("展開できません: {error}"))
}

impl Job {
    /// A placeholder used before anything has been started, so the UI can ask
    /// for a job slot without special-casing "nothing yet".
    pub fn idle() -> Job {
        Job {
            phase: Phase::Idle,
            rx: None,
        }
    }
}

/// The two downloads the server page exposes.
#[derive(Default)]
pub struct Downloads {
    java: Option<Job>,
    server: Option<Job>,
}

impl Downloads {
    fn slot(&mut self, kind: Kind) -> &mut Option<Job> {
        match kind {
            Kind::Java => &mut self.java,
            Kind::Server => &mut self.server,
        }
    }

    pub fn job_mut(&mut self, kind: Kind) -> &mut Job {
        self.slot(kind).get_or_insert_with(Job::idle)
    }

    pub fn start(&mut self, kind: Kind) {
        *self.slot(kind) = Some(Job::start(kind));
    }

    pub fn any_running(&self) -> bool {
        [&self.java, &self.server]
            .into_iter()
            .flatten()
            .any(Job::is_running)
    }

    /// Advance both jobs and return the kinds that finished this frame.
    pub fn poll(&mut self) -> Vec<(Kind, Phase)> {
        let mut finished = Vec::new();
        for kind in [Kind::Java, Kind::Server] {
            let Some(job) = self.slot(kind).as_mut() else {
                continue;
            };
            let before = job.phase.clone();
            job.poll();
            if before != job.phase
                && matches!(job.phase, Phase::Done(_) | Phase::Failed(_))
                && !matches!(before, Phase::Done(_) | Phase::Failed(_))
            {
                finished.push((kind, job.phase.clone()));
            }
        }
        finished
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_fraction_needs_a_total() {
        assert_eq!(
            Phase::Downloading {
                received: 50,
                total: Some(100)
            }
            .progress_fraction(),
            Some(0.5)
        );
        assert_eq!(
            Phase::Downloading {
                received: 50,
                total: None
            }
            .progress_fraction(),
            None
        );
        assert_eq!(Phase::Unpacking.progress_fraction(), None);
    }

    #[test]
    fn single_directory_only_matches_one_folder() {
        let root = std::env::temp_dir().join(format!("slimevr-dl-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);

        assert_eq!(single_directory(&root), None);

        fs::create_dir_all(root.join("jdk-17-jre")).unwrap();
        assert_eq!(single_directory(&root), Some(root.join("jdk-17-jre")));

        fs::write(root.join("extra.txt"), b"x").unwrap();
        assert_eq!(single_directory(&root), None);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn idle_jobs_are_not_running() {
        let mut downloads = Downloads::default();
        assert!(!downloads.any_running());
        assert_eq!(downloads.job_mut(Kind::Java).phase, Phase::Idle);
        assert_eq!(downloads.poll(), Vec::new());
    }
}
