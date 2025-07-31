mod cli;

use clap::Parser;
use cli::{Cli, Commands};
use colored::Colorize;
use scx_loader::{dbus::LoaderClientProxyBlocking, SchedMode, SupportedSched};
use std::process::exit;
use zbus::blocking::Connection;

fn cmd_get(scx_loader: LoaderClientProxyBlocking) -> Result<(), Box<dyn std::error::Error>> {
    let current_scheduler: String = scx_loader.current_scheduler()?;

    match current_scheduler.as_str() {
        "unknown" => println!("no scx scheduler running"),
        _ => {
            let sched = SupportedSched::try_from(current_scheduler.as_str())?;
            let current_args: Vec<String> = scx_loader.current_scheduler_args()?;

            if current_args.is_empty() {
                let sched_mode: SchedMode = scx_loader.scheduler_mode()?;
                println!("running {sched:?} in {sched_mode:?} mode");
            } else {
                println!(
                    "running {sched:?} with arguments \"{}\"",
                    current_args.join(" ")
                );
            }
        }
    }
    Ok(())
}

fn cmd_list(scx_loader: LoaderClientProxyBlocking) -> Result<(), Box<dyn std::error::Error>> {
    match scx_loader.supported_schedulers() {
        Ok(sl) => {
            let supported_scheds = sl
                .iter()
                .map(|s| remove_scx_prefix(&s.to_string()))
                .collect::<Vec<String>>();
            println!("supported schedulers: {:?}", supported_scheds);
            return Ok(());
        }
        Err(e) => {
            eprintln!("scheduler list failed: {e}");
            exit(1);
        }
    };
}

fn cmd_start(
    scx_loader: LoaderClientProxyBlocking,
    sched_name: String,
    mode_name: Option<SchedMode>,
    args: Option<Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Verify scx_loader is not running a scheduler
    let current_scheduler = scx_loader.current_scheduler().unwrap();
    if current_scheduler != "unknown" {
        println!(
            "{} scx scheduler already running, use '{}' instead of '{}'",
            "error:".red().bold(),
            "switch".bold(),
            "start".bold()
        );
        println!("\nFor more information, try '{}'", "--help".bold());
        exit(1);
    }

    let sched: SupportedSched = validate_sched(scx_loader.clone(), sched_name);
    let mode: SchedMode = mode_name.unwrap_or(SchedMode::Auto);
    match args {
        Some(args) => {
            scx_loader.start_scheduler_with_args(sched.clone(), &args.clone())?;
            println!("started {sched:?} with arguments \"{}\"", args.join(" "));
        }
        None => {
            scx_loader.start_scheduler(sched.clone(), mode.clone())?;
            println!("started {sched:?} in {mode:?} mode");
        }
    }
    Ok(())
}

fn cmd_switch(
    scx_loader: LoaderClientProxyBlocking,
    sched_name: Option<String>,
    mode_name: Option<SchedMode>,
    args: Option<Vec<String>>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Cache DBUS call result
    let current_scheduler = scx_loader.current_scheduler().unwrap();

    // Verify scx_loader is running a scheduler
    if current_scheduler == "unknown" {
        println!(
            "{} no scx scheduler running, use '{}' instead of '{}'",
            "error:".red().bold(),
            "start".bold(),
            "switch".bold()
        );
        println!("\nFor more information, try '{}'", "--help".bold());
        exit(1);
    }

    let sched: SupportedSched = match sched_name {
        Some(sched_name) => validate_sched(scx_loader.clone(), sched_name),
        None => SupportedSched::try_from(current_scheduler.as_str()).unwrap(),
    };
    let mode: SchedMode = match mode_name {
        Some(mode_name) => mode_name,
        None => scx_loader.scheduler_mode().unwrap(),
    };
    match args {
        Some(args) => {
            scx_loader.switch_scheduler_with_args(sched.clone(), &args.clone())?;
            println!(
                "switched to {sched:?} with arguments \"{}\"",
                args.join(" ")
            );
        }
        None => {
            scx_loader.switch_scheduler(sched.clone(), mode.clone())?;
            println!("switched to {sched:?} in {mode:?} mode");
        }
    }
    Ok(())
}

fn cmd_stop(scx_loader: LoaderClientProxyBlocking) -> Result<(), Box<dyn std::error::Error>> {
    scx_loader.stop_scheduler()?;
    println!("stopped");
    Ok(())
}

fn cmd_restart(scx_loader: LoaderClientProxyBlocking) -> Result<(), Box<dyn std::error::Error>> {
    scx_loader.restart_scheduler()?;
    println!("restarted");
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let conn = Connection::system()?;
    let scx_loader = LoaderClientProxyBlocking::new(&conn)?;

    match cli.command {
        Commands::Get => cmd_get(scx_loader)?,
        Commands::List => cmd_list(scx_loader)?,
        Commands::Start { args } => cmd_start(scx_loader, args.sched, args.mode, args.args)?,
        Commands::Switch { args } => cmd_switch(scx_loader, args.sched, args.mode, args.args)?,
        Commands::Stop => cmd_stop(scx_loader)?,
        Commands::Restart => cmd_restart(scx_loader)?,
    }

    Ok(())
}

/*
 * Utilities
 */

const SCHED_PREFIX: &str = "scx_";

fn ensure_scx_prefix(input: String) -> String {
    if !input.starts_with(SCHED_PREFIX) {
        return format!("{}{}", SCHED_PREFIX, input);
    }
    input
}

fn remove_scx_prefix(input: &String) -> String {
    if let Some(strip_input) = input.strip_prefix(SCHED_PREFIX) {
        return strip_input.to_string();
    }
    input.to_string()
}

fn validate_sched(scx_loader: LoaderClientProxyBlocking, sched: String) -> SupportedSched {
    let raw_supported_scheds: Vec<String> = scx_loader.supported_schedulers().unwrap();
    let supported_scheds: Vec<String> = raw_supported_scheds
        .iter()
        .map(|s| remove_scx_prefix(s))
        .collect();
    if !supported_scheds.contains(&sched) && !raw_supported_scheds.contains(&sched) {
        println!(
            "{} invalid value '{}' for '{}'",
            "error:".red().bold(),
            &sched.yellow(),
            "--sched <SCHED>".bold()
        );
        println!("supported schedulers: {:?}", supported_scheds);
        println!("\nFor more information, try '{}'", "--help".bold());
        exit(1);
    }

    SupportedSched::try_from(ensure_scx_prefix(sched).as_str()).unwrap()
}
