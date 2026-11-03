//! Shared settings controller. CLI and GTK dialog reuse core validation/storage.
use crate::settings::Settings;
use std::{io, path::Path, process::Command};

#[derive(Debug, Default, serde::Serialize)]
pub struct Notification {
    pub notified: usize,
    pub errors: Vec<String>,
}
pub fn notify_directory(directory: &Path) -> Notification {
    #[cfg(not(unix))]
    {
        let _ = directory;
        Notification::default()
    }
    #[cfg(unix)]
    {
        use std::{
            io::{BufRead, Read, Write},
            os::unix::{fs::FileTypeExt, net::UnixStream},
            time::Duration,
        };
        let mut report = Notification::default();
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return report,
            Err(e) => {
                report.errors.push(e.to_string());
                return report;
            }
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("core-")
                || !name.ends_with(".sock")
                || !entry.file_type().is_ok_and(|t| t.is_socket())
            {
                continue;
            }
            let result = (|| -> io::Result<()> {
                let mut stream = UnixStream::connect(&path)?;
                stream.set_read_timeout(Some(Duration::from_millis(500)))?;
                stream.set_write_timeout(Some(Duration::from_millis(500)))?;
                stream.write_all(b"{\"command\":\"reload\"}\n")?;
                let mut text = String::new();
                std::io::BufReader::new((&stream).take(1024 * 1024)).read_line(&mut text)?;
                let value: serde_json::Value =
                    serde_json::from_str(&text).map_err(io::Error::other)?;
                if value["ok"] != true {
                    return Err(io::Error::other(value["error"].to_string()));
                }
                Ok(())
            })();
            match result {
                Ok(()) => report.notified += 1,
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound
                    ) => {}
                Err(e) => report.errors.push(format!("{}: {e}", path.display())),
            }
        }
        report
    }
}
pub fn save_and_notify(settings: &Settings, path: &Path) -> io::Result<Notification> {
    settings.save(path)?;
    Ok(notify_directory(&Settings::runtime_directory(path)))
}
pub fn run(arguments: &[String]) -> io::Result<()> {
    let path = Settings::config_path()?;
    if arguments.is_empty() || arguments == ["--gui"] {
        return gui(&path);
    }
    if arguments == ["--help"] || arguments == ["-h"] {
        println!("viet-config [--gui | --status | --set-method Telex|VNI | --enable | --disable | --reload]");
        return Ok(());
    }
    let mut settings = Settings::load_editable(&path)?;
    if arguments == ["--status"] {
        println!(
            "{}",
            serde_json::to_string_pretty(&settings).map_err(io::Error::other)?
        );
        return Ok(());
    }
    let notification = match arguments {
        [flag, value] if flag == "--set-method" => {
            settings.input_method = value.clone();
            save_and_notify(&settings, &path)?
        }
        [flag] if flag == "--enable" || flag == "--disable" => {
            settings.enabled = flag == "--enable";
            save_and_notify(&settings, &path)?
        }
        [flag] if flag == "--reload" => notify_directory(&Settings::runtime_directory(&path)),
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Unknown arguments; use viet-config --help",
            ))
        }
    };
    println!(
        "{}",
        serde_json::to_string(&notification).map_err(io::Error::other)?
    );
    if !notification.errors.is_empty() {
        return Err(io::Error::other(
            "Settings saved; some running cores could not reload",
        ));
    }
    Ok(())
}
fn gui(path: &Path) -> io::Result<()> {
    let mut settings = Settings::load_editable(path)?;
    let text=format!("Đang dùng: {} · {}\nĐể trống lựa chọn để giữ giá trị hiện tại.\nCấu hình dùng chung cho IBus và Fcitx5.",settings.input_method,if settings.enabled{"Tiếng Việt"}else{"English"});
    let dialog = Command::new("zenity")
        .args([
            "--forms",
            "--title=Cài đặt Viet IME",
            "--width=440",
            "--separator=\t",
            "--text",
            &text,
            "--add-combo=Kiểu gõ",
            "--combo-values=Telex|Telex 2|VNI",
            "--add-combo=Chế độ",
            "--combo-values=Tiếng Việt|English",
            "--add-combo=Kiểu dấu",
            "--combo-values=hòa, ủy|hoà, uỷ",
        ])
        .output()
        .map_err(|e| {
            io::Error::new(
                e.kind(),
                format!("Cannot open GTK dialog: install zenity, or use viet-config --help. {e}"),
            )
        })?;
    if dialog.status.code() == Some(1) {
        return Ok(());
    }
    if !dialog.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&dialog.stderr).to_string(),
        ));
    }
    let response = String::from_utf8(dialog.stdout).map_err(io::Error::other)?;
    let fields: Vec<_> = response
        .trim_end_matches(['\r', '\n'])
        .split('\t')
        .collect();
    if let Some(method) = fields.first().filter(|s| !s.is_empty()) {
        settings.input_method = method.to_string();
    }
    match fields.get(1).copied() {
        Some("Tiếng Việt") => settings.enabled = true,
        Some("English") => settings.enabled = false,
        _ => {}
    }
    match fields.get(2).copied() {
        Some("hòa, ủy") => settings.flags |= 2,
        Some("hoà, uỷ") => settings.flags &= !2,
        _ => {}
    }
    let report = save_and_notify(&settings, path)?;
    let message = format!(
        "Đã lưu cấu hình. Đã báo {} core đang chạy.{}",
        report.notified,
        if report.errors.is_empty() {
            String::new()
        } else {
            format!("\n{}", report.errors.join("\n"))
        }
    );
    Command::new("zenity")
        .args(["--info", "--title=Viet IME", "--text", &message])
        .status()?;
    Ok(())
}
