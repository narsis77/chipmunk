mod app_runner;
mod checksum_records;
mod cli_args;
mod dev_environment;
mod dev_tools;
mod fail_fast;
mod fstools;
mod job_type;
mod jobs_runner;
mod location;
mod log_print;
mod print_dot;
mod shell_completion;
mod spawner;
mod target;
mod tracker;

use anyhow::{bail, Error};
use checksum_records::ChecksumRecords;
use clap::Parser;
use cli_args::{CargoCli, Command, UiMode};
use console::style;
use dev_environment::{print_env_info, resolve_dev_tools};
use fail_fast::set_fail_fast;
use job_type::JobType;
use jobs_runner::cancellation::{cancellation_token, graceful_shutdown};
use location::init_location;
use log_print::{print_log_separator, print_report};
use std::{fs::File, io::Stdout, path::PathBuf};
use target::Target;
use tokio::signal;
use tracker::{get_tracker, init_tracker};

use crate::cli_args::EnvironmentCommand;

#[derive(Debug)]
pub enum ReportOptions {
    None,
    Stdout(Stdout),
    File(PathBuf, File),
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    // CLI command parsing.
    let CargoCli::Chipmunk(cli) = CargoCli::parse();
    let command = cli.command;

    // Validate current directory location.
    init_location()?;

    // Handle the app main process in a separate method, keeping this method for handling
    // manual cancelling as well.

    tokio::select! {
        main_res = main_process(command) => {
            return main_res
        }
        _ = signal::ctrl_c() => {
            // Cancel all the running tasks and wait for them to return.
            cancellation_token().cancel();
            graceful_shutdown().await;

            // Shutdown the tracker channels.
            let tracker = get_tracker();
            tracker.shutdown(false).await?;

            eprintln!();
            eprintln!("Tasks have been cancelled");

            return Ok(())
        }
    }
}

// The main process of the app is encapsulated in this method so we can use it inside select loop
// to handle manual cancellation and have a graceful shutdown for the application in that case too.
async fn main_process(command: Command) -> Result<(), Error> {
    // Run the given command
    let (job_type, results) = match command {
        Command::Environment(sub_command) => match sub_command {
            EnvironmentCommand::Check => {
                resolve_dev_tools()?;
                println!("All needed tools for development are installed");
                return Ok(());
            }
            EnvironmentCommand::Print => {
                print_env_info();
                return Ok(());
            }
        },
        Command::PrintDot { all_jobs } => {
            if all_jobs {
                print_dot::print_dependencies_jobs();
            } else {
                print_dot::print_dependencies_targets();
            }
            return Ok(());
        }
        Command::Lint {
            target,
            fail_fast,
            ui_mode,
        } => {
            set_fail_fast(fail_fast);
            init_tracker(ui_mode);
            resolve_dev_tools()?;
            let targets = get_targets_or_default(target);
            let results = jobs_runner::run(&targets, JobType::Lint).await?;
            (JobType::Lint, results)
        }
        Command::Build {
            target,
            production,
            fail_fast,
            ui_mode,
        } => {
            set_fail_fast(fail_fast);
            init_tracker(ui_mode);
            resolve_dev_tools()?;
            let targets = get_targets_or_default(target);
            let results = jobs_runner::run(&targets, JobType::Build { production }).await?;
            (JobType::Build { production }, results)
        }
        Command::Clean { target, ui_mode } => {
            set_fail_fast(false);
            init_tracker(ui_mode);
            resolve_dev_tools()?;
            let targets = get_targets_or_default(target);
            let results = jobs_runner::run(&targets, JobType::Clean).await?;
            (JobType::Clean, results)
        }
        Command::Test {
            target,
            production,
            fail_fast,
            ui_mode,
        } => {
            set_fail_fast(fail_fast);
            init_tracker(ui_mode);
            resolve_dev_tools()?;
            let targets = get_targets_or_default(target);
            let results = jobs_runner::run(&targets, JobType::Test { production }).await?;
            (JobType::Test { production }, results)
        }
        Command::Run {
            production,
            fail_fast,
        } => {
            set_fail_fast(fail_fast);
            init_tracker(Default::default());
            resolve_dev_tools()?;
            let results = jobs_runner::run(&[Target::App], JobType::Build { production }).await?;
            (JobType::Run { production }, results)
        }
        Command::ResetChecksum { production } => {
            ChecksumRecords::remove_records_file(production)?;
            println!(
                "Checksum-Records for {} has been reset",
                if production {
                    "production"
                } else {
                    "development"
                }
            );

            return Ok(());
        }
        Command::ShellCompletion { shell } => {
            shell_completion::generate_completion(shell)?;

            return Ok(());
        }
    };

    // Shutdown and show results & report
    let tracker = get_tracker();
    tracker.shutdown(true).await?;
    let mut success: bool = true;
    let print_err = match tracker.ui_mode() {
        UiMode::ProgressBars => true,
        UiMode::BarsWithReport | UiMode::PrintOnJobFinish | UiMode::PrintImmediately => {
            // logs have been already printed.
            false
        }
    };
    for (idx, res) in results.iter().enumerate() {
        match res {
            Ok(status) => {
                if matches!(tracker.ui_mode(), UiMode::BarsWithReport) {
                    if idx == 0 {
                        println!();
                        println!();
                        println!("{}", style("Build Logs Report:").bold().blue());
                        println!();
                    }
                    print_report(status);
                    print_log_separator();
                }

                if !status.status.success() {
                    if print_err {
                        eprintln!("Failed with errors");
                        eprintln!("{}:\n{}", status.job, status.report.join(""));
                        eprintln!(
                            "---------------------------------------------------------------------"
                        );
                    }
                    success = false;
                }
            }
            Err(err) => {
                eprintln!("Builder error: {:?}", err);
                eprintln!("---------------------------------------------------------------------");
                success = false;
            }
        }
    }
    if !success {
        bail!("Some tasks have failed")
    };

    ChecksumRecords::update_and_save(job_type)?;

    if matches!(job_type, JobType::Run { .. }) {
        println!("Starting chipmunk...");
        let status = app_runner::run_app().await?;
        if !status.success() {
            bail!("Error: Chipmunk Exited with the Code {status}");
        }
    }
    Ok(())
}

fn get_targets_or_default(targets: Option<Vec<Target>>) -> Vec<Target> {
    if let Some(mut list) = targets {
        list.dedup();
        list
    } else {
        Target::all().to_vec()
    }
}
