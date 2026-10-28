#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod ui;

use std::env;
use std::net::ToSocketAddrs;
use std::process::ExitCode;
use std::time::Duration;

use ovpilot::{ModelScanner, SettingsStore, UpdateChecker, UpdateStatus, XPlaneProbe};

fn main() -> ExitCode {
    if let Err(error) = execute() {
        eprintln!("error: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn execute() -> ovpilot::Result<()> {
    let mut args = env::args_os().skip(1);
    let command = args.next().unwrap_or_else(|| "run".into());
    match command.to_string_lossy().as_ref() {
        "run" => ui::run(),
        "about" => {
            println!("ovPilot {}", ovpilot::VERSION);
            println!("Repository: {}", ovpilot::GITHUB_REPOSITORY);
            Ok(())
        }
        "scan" => {
            let roots = args.map(Into::into).collect::<Vec<_>>();
            let roots = if roots.is_empty() { SettingsStore::discover()?.load()?.model_roots } else { roots };
            let result = ModelScanner.scan(&roots);
            println!("Scan complete. {} aircraft models found.", result.models.len());
            for model in result.models {
                println!("{}: {}", model.name, model.path.display());
            }
            for path in result.unreadable_directories {
                eprintln!("Could not read: {}", path.display());
            }
            Ok(())
        }
        "update" => {
            if args.next().as_deref() != Some(std::ffi::OsStr::new("check")) {
                return Err("usage: ovPilot update check".into());
            }
            match UpdateChecker::new().check()? {
                UpdateStatus::NoRelease => println!("No published stable GitHub Release yet"),
                UpdateStatus::Current { latest, url } => println!("Current: {} (latest {latest})\n{url}", ovpilot::VERSION),
                UpdateStatus::Available { latest, url } => println!("Update available: {} -> {latest}\n{url}", ovpilot::VERSION),
            }
            Ok(())
        }
        "xplane" => {
            if args.next().as_deref() != Some(std::ffi::OsStr::new("probe")) {
                return Err("usage: ovPilot xplane probe [host:port]".into());
            }
            let endpoint = args.next().unwrap_or_else(|| "127.0.0.1:49000".into());
            let address = endpoint.to_string_lossy().to_socket_addrs()?.next().ok_or("invalid X-Plane endpoint")?;
            let sample = XPlaneProbe::new(address, Duration::from_secs(3)).sample()?;
            println!("X-Plane {:?}: {}, {}, {} m", sample.version, sample.latitude, sample.longitude, sample.altitude_m);
            Ok(())
        }
        _ => Err("usage: ovPilot <run|scan [directories...]|about|update check|xplane probe [host:port]>".into()),
    }
}
