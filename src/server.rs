//! Launching and supervising the Java server process, mirroring what the
//! Electron main process does in `gui/electron/main`.
//!
//! The JVM is owned by a supervisor thread: the UI only enqueues requests and
//! drains events, so starting, stopping and the (blocking) `java -version`
//! probe never stall the render loop.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use crate::ui::log::LogEntry;

/// Where the supervisor currently is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Stopped,
    Starting,
    Running(u32),
    Stopping,
}

enum Request {
    Start { java: PathBuf, jar: PathBuf },
    Stop,
    Shutdown,
}

enum Event {
    State(State),
    Log(String),
    Error(String),
}

pub struct ServerProcess {
    request_tx: Option<Sender<Request>>,
    event_rx: Option<Receiver<Event>>,
    worker: Option<std::thread::JoinHandle<()>>,
    state: State,
    pub logs: Vec<LogEntry>,
    pub jar: Option<PathBuf>,
    pub java: Option<PathBuf>,
    pub last_error: Option<String>,
}

impl Default for ServerProcess {
    fn default() -> Self {
        Self {
            request_tx: None,
            event_rx: None,
            worker: None,
            state: State::Stopped,
            logs: Vec::new(),
            jar: None,
            java: None,
            last_error: None,
        }
    }
}

impl ServerProcess {
    pub fn state(&self) -> State {
        self.state
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, State::Running(_))
    }

    /// True while a start/stop request is still being carried out.
    pub fn is_busy(&self) -> bool {
        matches!(self.state, State::Starting | State::Stopping)
    }

    /// One-line summary for the server page.
    pub fn status_text(&self) -> String {
        match (self.state, &self.last_error) {
            (State::Running(pid), _) => format!("PID {pid}"),
            (State::Starting, _) => "起動中…".to_string(),
            (State::Stopping, _) => "停止処理中…".to_string(),
            (State::Stopped, Some(error)) => format!("エラー: {error}"),
            // The badge already says 停止中; avoid printing it twice.
            (State::Stopped, None) => String::new(),
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
        self.logs.push(LogEntry::parse(line));
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
        log::info!(
            "サーバー検出: jar={} java={}",
            self.jar_display(),
            self.java_display()
        );
    }

    /// Drain supervisor events. Call once per frame.
    pub fn poll(&mut self) {
        let Some(rx) = &self.event_rx else {
            return;
        };
        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        for event in events {
            match event {
                Event::State(state) => self.state = state,
                Event::Log(line) => self.push_log(line),
                Event::Error(error) => {
                    self.push_log(format!("エラー: {error}"));
                    self.last_error = Some(error);
                }
            }
        }
    }

    fn ensure_worker(&mut self) {
        if self.worker.is_some() {
            return;
        }
        let (request_tx, request_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        self.request_tx = Some(request_tx);
        self.event_rx = Some(event_rx);
        self.worker = Some(std::thread::spawn(move || {
            supervise(request_rx, event_tx);
        }));
    }

    /// Queue a start. Returns immediately; failures past the missing-file
    /// check arrive as `last_error` events.
    pub fn start(&mut self) -> Result<(), String> {
        if self.is_running() || self.is_busy() {
            return Ok(());
        }
        let jar = self.jar.clone().ok_or_else(|| {
            "slimevr.jar が見つかりません。サーバーディレクトリを指定してください。".to_string()
        })?;
        let java = self
            .java
            .clone()
            .ok_or_else(|| "Java 17 以降の実行ファイルが見つかりません。".to_string())?;

        self.ensure_worker();
        self.last_error = None;
        self.state = State::Starting;
        if let Some(tx) = &self.request_tx {
            let _ = tx.send(Request::Start { java, jar });
        }
        Ok(())
    }

    /// Ask the supervisor to stop. Returns immediately.
    pub fn stop(&mut self) {
        if self.request_tx.is_some() && matches!(self.state, State::Running(_) | State::Starting) {
            self.state = State::Stopping;
            if let Some(tx) = &self.request_tx {
                let _ = tx.send(Request::Stop);
            }
        }
    }

    /// Stop the server and join the supervisor. Used on shutdown and by the
    /// headless `--start-server` check.
    pub fn shutdown(&mut self) {
        if let Some(tx) = &self.request_tx {
            let _ = tx.send(Request::Shutdown);
        }
        self.request_tx = None;
        self.event_rx = None;
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.state = State::Stopped;
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn supervise(requests: Receiver<Request>, events: Sender<Event>) {
    let mut child: Option<Child> = None;
    loop {
        match requests.recv_timeout(Duration::from_millis(100)) {
            Ok(Request::Start { java, jar }) => {
                if child.is_some() {
                    continue;
                }
                match spawn_server(&java, &jar, &events) {
                    Ok(new_child) => {
                        let pid = new_child.id();
                        child = Some(new_child);
                        let _ = events.send(Event::State(State::Running(pid)));
                    }
                    Err(error) => {
                        let _ = events.send(Event::Error(error));
                        let _ = events.send(Event::State(State::Stopped));
                    }
                }
            }
            Ok(Request::Stop) => {
                if let Some(mut running) = child.take() {
                    stop_child(&mut running);
                    let _ = events.send(Event::Log("サーバーを停止しました".to_string()));
                    let _ = events.send(Event::State(State::Stopped));
                }
            }
            Ok(Request::Shutdown) => {
                if let Some(mut running) = child.take() {
                    stop_child(&mut running);
                }
                return;
            }
            Err(RecvTimeoutError::Timeout) => {
                let Some(running) = child.as_mut() else {
                    continue;
                };
                match running.try_wait() {
                    Ok(Some(status)) => {
                        child = None;
                        let _ =
                            events.send(Event::Log(format!("サーバーが終了しました ({status})")));
                        let _ = events.send(Event::State(State::Stopped));
                    }
                    Ok(None) => {}
                    Err(error) => {
                        child = None;
                        let _ = events.send(Event::Error(error.to_string()));
                        let _ = events.send(Event::State(State::Stopped));
                    }
                }
            }
            Err(RecvTimeoutError::Disconnected) => {
                if let Some(mut running) = child.take() {
                    stop_child(&mut running);
                }
                return;
            }
        }
    }
}

fn spawn_server(java: &Path, jar: &Path, events: &Sender<Event>) -> Result<Child, String> {
    if let Ok(Some(version)) = java_major_version(java)
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
    let mut command = Command::new(java);
    command
        .arg("-Xmx128M")
        .arg("-jar")
        .arg(jar)
        .arg("run")
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);

    let mut child = command
        .spawn()
        .map_err(|error| format!("サーバーを起動できません: {error}"))?;

    let mut readers: Vec<(&'static str, Box<dyn std::io::Read + Send>)> = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(("out", Box::new(stdout)));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(("err", Box::new(stderr)));
    }
    for (name, pipe) in readers {
        let tx = events.clone();
        std::thread::spawn(move || {
            let reader = BufReader::new(pipe);
            for line in reader.lines().map_while(Result::ok) {
                if tx.send(Event::Log(format!("[{name}] {line}"))).is_err() {
                    break;
                }
            }
        });
    }

    let _ = events.send(Event::Log(format!(
        "起動: {} -jar {} run",
        java.display(),
        jar.display()
    )));
    Ok(child)
}

fn stop_child(child: &mut Child) {
    if let Some(stdin) = child.stdin.as_mut() {
        let _ = stdin.write_all(b"exit\n");
        let _ = stdin.flush();
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => break,
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// The GUI has no console of its own on Windows, so a console-subsystem JVM
/// would flash (or keep) a terminal window. CREATE_NO_WINDOW suppresses it.
#[cfg(windows)]
fn hide_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn hide_console_window(_command: &mut Command) {}

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
    // A jar downloaded from the settings page wins over the bundled copy:
    // clicking "download" is an explicit request to use that build.
    candidates.push(crate::download::installed_jar());
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

    for name in ["bin/java.exe", "bin/java"] {
        let candidate = crate::download::installed_java().join(name);
        if candidate.is_file() {
            return Some(candidate);
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
    let mut command = Command::new(java);
    command
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    let output = command.output().map_err(|error| error.to_string())?;
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

/// Cached liveness of the server port. The server page used to bind the port
/// on every repaint, which is a syscall per frame for a value that changes at
/// human speed; this refreshes at most once a second.
pub struct PortProbe {
    in_use: bool,
    checked: Instant,
}

impl Default for PortProbe {
    fn default() -> Self {
        Self {
            in_use: !port_is_free(21110),
            checked: Instant::now(),
        }
    }
}

impl PortProbe {
    pub fn check(&mut self) {
        self.in_use = !port_is_free(21110);
        self.checked = Instant::now();
    }

    pub fn refresh(&mut self) {
        if self.checked.elapsed() >= Duration::from_secs(1) {
            self.in_use = !port_is_free(21110);
            self.checked = Instant::now();
        }
    }

    pub fn in_use(&self) -> bool {
        self.in_use
    }
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

    /// The probe must follow a listener appearing and disappearing; a refresh
    /// within the throttle window keeps the cached value.
    #[test]
    fn port_probe_tracks_listener() {
        let Ok(listener) = std::net::TcpListener::bind(("127.0.0.1", 21110)) else {
            // Something else owns the port on this machine; nothing to assert.
            return;
        };
        let mut probe = PortProbe::default();
        assert!(probe.in_use(), "bound port must read as in use");
        probe.refresh();
        assert!(probe.in_use(), "throttled refresh keeps the cached value");
        drop(listener);
        probe.check();
        assert!(!probe.in_use(), "released port must read as free");
    }
}
