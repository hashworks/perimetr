use std::{error::Error, path::PathBuf};

use bls12_381_plus::{G1Projective, Scalar};
use serde::{Deserialize, Serialize};
use vsss_rs::FeldmanVerifier;

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct Layer {
    pub(crate) uuid: String,
    pub(crate) state: LayerState,
    pub(crate) commands: Vec<LayerCommands>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) vsss: Option<VSSSMetadata>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub(crate) enum LayerState {
    #[serde(rename = "idle")]
    Idle,
    #[serde(rename = "decrypting")]
    Decrypting,
    #[serde(rename = "decrypted")]
    Decrypted,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct LayerCommands {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) working_dir: String,
    pub(crate) secret_stdin: bool,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct VSSSMetadata {
    pub(crate) threshold: u8,
    pub(crate) feldman_verifier: FeldmanVerifier<Scalar, G1Projective>,
}

impl Layer {
    #[allow(dead_code)]
    pub(crate) fn read_metadata(metadata_file: &PathBuf) -> Result<Layer, Box<dyn Error>> {
        let mut reader = std::fs::File::open(metadata_file)?;
        Ok(serde_yaml::from_reader(&mut reader)?)
    }

    #[allow(dead_code)]
    pub(crate) fn write_metadata(&self, metadata_file: &PathBuf) -> Result<(), Box<dyn Error>> {
        let mut writer = std::fs::File::create(metadata_file)?;
        serde_yaml::to_writer(&mut writer, self)?;
        Ok(())
    }
}
