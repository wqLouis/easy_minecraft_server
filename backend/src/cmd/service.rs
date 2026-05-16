//! Install systemd service.
use std::path::PathBuf;

pub async fn install_service(output: &str, _settings_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let cwd = std::env::current_dir()?;
    let cwd_s = cwd.display();
    let unit = format!(
"[Unit]\nDescription=Minecraft Server Backend API\nAfter=network.target\n\n[Service]\nType=simple\nExecStart={} serve\nWorkingDirectory={}\nRestart=on-failure\nRestartSec=5\nEnvironment=RUST_LOG=info\nStandardOutput=append:{cwd_s}/server.log\nStandardError=append:{cwd_s}/server.log\n\n[Install]\nWantedBy=multi-user.target\n",
        exe.display(), cwd_s);
    let path = std::path::Path::new(output);
    if let Some(parent) = path.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(path, &unit)?;
    let sn = path.file_stem().unwrap().to_string_lossy().to_string();
    let user = output.contains("systemd/user/");
    let p = if user { "" } else { "sudo " };
    let f = if user { " --user" } else { "" };
    println!("✅ Systemd{} service written to {}", if user { " user" } else { "" }, path.display());
    println!("\n   Enable/start:\n     {p}systemctl{f} daemon-reload\n     {p}systemctl{f} enable --now {sn}",);
    println!("\n   Logs:\n     {p}journalctl{f} -u {sn} -f\n\n   Stop:\n     {p}systemctl{f} stop {sn}");
    Ok(())
}
