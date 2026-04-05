use anyhow::{bail, Context, Result};
use std::process::{Command, Stdio};

/// Launch an application with LeaveAlternateScreen/EnterAlternateScreen
/// to avoid terminal corruption (same pattern as lazygit).
pub(crate) fn launch_app_with_args(bin: &str, args: &[&str], run_dir: &str) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    crossterm::execute!(std::io::stdout(), crossterm::cursor::MoveTo(0, 0))?;
    let status = Command::new(bin)
        .args(args)
        .current_dir(run_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    match status {
        Ok(_) => Ok(()),
        Err(e) => bail!("launch failed: {e}"),
    }
}

pub(crate) fn spawn_app_with_args(bin: &str, args: &[&str], run_dir: &str) -> Result<()> {
    Command::new(bin)
        .args(args)
        .current_dir(run_dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("launch failed")?;
    Ok(())
}

pub(crate) fn launch_lazygit(base_dir: &str, repo_name: &str) -> Result<()> {
    let repo_path = format!("{}/{}", base_dir.trim_end_matches(['/', '\\']), repo_name);
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
    )?;
    let status = Command::new("lazygit").args(["-p", &repo_path]).status();
    let _ = crossterm::terminal::enable_raw_mode();
    let _ = crossterm::execute!(
        std::io::stdout(),
        crossterm::terminal::EnterAlternateScreen,
        crossterm::event::EnableMouseCapture,
    );
    match status {
        Ok(_) => Ok(()),
        Err(e) => bail!("lazygit failed: {e}"),
    }
}

pub(crate) fn open_url(url: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .context("Failed to open browser")?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .context("Failed to open browser")?;
    }
    Ok(())
}
