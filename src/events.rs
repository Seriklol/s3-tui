use crate::s3::objects::S3ObjectInfo;
use std::sync::Arc;

pub enum S3Update {
    ListObjects(crate::s3::objects::S3ObjectsList),
    ObjectInfo(Arc<S3ObjectInfo>),
    DownloadProgress(FileUpdate),
    UploadProgress(FileUpdate),
    Error(anyhow::Error),
}

pub struct FileUpdate {
    pub bucket: String,
    pub key: String,
    pub result: Result<FileProgress, anyhow::Error>,
}

impl FileUpdate {
    pub fn new(bucket: String, key: String, result: Result<FileProgress, anyhow::Error>) -> Self {
        Self { bucket, key, result }
    }
}

pub struct FileProgress {
    pub done: u64,
    pub size: u64,
}

impl FileProgress {
    pub fn new(done: u64, size: u64) -> Self {
        Self { done, size }
    }
}
