mod models;

use chrono::{DateTime, Local};
use clap::{value_parser, Arg, Command};
use std::{error::Error, io::Write, path::PathBuf};

use models::dms::DMS;

fn verify_pgp_signature(signature: String, keyring: PathBuf) -> Result<String, Box<dyn Error>> {
    let mut process = std::process::Command::new("gpgv")
        .args(&[
            "-q",
            "--keyring",
            keyring.to_str().unwrap(),
            "--output",
            "-",
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let maybe_stdin = process.stdin.take();
    if maybe_stdin.is_none() {
        let _ = process.kill();
        return Err("Unable to write stdin of gpgv process".into());
    }
    let mut stdin = maybe_stdin.unwrap();
    stdin.write_all(signature.as_bytes())?;
    drop(stdin);

    let output = process.wait_with_output()?;

    if !output.status.success() {
        output.stderr.iter().for_each(|b| print!("{}", *b as char));
        return Err(format!("gpgv returned exit status {}", output.status).into());
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn main() {
    let matches = Command::new("perimetr-dms")
        .about("Service that checks endpoints for signed timestamps and executes commands when threshold are reached.")
        .arg_required_else_help(false)
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .help("Path to config file")
                .default_value("dms.yml")
                .value_parser(value_parser!(PathBuf)),
        )
        .get_matches();

    let config_file_path = matches.get_one::<PathBuf>("config").unwrap();

    let mut reader = std::fs::File::open(config_file_path).expect("Failed to open config file");
    let mut config: DMS =
        serde_yaml::from_reader(&mut reader).expect("Failed to parse config file");

    let mut last_valid_timestamp = config
        .last_valid_timestamp
        .clone()
        .map(|ts| DateTime::parse_from_rfc3339(&ts).expect("Failed to parse last valid timestamp"));

    for timestamp_source in config.timestamp_sources.clone() {
        let request = reqwest::blocking::get(timestamp_source.clone());
        if request.is_err() {
            println!("Failed to request timestamp from {}", timestamp_source);
            continue;
        }
        let request_text = request.unwrap().text();
        if request_text.is_err() {
            println!("Failed to read timestamp from {}", timestamp_source);
            continue;
        }
        let signed_timestamp = request_text.unwrap();

        let verification_result =
            verify_pgp_signature(signed_timestamp, config.pgp_keyring_file.clone());

        if let Err(e) = verification_result {
            println!(
                "Failed to verify signature of source \"{}\": {}",
                timestamp_source, e
            );
            continue;
        }
        let verification_result = verification_result.unwrap();

        let valid_timestamp = verification_result.trim();
        let valid_datetime =
            DateTime::parse_from_rfc3339(valid_timestamp).expect("Failed to parse valid timestamp");

        if last_valid_timestamp.is_none() || last_valid_timestamp.unwrap() < valid_datetime {
            config.last_valid_timestamp = Some(valid_timestamp.to_string());
            last_valid_timestamp = Some(valid_datetime);

            println!(
                "Newer valid timestamp from \"{}\": {}. Resetting triggers.",
                timestamp_source, valid_timestamp
            );

            for action in config.threshold_actions.iter_mut() {
                action.triggered = Some(false);
            }

            update_config_file(config_file_path, &config);
        }
    }

    if config.last_valid_timestamp.is_none() || last_valid_timestamp.is_none() {
        println!("No valid timestamp found. Exiting.");
        std::process::exit(1);
    }
    let last_valid_timestamp = last_valid_timestamp.unwrap();

    println!(
        "Newest valid timestamp: {}. Checking threshold of actions…",
        config.last_valid_timestamp.clone().unwrap(),
    );

    'actionLoop: for action in config.threshold_actions.iter_mut() {
        if action.triggered.unwrap_or(false) {
            println!(
                "Skipping action with threshold {}s because it was already triggered.",
                action.threshold
            );
            continue;
        }

        let time_passed = Local::now().signed_duration_since(last_valid_timestamp);
        let threshold = chrono::Duration::seconds(action.threshold as i64);
        if time_passed < threshold {
            println!(
                "Skipping action with threshold {}s because it hasn't been reached yet ({} remaining).",
                action.threshold,
                threshold - time_passed
            );
            continue;
        }

        println!(
            "Threshold {}s reached. Executing configured commands.",
            action.threshold
        );

        for command in action.commands.iter_mut() {
            println!("Executing program: {}", command.program);
            let mut process = std::process::Command::new(command.program.clone());
            let mut process = process
                .args(command.args.clone())
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());

            if let Some(working_dir) = &command.working_dir {
                process = process.current_dir(working_dir);
            }

            let process = process.spawn();
            if let Err(e) = process {
                println!("Failed to execute command: {}", e);
                continue 'actionLoop;
            }
            let mut process = process.unwrap();

            if let Some(command_stdin) = &command.stdin {
                let maybe_stdin = process.stdin.take();
                if maybe_stdin.is_none() {
                    let _ = process.kill();
                    println!("Failed to get stdin of command");
                    continue 'actionLoop;
                }
                let mut stdin = maybe_stdin.unwrap();
                let res = stdin.write_all(command_stdin.as_bytes());
                if let Err(e) = res {
                    let _ = process.kill();
                    println!("Failed to write stdin of command: {}", e);
                    continue 'actionLoop;
                }
            }

            let exit_status = process.wait();
            if let Err(e) = exit_status {
                let _ = process.kill();
                println!("Failed to wait for command to finish: {}", e);
                continue 'actionLoop;
            }
            let exit_status = exit_status.unwrap();

            if !exit_status.success() {
                println!("Command failed with exit status {}", exit_status);
                continue 'actionLoop;
            }
        }

        println!("Commands finished successfully, marking action as triggered.");
        action.triggered = Some(true);
    }

    update_config_file(config_file_path, &config);
}

fn update_config_file(config_file_path: &PathBuf, config: &DMS) {
    let mut writer = std::fs::File::create(config_file_path).expect("Failed to open config file");
    serde_yaml::to_writer(&mut writer, &config).expect("Failed to write config file");
}
