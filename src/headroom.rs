use std::process::Command;
use anyhow::{Context, Result};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

pub fn check_headroom() -> Result<()> {
    let output = Command::new("headroom")
        .arg("--version")
        .output()
        .context("Headroom não encontrado no PATH. Instale com: npm install -g @context-forge/headroom")?;

    if !output.status.success() {
        anyhow::bail!("Headroom instalado mas não responde corretamente");
    }

    Ok(())
}

pub fn spawn_headroom(proxy_port: u16, headroom_port: u16) -> Result<std::process::Child> {
    let mut child = Command::new("headroom");
    child.args(&[
        "proxy",
        "--port", &headroom_port.to_string(),
        "--openai-api-url", &format!("http://127.0.0.1:{}/v1", proxy_port)
    ]);

    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);

    let child = child.spawn().context("Falha ao iniciar headroom")?;

    eprintln!("[HEADROOM] spawned pid={}", child.id());
    Ok(child)
}

pub fn spawn_headroom_via_cmd(proxy_port: u16, headroom_port: u16) -> Result<std::process::Child> {
    let cmd_args = format!(
        "headroom proxy --port {} --openai-api-url http://127.0.0.1:{}/v1",
        headroom_port, proxy_port
    );

    let mut child = Command::new("cmd");
    child.args(&["/c", &cmd_args]);

    #[cfg(target_os = "windows")]
    child.creation_flags(0x08000000);

    let child = child.spawn().context("Falha ao iniciar headroom via cmd")?;

    eprintln!("[HEADROOM] spawned via cmd pid={}", child.id());
    Ok(child)
}

pub async fn wait_for_headroom(port: u16, timeout_secs: u64) -> Result<()> {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(timeout_secs);

    loop {
        if start.elapsed() > timeout {
            anyhow::bail!("Headroom não respondeu em {}s", timeout_secs);
        }

        match tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port)).await {
            Ok(_) => {
                eprintln!("[HEADROOM] ready on port={} after={}ms", port, start.elapsed().as_millis());
                return Ok(());
            }
            Err(_) => {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
}
