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
