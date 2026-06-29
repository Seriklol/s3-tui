use crate::s3::s3_profile::S3Profile;
use anyhow::anyhow;
use bendy::serde::from_bytes;
use keyring::Entry;
use serde::{Deserialize, Serialize};

const KEYRING_SERVICE: &str = "rust-s3-tui";

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct S3Secret {
    pub profiles: Vec<S3Profile>,
}

impl S3Secret {
    pub fn from_keyring() -> anyhow::Result<Self> {
        let user = match users::get_current_username() {
            None => Err(anyhow!("Could not get the current username")),
            Some(name) => Ok(name),
        }?;
        let user = user.to_string_lossy();
        let entry = Entry::new(KEYRING_SERVICE, &user)?
            .get_secret()
            .unwrap_or_default();

        Ok(from_bytes::<S3Secret>(entry.as_slice()).unwrap_or(S3Secret { profiles: vec![] }))
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let user = match users::get_current_username() {
            None => Err(anyhow!("Could not get the current username")),
            Some(name) => Ok(name),
        }?;

        let user = user.to_string_lossy();
        let entry = Entry::new(KEYRING_SERVICE, &user)?;
        let bytes = bendy::serde::to_bytes(self)?;

        Ok(entry.set_secret(&bytes)?)
    }
}
