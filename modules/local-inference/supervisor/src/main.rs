//! redrob-local-inference: supervises the llama.cpp router server.
//!
//! The model lives under `models_dir` and is provisioned separately from OTA,
//! so it may be absent at boot and appear later. The loop therefore:
//!   * waits (polling) until a router model is installed,
//!   * launches the llama.cpp OpenAI-compatible server bound to loopback,
//!   * relaunches it with a backoff if it exits,
//!   * on SIGTERM/SIGINT, kills the child and exits cleanly.
//!
//! The agent reaches this server as its `llamacpp.router` provider and uses it
//! as the offline fallback when Redrob Console is unreachable (agent.toml).
use anyhow::{Context, Result};
use redrob_local_inference::{config::Config, model, server};
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn usage() -> ! {
    eprintln!(
        "usage: redrob-local-inference [--config /etc/redrob/local-inference.toml] \
         [--models-dir DIR] [--server-bin PATH] [--listen-host IP] [--listen-port N] [--once]"
    );
    std::process::exit(2)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_ansi(false)
        .init();

    let mut config_path = PathBuf::from("/etc/redrob/local-inference.toml");
    let mut overrides = Overrides::default();
    let mut once = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--config" => config_path = args.next().map(PathBuf::from).unwrap_or_else(|| usage()),
            "--models-dir" => overrides.models_dir = args.next().map(PathBuf::from),
            "--server-bin" => overrides.server_bin = args.next().map(PathBuf::from),
            "--listen-host" => overrides.listen_host = args.next(),
            "--listen-port" => overrides.listen_port = args.next().and_then(|v| v.parse().ok()),
            // --once: launch at most one server generation and return its exit
            // status instead of looping. Used by the L0 harness.
            "--once" => once = true,
            _ => usage(),
        }
    }

    let mut cfg = if config_path.exists() {
        Config::load(&config_path).with_context(|| format!("load {}", config_path.display()))?
    } else {
        tracing::warn!(path = %config_path.display(), "no config file; using defaults");
        Config::default()
    };
    overrides.apply(&mut cfg);
    cfg.validate().context("validate config")?;

    tracing::info!(
        models_dir = %cfg.models_dir.display(),
        bind = %format!("{}:{}", cfg.listen_host, cfg.listen_port),
        alias = %cfg.model_alias,
        "local-inference supervisor up"
    );

    // SIGTERM/SIGINT -> flip the flag; the loop kills any child and exits.
    let stop = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGTERM, signal_hook::consts::SIGINT] {
        signal_hook::flag::register(sig, Arc::clone(&stop))
            .with_context(|| format!("register signal {sig}"))?;
    }

    run_loop(&cfg, &stop, once)
}

#[derive(Default)]
struct Overrides {
    models_dir: Option<PathBuf>,
    server_bin: Option<PathBuf>,
    listen_host: Option<String>,
    listen_port: Option<u16>,
}

impl Overrides {
    fn apply(self, cfg: &mut Config) {
        if let Some(v) = self.models_dir {
            cfg.models_dir = v;
        }
        if let Some(v) = self.server_bin {
            cfg.server_bin = v;
        }
        if let Some(v) = self.listen_host {
            cfg.listen_host = v;
        }
        if let Some(v) = self.listen_port {
            cfg.listen_port = v;
        }
    }
}

fn run_loop(cfg: &Config, stop: &Arc<AtomicBool>, once: bool) -> Result<()> {
    while !stop.load(Ordering::Relaxed) {
        let Some(model_path) = model::select_model(cfg) else {
            if once {
                tracing::error!(dir = %cfg.models_dir.display(), "no router model installed");
                std::process::exit(3);
            }
            tracing::info!(dir = %cfg.models_dir.display(), "no router model yet; waiting");
            interruptible_sleep(stop, cfg.poll_secs);
            continue;
        };

        let argv = server::build_argv(cfg, &model_path);
        tracing::info!(bin = %cfg.server_bin.display(), model = %model_path.display(), "starting router server");
        let mut child = Command::new(&cfg.server_bin)
            .args(&argv)
            .spawn()
            .with_context(|| format!("spawn {}", cfg.server_bin.display()))?;

        let status = wait_or_stop(&mut child, stop);
        if stop.load(Ordering::Relaxed) {
            tracing::info!("stop requested; terminating router server");
            let _ = child.kill();
            let _ = child.wait();
            return Ok(());
        }
        match status {
            Some(s) => tracing::warn!(code = s.code(), "router server exited"),
            None => tracing::warn!("router server wait failed"),
        }
        if once {
            return Ok(());
        }
        interruptible_sleep(stop, cfg.restart_secs);
    }
    Ok(())
}

/// Poll the child while watching the stop flag so SIGTERM is honoured promptly.
fn wait_or_stop(child: &mut Child, stop: &Arc<AtomicBool>) -> Option<std::process::ExitStatus> {
    loop {
        if stop.load(Ordering::Relaxed) {
            return None;
        }
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(e) => {
                tracing::error!(error = %e, "try_wait failed");
                return None;
            }
        }
    }
}

fn interruptible_sleep(stop: &Arc<AtomicBool>, secs: u64) {
    for _ in 0..(secs * 5) {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}
