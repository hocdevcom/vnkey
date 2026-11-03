//! One core-owned listener per hosting process; shared settings, isolated typing state.
use crate::settings::Settings;
use serde::Deserialize;
use std::{
    io,
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock, RwLock, Weak},
};

#[derive(Clone, Debug)]
pub struct Snapshot {
    pub revision: u64,
    pub settings: Settings,
}
pub struct Runtime {
    shared: Arc<RwLock<Snapshot>>,
    config_path: PathBuf,
    #[cfg(unix)]
    server: Option<Server>,
}
impl Runtime {
    /// Runtime without a listener, useful for embedders and deterministic tests.
    pub fn isolated(settings: Settings, config_path: PathBuf) -> io::Result<Arc<Self>> {
        settings.validate()?;
        Ok(Arc::new(Self {
            shared: Arc::new(RwLock::new(Snapshot {
                revision: 0,
                settings,
            })),
            config_path,
            #[cfg(unix)]
            server: None,
        }))
    }
    pub fn shared() -> io::Result<Arc<Self>> {
        static INSTANCE: OnceLock<Mutex<Weak<Runtime>>> = OnceLock::new();
        let mut slot = INSTANCE
            .get_or_init(|| Mutex::new(Weak::new()))
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(runtime) = slot.upgrade() {
            return Ok(runtime);
        }
        let config_path = Settings::config_path()?;
        let settings = Settings::load(&config_path)?;
        let mut runtime = Self {
            shared: Arc::new(RwLock::new(Snapshot {
                revision: 0,
                settings,
            })),
            config_path,
            #[cfg(unix)]
            server: None,
        };
        runtime.start_server()?;
        let runtime = Arc::new(runtime);
        *slot = Arc::downgrade(&runtime);
        Ok(runtime)
    }
    pub fn snapshot(&self) -> Snapshot {
        self.shared
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn command(&self, json: &str) -> String {
        command(&self.shared, &self.config_path, json)
    }
    #[cfg(not(unix))]
    fn start_server(&mut self) -> io::Result<()> {
        Ok(())
    }
    #[cfg(unix)]
    fn start_server(&mut self) -> io::Result<()> {
        let base = if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR").filter(|s| !s.is_empty())
        {
            PathBuf::from(dir)
        } else {
            self.config_path
                .parent()
                .ok_or_else(|| io::Error::other("missing config directory"))?
                .join("run")
        };
        self.server = Some(Server::start(
            base.join("viet-ime"),
            self.shared.clone(),
            self.config_path.clone(),
        )?);
        Ok(())
    }
    #[cfg(unix)]
    pub fn start_at(
        settings: Settings,
        config_path: PathBuf,
        directory: PathBuf,
    ) -> io::Result<Arc<Self>> {
        settings.validate()?;
        let shared = Arc::new(RwLock::new(Snapshot {
            revision: 0,
            settings,
        }));
        let server = Server::start(directory, shared.clone(), config_path.clone())?;
        Ok(Arc::new(Self {
            shared,
            config_path,
            server: Some(server),
        }))
    }
    #[cfg(unix)]
    pub fn socket_path(&self) -> Option<&std::path::Path> {
        self.server.as_ref().map(|s| s.path.as_path())
    }
}
#[derive(Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum Command {
    Status {},
    SetInputMethod { value: String },
    SetEnabled { value: bool },
    Toggle {},
    Reload {},
}
fn command(shared: &RwLock<Snapshot>, config_path: &std::path::Path, json: &str) -> String {
    let parsed: Result<Command, _> = serde_json::from_str(json);
    let cmd = match parsed {
        Ok(cmd) => cmd,
        Err(error) => return serde_json::json!({"ok":false,"error":error.to_string()}).to_string(),
    };
    // Serialize commands so concurrent updates cannot overwrite each other.
    let mut state = shared.write().unwrap_or_else(|e| e.into_inner());
    let mut settings = state.settings.clone();
    let changed = !matches!(cmd, Command::Status { .. });
    match cmd {
        Command::Status {} => {}
        Command::SetInputMethod { value } => settings.input_method = value,
        Command::SetEnabled { value } => settings.enabled = value,
        Command::Toggle {} => settings.enabled = !settings.enabled,
        Command::Reload {} => match Settings::load(config_path) {
            Ok(new) => settings = new,
            Err(e) => return serde_json::json!({"ok":false,"error":e.to_string()}).to_string(),
        },
    }
    if let Err(error) = settings.validate() {
        return serde_json::json!({"ok":false,"error":error.to_string()}).to_string();
    }
    if changed {
        state.settings = settings;
        state.revision = state.revision.wrapping_add(1);
    }
    serde_json::json!({"ok":true,"revision":state.revision,"settings":state.settings}).to_string()
}

#[cfg(unix)]
struct Server {
    path: PathBuf,
    stop: Arc<std::sync::atomic::AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}
#[cfg(unix)]
impl Server {
    fn start(
        directory: PathBuf,
        shared: Arc<RwLock<Snapshot>>,
        config: PathBuf,
    ) -> io::Result<Self> {
        use std::{
            io::{Read, Write},
            os::unix::{
                fs::{DirBuilderExt, MetadataExt, PermissionsExt},
                net::UnixListener,
            },
            sync::atomic::{AtomicBool, Ordering},
            time::Duration,
        };
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true).mode(0o700);
        builder.create(&directory)?;
        let meta = std::fs::symlink_metadata(&directory)?;
        // Never chmod or use a directory controlled by another account or a symlink.
        if !meta.is_dir()
            || meta.uid() != unsafe { libc::geteuid() }
            || meta.permissions().mode() & 0o077 != 0
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "IPC directory must be owned by this user with mode 0700",
            ));
        }
        let path = directory.join(format!("core-{}.sock", std::process::id()));
        let listener = UnixListener::bind(&path)?;
        let setup = (|| {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            listener.set_nonblocking(true)
        })();
        if let Err(error) = setup {
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let worker = match std::thread::Builder::new().name("viet-ime-ipc".into()).spawn(move || {
            while !thread_stop.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((mut stream,_)) => {
                        let _ = stream.set_write_timeout(Some(Duration::from_millis(100)));
                        let deadline = std::time::Instant::now() + Duration::from_millis(100);
                        let mut request = Vec::new();
                        while request.len() <= 4096 && !request.contains(&b'\n') && !thread_stop.load(Ordering::Acquire) {
                            let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else { break; };
                            if remaining.is_zero() { break; }
                            let _ = stream.set_read_timeout(Some(remaining));
                            let mut buffer = [0u8; 512];
                            match stream.read(&mut buffer) {
                                Ok(0) => break,
                                Ok(count) => request.extend_from_slice(&buffer[..count]),
                                Err(_) => break,
                            }
                        }
                        let line = std::str::from_utf8(&request).ok();
                        let reply = if let Some(line) = line.filter(|line| line.len() <= 4096 && line.ends_with('\n')) { command(&shared, &config, line.trim()) }
                            else { "{\"ok\":false,\"error\":\"request must be UTF-8 JSON followed by newline, max 4096 bytes\"}".into() };
                        let _ = writeln!(stream,"{reply}");
                    },
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(10)),
                    Err(_) => break,
                }
            }
        }) { Ok(worker) => worker, Err(error) => { let _ = std::fs::remove_file(&path); return Err(error); } };
        Ok(Self {
            path,
            stop,
            worker: Some(worker),
        })
    }
}
#[cfg(unix)]
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}
