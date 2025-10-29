use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct S3Profile {
    pub name: String,
    pub endpoint: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
}

impl S3Profile {
    pub fn new(
        name: String,
        endpoint: String,
        region: String,
        access_key_id: String,
        secret_access_key: String,
    ) -> Self {
        Self {
            name,
            endpoint,
            region,
            access_key_id,
            secret_access_key,
        }
    }
}
