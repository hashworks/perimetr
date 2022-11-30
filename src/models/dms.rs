use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ignore option

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DMS {
    pub(crate) timestamp_sources: Vec<String>,
    pub(crate) pgp_keyring_file: PathBuf,
    pub(crate) threshold_actions: Vec<DMSAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_valid_timestamp: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DMSAction {
    pub(crate) commands: Vec<DMSCommand>,
    pub(crate) threshold: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) triggered: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug)]
pub(crate) struct DMSCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) stdin: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) working_dir: Option<String>,
}
