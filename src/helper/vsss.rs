use std::error::Error;

use vsss_rs::Share;

pub(crate) fn base64_str_to_share(share: &str) -> Result<Share, Box<dyn Error>> {
    let share = base64::decode(share)?;
    let share = Share::try_from(&share[..])?;
    Ok(share)
}
