//! Launching and supervising the Java server process, mirroring what the
//! Electron main process does in `gui/electron/main`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[derive(Debug, Default)]
pub struct ServerProcess {
    child: Option<Child>,
    log_rx: Option<Receiver<String>>,
    log_tx: Option<Sender<String>>,
    pub logs: Vec<String>,
    pub jar: Option<PathBuf>,
    pub java: Option<PathBuf>,
    pub last_error: Option<String>,
}

impl ServerProcess {
    pub fn is_running(&mut self) -> bool {
        match self.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => {
                    self.push_log(format!("サーバーが終了しました ({status})"));
                    self.child = None;
                    false
                }
                Ok(None) => true,
                Err(error) => {
                    self.last_error = Some(error.to_string());
                    false
                }
            },
            None => false,
        }
    }

    /// One-line summary for the server page.
    pub fn status_text(&self) -> String {
        match (&self.child, &self.last_error) {
            (Some(child), _) => format!("PID {}", child.id()),
            (None, Some(error)) => format!("エラー: {error}"),
            (None, None) => "停止中".to_string(),
        }
    }

    pub fn jar_display(&self) -> String {
        self.jar
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未検出".to_string())
    }

    pub fn java_display(&self) -> String {
        self.java
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "未検出".to_string())
    }

    pub fn clear_logs(&mut self) {
        self.logs.clear();
    }

    fn push_log(&mut self, line: String) {
        self.logs.push(line);
        let excess = self.logs.len().saturating_sub(2000);
        if excess > 0 {
            self.logs.drain(..excess);
        }
    }

    /// Refresh the jar/java paths from a user-provided server directory.
    pub fn detect(&mut self, server_dir: Option<&Path>, java_path: Option<&Path>) {
        self.jar = match server_dir {
            Some(dir) => find_jar(Some(dir)),
            None => find_jar(Some(&exe_dir().unwrap_or_default())),
        };
        self.java = java_path
            .filter(|path| path.is_file())
            .map(Path::to_path_buf)
            .or_else(|| self.jar.as_deref().and_then(|jar| find_java(jar.parent())));
    }

    pub fn drain_logs(&mut self) {
        let Some(rx) = &self.log_rx else {
            return;
        };
        let mut lines = Vec::new();
        loop {
            match rx.try_recv() {
                Ok(line) => lines.push(line),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.log_rx = None;
                    break;
                }
            }
        }
        for line in lines {
            self.push_log(line);
        }
    }

    pub fn start(&mut self) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }

        let jar = self.jar.clone().ok_or_else(|| {
            "slimevr.jar が見つかりません。サーバーディレクトリを指定してください。".to_string()
        })?;
        let java = self
            .java
            .clone()
            .ok_or_else(|| "Java 17 以降の実行ファイルが見つかりません。".to_string())?;

        if let Ok(Some(version)) = java_major_version(&java)
            && version < 17
        {
            return Err(format!("Java {version} は古すぎます (17 以降が必要)。"));
        }

        for variable in ["_JAVA_OPTIONS", "JAVA_TOOL_OPTIONS"] {
            if std::env::var_os(variable).is_some() {
                return Err(format!(
                    "{variable} が設定されています。サーバーはこれを拒否するため解除してください。"
                ));
            }
        }

        if !port_is_free(21110) {
            return Err("ポート 21110 が既に使用されています (別のサーバーが動作中？)".to_string());
        }

        let cwd = jar.parent().map(Path::to_path_buf).unwrap_or_default();
        let mut command = Command::new(&java);
        command
            .arg("-Xmx128M")
            .arg("-jar")
            .arg(&jar)
            .arg("run")
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = command
            .spawn()
            .map_err(|error| format!("サーバーを起動できません: {error}"))?;

        let (tx, rx) = mpsc::channel();
        self.log_tx = Some(tx.clone());
        self.log_rx = Some(rx);
        let mut readers: Vec<(&'static str, Box<dyn std::io::Read + Send>)> = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            readers.push(("out", Box::new(stdout)));
        }
        if let Some(stderr) = child.stderr.take() {
            readers.push(("err", Box::new(stderr)));
        }
        for (name, pipe) in readers {
            let tx = tx.clone();
            std::thread::spawn(move || {
                let reader = BufReader::new(pipe);
                for line in reader.lines().map_while(Result::ok) {
                    if tx.send(format!("[{name}] {line}")).is_err() {
                        break;
                    }
                }
            });
        }

        self.push_log(format!(
            "起動: {} -jar {} run",
            java.display(),
            jar.display()
        ));
        self.last_error = None;
        self.child = Some(child);
        Ok(())
    }

    pub fn stop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if let Some(stdin) = child.stdin.as_mut() {
            use std::io::Write;
            let _ = stdin.write_all(b"exit\n");
            let _ = stdin.flush();
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        match child.try_wait() {
            Ok(Some(_)) => {}
            _ => {
                let _ = child.kill();
            }
        }
        let _ = child.wait();
        self.push_log("サーバーを停止しました".to_string());
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.stop();
    }
}

pub fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(Path::to_path_buf))
}

/// Search order mirrors the Electron main process (`paths.ts`).
pub fn find_jar(preferred: Option<&Path>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(dir) = preferred {
        candidates.push(dir.join("slimevr.jar"));
    }
    if let Some(dir) = exe_dir() {
        candidates.push(dir.join("slimevr.jar"));
        candidates.push(dir.join("share/slimevr/slimevr.jar"));
    }
    if let Ok(appdir) = std::env::var("APPDIR") {
        candidates.push(PathBuf::from(appdir).join("usr/share/slimevr/slimevr.jar"));
    }
    candidates.push(PathBuf::from("/app/share/slimevr/slimevr.jar"));
    candidates.push(PathBuf::from("/usr/share/slimevr/slimevr.jar"));
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("slimevr.jar"));
        candidates.push(cwd.join("server/desktop/build/libs/slimevr.jar"));
        candidates.push(cwd.join("server/server/desktop/build/libs/slimevr.jar"));
        for ancestor in cwd.ancestors().skip(1).take(3) {
            candidates.push(ancestor.join("server/server/desktop/build/libs/slimevr.jar"));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

pub fn find_java(jar_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(dir) = jar_dir {
        for name in ["jre/bin/java.exe", "jre/bin/java"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    if let Some(home) = std::env::var_os("JAVA_HOME") {
        for name in ["bin/java.exe", "bin/java"] {
            let candidate = PathBuf::from(&home).join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(entries) = std::fs::read_dir("/usr/lib/jvm") {
            for entry in entries.flatten() {
                let candidate = entry.path().join("bin/java");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
        let mac = PathBuf::from("/Library/Java/JavaVirtualMachines");
        if let Ok(entries) = std::fs::read_dir(mac) {
            for entry in entries.flatten() {
                let candidate = entry.path().join("Contents/Home/bin/java");
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    // Fall back to PATH.
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for name in ["java.exe", "java"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Parse the major version from `java -version` output.
pub fn java_major_version(java: &Path) -> Result<Option<u32>, String> {
    let output = Command::new(java)
        .arg("-version")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| error.to_string())?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let Some(start) = text.find('"') else {
        return Ok(None);
    };
    let rest = &text[start + 1..];
    let Some(end) = rest.find('"') else {
        return Ok(None);
    };
    let version = &rest[..end];
    let mut parts = version.split(['.', '_']);
    let first = parts.next().unwrap_or_default();
    if first == "1" {
        return Ok(parts.next().and_then(|part| part.parse().ok()));
    }
    Ok(first.parse().ok())
}

pub fn port_is_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Format a 48-bit hardware address as `AA:BB:CC:DD:EE:FF`.
pub fn format_mac(addr: u64) -> String {
    let bytes = addr.to_be_bytes();
    let mac = &bytes[2..];
    mac.iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_mac_address() {
        assert_eq!(format_mac(0x001122334455), "00:11:22:33:44:55");
        assert_eq!(format_mac(0x000000000000), "00:00:00:00:00:00");
    }
}
