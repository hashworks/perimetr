use std::io::Write;
use std::{fs, path::PathBuf};

use actix_web::{get, post, web, HttpResponse};
use bls12_381_plus::Scalar;
use sqlx::{Pool, Postgres};
use vsss_rs::{Feldman, Share};

use std::process::Command;

use crate::database::shares::{count_shares, insert_share, select_shares};
use crate::helper::strings::null_terminated_bytes_to_string;
use crate::helper::vsss::base64_str_to_share;
use crate::models::layer::{Layer, LayerState};
use crate::Configuration;

async fn decrypt_layer(
    db_pool: web::Data<Pool<Postgres>>,
    filepath: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // read layer metadata again to make sure it isn't already being decrypted
    let mut layer = Layer::read_metadata(&filepath)?;
    if layer.state != LayerState::Idle {
        return Ok(());
    }

    // lock layer
    layer.state = LayerState::Decrypting;
    layer.write_metadata(&filepath)?;

    let shares = select_shares(&db_pool, layer.uuid.clone()).await?;

    let threshold = layer.vsss.as_ref().map(|v| v.threshold).unwrap_or(1);

    if shares.len() < threshold as usize {
        return Err("Not enough shares to decrypt layer".into());
    }

    // combine shares to secret if needed
    let secret = if threshold > 1 {
        let mut vsss_shares = Vec::with_capacity(shares.len());
        for share in shares {
            let share = base64_str_to_share(&share)?;
            vsss_shares.push(share);
        }
        let res = Feldman {
            t: threshold as usize,
            n: 255,
        }
        .combine_shares::<Scalar>(&vsss_shares);
        if let Err(e) = res {
            return Err(format!("{:?}", e).into());
        }
        null_terminated_bytes_to_string(&res.unwrap().to_bytes())?
    } else {
        shares.first().unwrap().to_string() // asserted: shares.len() > 1
    };

    let working_dir = filepath.parent().unwrap_or(".".as_ref());

    // call commands for decryption process
    for command in layer.commands.iter() {
        let action_working_dir = if command.working_dir.is_empty() {
            working_dir.to_path_buf()
        } else {
            if command.working_dir.starts_with("/") {
                PathBuf::from(&command.working_dir)
            } else {
                working_dir.join(&command.working_dir)
            }
        };

        let mut process = Command::new(command.program.clone())
            .args(command.args.clone())
            .current_dir(action_working_dir)
            .stdin(std::process::Stdio::piped())
            .spawn()?;

        if command.secret_stdin {
            let maybe_stdin = process.stdin.take();
            if maybe_stdin.is_none() {
                let _ = process.kill();
                return Err("Unable to write stdin of process".into());
            }
            let mut stdin = maybe_stdin.unwrap();
            stdin.write_all(secret.as_bytes())?;
        }

        let status = process.wait()?;

        if !status.success() {
            return Err(format!(
                "Action \"{}\" returned exit status {}",
                command.program, status
            )
            .into());
        }
    }

    if filepath.exists() {
        layer.state = LayerState::Decrypted;
        layer.write_metadata(&filepath)?;
    }

    Ok(())
}

#[post("/layer/{uuid}/share")]
pub(crate) async fn provide_share_for_layer(
    db_pool: web::Data<Pool<Postgres>>,
    config: web::Data<Configuration>,
    layer_uuid: web::Path<String>,
    share_str: String,
) -> Result<HttpResponse, Box<dyn std::error::Error>> {
    if let Some((filepath, layer)) =
        find_layer_file(config.layer_path.clone(), layer_uuid.to_string())?
    {
        let mut threshold = 1;
        if let Some(vsss) = layer.vsss.as_ref() {
            let share = base64::decode(share_str.clone())?;
            let share = Share::try_from(&share[..])?;
            if !vsss.feldman_verifier.verify(&share) {
                return Ok(HttpResponse::BadRequest().body("Invalid share"));
            }
            threshold = vsss.threshold;
        }

        if insert_share(&db_pool, layer_uuid.to_string(), share_str).await? {
            let res = count_shares(&db_pool, layer_uuid.to_string()).await?;

            if let Some(count) = res {
                if count >= threshold as i64 {
                    actix_rt::spawn(async move {
                        if let Err(e) = decrypt_layer(db_pool, &filepath).await {
                            log::error!("Decryption failed: {}", e);
                            let mut layer = layer;
                            layer.state = LayerState::Idle;
                            let _ = layer.write_metadata(&filepath);
                        }
                    });

                    return Ok(
                        HttpResponse::Ok().body("Share accepted, threshold reached. Decrypting.")
                    );
                }
            }

            return Ok(
                HttpResponse::Ok().body("Share accepted, threshold hasn't been reached yet.")
            );
        }
        return Ok(HttpResponse::InternalServerError().body("Failed to store share"));
    } else {
        return Ok(HttpResponse::NotFound().finish());
    }
}

#[get("/layers")]
pub(crate) async fn get_available_layers(
    config: web::Data<Configuration>,
) -> Result<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::Ok().json(rec_read_layer_files(
        config.layer_path.clone(),
        config.layer_suffix.clone(),
    )?))
}

fn rec_read_layer_files(path: PathBuf, suffix: String) -> Result<Vec<Layer>, actix_web::Error> {
    let mut files: Vec<Layer> = Vec::new();

    let dir_entries = fs::read_dir(path)?;

    for dir_entry in dir_entries {
        let dir_entry = dir_entry?;
        if dir_entry.metadata()?.is_dir() {
            files.append(&mut rec_read_layer_files(dir_entry.path(), suffix.clone())?);
        } else if dir_entry.file_name().to_string_lossy().ends_with(&suffix) {
            if let Ok(layer) = Layer::read_metadata(&dir_entry.path()) {
                files.push(layer);
            } else {
                log::warn!("Failed to read YAML file: {}", dir_entry.path().display());
            }
        }
    }

    Ok(files)
}

fn find_layer_file(
    path: PathBuf,
    uuid: String,
) -> Result<Option<(PathBuf, Layer)>, actix_web::Error> {
    let dir_entries = fs::read_dir(path)?;

    for dir_entry in dir_entries {
        let dir_entry = dir_entry?;
        if dir_entry.metadata()?.is_dir() {
            if let Some(result) = find_layer_file(dir_entry.path(), uuid.clone())? {
                return Ok(Some(result));
            }
        } else if dir_entry
            .file_name()
            .to_string_lossy()
            .ends_with(".layer.yml")
        {
            if let Ok(layer) = Layer::read_metadata(&dir_entry.path()) {
                if layer.uuid == uuid {
                    return Ok(Some((dir_entry.path(), layer)));
                }
            } else {
                log::error!("Failed to read YAML file: {}", dir_entry.path().display());
            }
        }
    }

    Ok(None)
}
