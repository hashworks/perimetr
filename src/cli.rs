mod helper;
mod models;

use std::{error::Error, path::PathBuf};

use bls12_381_plus::{G1Projective, Scalar};
use clap::{value_parser, Arg, ArgAction, Command};
use helper::strings::null_terminated_bytes_to_string;
use helper::vsss::base64_str_to_share;
use rand::rngs::OsRng;
use vsss_rs::{Feldman, Share};

use crate::models::layer::{Layer, LayerCommands, LayerState, VSSSMetadata};

fn split_secret(
    secret_str: &str,
    threshold: &u8,
    shares: &u8,
) -> Result<
    (
        Vec<vsss_rs::Share>,
        vsss_rs::FeldmanVerifier<Scalar, G1Projective>,
    ),
    Box<dyn Error>,
> {
    if secret_str.len() < 1 || secret_str.len() > 32 {
        return Err("Secret must be 1-32 bytes in size".into());
    }
    let mut input_bytes = [0u8; 32];
    input_bytes[..secret_str.len()].copy_from_slice(secret_str.as_bytes());
    let secret = Scalar::from_bytes(&input_bytes);
    if bool::from(secret.is_none()) {
        return Err("Unable to convert passphrase to scalar".into());
    }
    let secret = secret.unwrap();
    let res = Feldman {
        t: *threshold as usize,
        n: *shares as usize,
    }
    .split_secret::<Scalar, G1Projective, OsRng>(secret, None, &mut OsRng::default());
    match res {
        Ok((shares, verifier)) => Ok((shares, verifier)),
        Err(e) => Err(format!("Failed to split secret ({:?})", e).into()),
    }
}

fn combine_shares_to_secret_string(
    threshold: usize,
    shares: Vec<Share>,
) -> Result<String, Box<dyn Error>> {
    let res = Feldman {
        t: threshold,
        n: 255,
    }
    .combine_shares::<Scalar>(&shares);
    if let Err(e) = res {
        return Err(format!("{:?}", e).into());
    }
    Ok(null_terminated_bytes_to_string(&res.unwrap().to_bytes())?)
}

fn read_and_verify_shares(vsss: &VSSSMetadata) -> Result<Vec<Share>, Box<dyn Error>> {
    let mut shares = Vec::new();
    for i in 0..vsss.threshold {
        let share = rpassword::prompt_password(format!(
            "Please provide share {} of {}: ",
            i + 1,
            vsss.threshold
        ))?;
        let share = share.trim();

        let share = base64_str_to_share(share)?;
        if !vsss.feldman_verifier.verify(&share) {
            return Err("Invalid share".into());
        }
        shares.push(share);
    }
    Ok(shares)
}

fn main() {
    let matches = Command::new("perimetr")
        .about("CLI tool to generate perimetr layers and decrypt them manually if needed.")
        .arg_required_else_help(true)
        .subcommand(
            Command::new("split")
                .about("Split a secret into shares and store metadata in the metadata-dir.")
                .arg(
                    Arg::new("shares")
                        .short('s')
                        .long("shares")
                        .help("Number of shares to generate (max: 255)")
                        .required(true)
                        .value_parser(value_parser!(u8).range(1..)),
                )
                .arg(
                    Arg::new("threshold")
                        .short('t')
                        .long("threshold")
                        .help("Threshold of shares needed to recover secret (max: 255)")
                        .required(true)
                        .value_parser(value_parser!(u8).range(1..)),
                )
                .arg(
                    Arg::new("metadata-path")
                        .short('m')
                        .long("metadata-path")
                        .help("Path to metadata file or directory")
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("default-actions")
                        .short('d')
                        .long("default-actions")
                        .help("Include default actions in metadata file")
                        .action(ArgAction::SetTrue),
                ),
        )
        .subcommand(
            Command::new("combine")
                .about("Combine shares into a secret with the provided metadata-file")
                .arg(
                    Arg::new("metadata-file")
                        .short('m')
                        .long("metadata-file")
                        .help("Path to metadata file")
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                ),
        )
        .get_matches();

    match matches.subcommand() {
        Some(("split", split_matches)) => {
            // safe unwraps because of required(true)
            let shares: &u8 = split_matches.get_one("shares").unwrap();
            let threshold: &u8 = split_matches.get_one("threshold").unwrap();
            let metadata_path: &PathBuf = split_matches.get_one("metadata-path").unwrap();

            if threshold > shares {
                println!("Error: Threshold must be lower than or equal to shares");
                std::process::exit(1);
            }

            let mut layer = Layer {
                uuid: uuid::Uuid::new_v4().to_string(),
                state: LayerState::Idle,
                commands: Vec::new(),
                vsss: None,
            };

            let metadata_path = if metadata_path.is_dir() {
                metadata_path.join(format!("{}.layer.yml", layer.uuid))
            } else {
                metadata_path.to_path_buf()
            };

            if *threshold > 1 {
                println!("Please provide a secret with up to 32 bytes on STDIN.");

                // TODO: This doesn't work with pipes
                let input = rpassword::read_password();
                if let Err(e) = input {
                    println!("Error: Failed to read input ({})", e);
                    std::process::exit(1);
                }
                let input = input.unwrap();
                let input = input.trim();

                let res = split_secret(input, threshold, shares);
                if let Err(e) = res {
                    println!("Error: {}", e);
                    std::process::exit(1);
                }
                let (shares, verifier) = res.unwrap();

                println!("Shares of \"{}\":", layer.uuid);
                for share in shares {
                    let share = base64::encode(share);
                    println!("{}", share);
                }

                layer.vsss = Some(VSSSMetadata {
                    threshold: *threshold,
                    feldman_verifier: verifier,
                });
                println!();
            } else {
                println!("Threshold is 1, no need to split secret.");
            }

            if *split_matches.get_one("default-actions").unwrap_or(&false) {
                layer.commands = vec![
                    LayerCommands {
                        program: "gpg".to_string(),
                        args: vec![
                            "--decrypt",
                            "--passphrase-fd",
                            "0",
                            "--batch",
                            "-o",
                            &format!("{}.tar.zst", layer.uuid).to_string(),
                            &format!("{}.tar.zst.gpg", layer.uuid).to_string(),
                        ]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                        working_dir: ".".to_string(),
                        secret_stdin: true,
                    },
                    LayerCommands {
                        program: "tar".to_string(),
                        args: vec!["xf".to_string(), format!("{}.tar.zst", layer.uuid)],
                        working_dir: ".".to_string(),
                        secret_stdin: false,
                    },
                    LayerCommands {
                        program: "rm".to_string(),
                        args: vec![
                            format!("{}.tar.zst", layer.uuid),
                            format!("{}.tar.zst.gpg", layer.uuid),
                        ],
                        working_dir: ".".to_string(),
                        secret_stdin: false,
                    },
                ];
            }

            if let Err(e) = layer.write_metadata(&metadata_path) {
                println!("Error: {}", e);
                std::process::exit(1);
            }

            println!("Metadata written to \"{}\".", metadata_path.display());
        }
        Some(("combine", _combine_matches)) => {
            let metadata_file: &PathBuf = matches.get_one("metadata-file").unwrap();

            let res = Layer::read_metadata(metadata_file);
            if let Err(e) = res {
                println!("Error: {}", e);
                std::process::exit(1);
            }
            let layer = res.unwrap();

            if layer.vsss.is_none() {
                println!(
                    "Error: No VSSS metadata found in {}",
                    metadata_file.display()
                );
                std::process::exit(1);
            }
            let vsss = layer.vsss.unwrap();

            let shares = read_and_verify_shares(&vsss);
            if let Err(e) = shares {
                println!("Error: {}", e);
                std::process::exit(1);
            }
            let shares = shares.unwrap();

            let res = combine_shares_to_secret_string(vsss.threshold as usize, shares);
            if let Err(e) = res {
                println!("Error: Failed to combine shares ({})", e);
                std::process::exit(1);
            }

            println!("Secret: {}", res.unwrap());
        }
        _ => unreachable!(),
    }
}
