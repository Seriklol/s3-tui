use crate::events::{FileProgress, FileUpdate, S3Update};
use anyhow::anyhow;
use aws_sdk_s3::error::{DisplayErrorContext, ProvideErrorMetadata};
use aws_sdk_s3::operation::head_object::HeadObjectOutput;
use aws_sdk_s3::primitives::{ByteStream, Length};
use aws_sdk_s3::types::{CompletedMultipartUpload, CompletedPart, MultipartUpload, Object};
use aws_sdk_s3::Client;
use directories::UserDirs;
use std::path::PathBuf;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedSender;

pub struct S3ObjectsList {
    pub bucket: String,
    pub objects: Vec<Object>,
    pub uploads: Vec<MultipartUpload>,
}

pub struct S3ObjectInfo {
    pub key: String,
    pub info: HeadObjectOutput,
}

const DOWNLOADING_SUFFIX: &str = ".downloading";
const BUFFER_SIZE: u64 = 5 * 1024 * 1024;
const S3_PARTS_LIMIT: u64 = 1_000;

pub fn get_file_name(key: &str) -> anyhow::Result<&str> {
    let name = key
        .split('/')
        .next_back()
        .ok_or(anyhow!("Could not get file name from path: {0}", key))?;

    Ok(name)
}

pub fn get_file_download_path(key: &str) -> anyhow::Result<PathBuf> {
    let downloading_file_name = get_file_name(key)?.to_owned() + DOWNLOADING_SUFFIX;
    let downloading_path = UserDirs::new()
        .ok_or(anyhow!("user directory not found"))?
        .download_dir()
        .ok_or(anyhow!("downloads directory not found"))?
        .join(downloading_file_name);

    Ok(downloading_path)
}

pub async fn list_buckets(client: &Client) -> anyhow::Result<Vec<String>>{
    let buckets = client
        .list_buckets()
        .send()
        .await
        .map_err(|err| anyhow!("Could not list buckets: {}", DisplayErrorContext(&err)))?
        .buckets
        .unwrap_or_default()
        .iter()
        .map(|b| b.name.clone().unwrap_or_default())
        .filter(|name| !name.is_empty())
        .collect();
    
    Ok(buckets)
}

pub async fn list_objects(client: &Client, bucket: &str) -> anyhow::Result<S3ObjectsList> {
    let mut objects_resp = client
        .list_objects_v2()
        .bucket(bucket)
        .into_paginator()
        .send();

    let mut all_objects: Vec<Object> = vec![];
    while let Some(result) = objects_resp.next().await {
        if let Some(mut objects) = result
            .map_err(|err| anyhow!("Could not get objects: {}", DisplayErrorContext(&err)))?
            .contents
        {
            all_objects.append(&mut objects)
        }
    }

    let uploads = client
        .list_multipart_uploads()
        .bucket(bucket)
        .send()
        .await
        .map_err(|err| {
            anyhow!("Could not get unfinished uploads: {}", DisplayErrorContext(&err))
        })?
        .uploads
        .unwrap_or_default();

    Ok(S3ObjectsList {
        bucket: String::from(bucket),
        objects: all_objects,
        uploads,
    })
}

pub async fn download_object(
    client: &Client,
    key: &str,
    bucket: &str,
    tx: &UnboundedSender<S3Update>,
) {
    if let Err(err) = download(client, key, bucket, tx).await {
        let _ = tx.send(S3Update::DownloadProgress(FileUpdate::new(
            bucket.to_string(),
            key.into(),
            Err(err),
        )));
    }
}

async fn download(
    client: &Client,
    key: &str,
    bucket: &str,
    tx: &UnboundedSender<S3Update>,
) -> anyhow::Result<()> {
    let downloading_path = get_file_download_path(key)?;
    let final_name = get_file_name(key)?;
    let final_path = downloading_path.with_file_name(final_name);

    let object_size = get_object_info(client, bucket, key)
        .await?
        .content_length()
        .ok_or(anyhow!("Could not get file size from key {0}", key))? as u64;

    if final_path.is_file() {
        return Err(anyhow!(
            "Could not download file, since it already exists at path: {final_path:?}"
        ));
    }

    let mut from = 0;
    if downloading_path.is_file() {
        from = downloading_path.metadata()?.len();
        if from >= object_size {
            std::fs::rename(&downloading_path, final_path)?;

            return Err(anyhow!(
                "Could not download file, since it was already downloaded but not renamed"
            ));
        }
    }

    let range = format!("bytes={}-{}", from, object_size);
    let resp = client
        .get_object()
        .bucket(bucket)
        .key(key)
        .range(range)
        .send()
        .await;

    match resp {
        Err(err) => Err(anyhow!(
            "Error when downloading object {0} from bucket {1}: {2}",
            key,
            bucket,
            err.into_service_error()
        )),
        Ok(resp) => {
            tx.send(S3Update::DownloadProgress(FileUpdate::new(
                bucket.to_string(),
                key.to_string(),
                Ok(FileProgress::new(from, object_size)),
            )))?;

            let mut downloaded = from;
            let mut body = resp.body.into_async_read();
            let mut file = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&downloading_path)
                .await?;
            let mut buffer = vec![0; BUFFER_SIZE as usize];

            let mut written_chunk = 0;
            loop {
                let n = body.read(&mut buffer).await?;
                if n == 0 {
                    break;
                }

                file.write_all(&buffer[0..n]).await?;
                written_chunk += n;
                downloaded += n as u64;

                if written_chunk >= BUFFER_SIZE as usize {
                    written_chunk = 0;
                    tx.send(S3Update::DownloadProgress(FileUpdate::new(
                        bucket.to_string(),
                        key.to_string(),
                        Ok(FileProgress::new(downloaded, object_size)),
                    )))?;
                }
            }

            tx.send(S3Update::DownloadProgress(FileUpdate::new(
                bucket.to_string(),
                key.to_string(),
                Ok(FileProgress::new(downloaded, object_size)),
            )))?;
            std::fs::rename(&downloading_path, final_path)?;
            Ok(())
        }
    }
}

pub async fn delete_object(client: &Client, key: &str, bucket: &str) -> anyhow::Result<()> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await?;

    Ok(())
}

pub async fn upload_object(
    client: &Client,
    bucket: &str,
    key: &str,
    path: &str,
    tx: &UnboundedSender<S3Update>,
) {
    if let Err(err) = upload(client, bucket, key, path, tx).await {
        let _ = tx.send(S3Update::UploadProgress(FileUpdate::new(
            bucket.to_string(),
            key.into(),
            Err(err),
        )));
    }
}

async fn upload(
    client: &Client,
    bucket: &str,
    key: &str,
    path: &str,
    tx: &UnboundedSender<S3Update>,
) -> anyhow::Result<()> {
    let path_buf = PathBuf::from(path);
    if !(path_buf.is_file() && path_buf.exists()) {
        return Err(anyhow!("File {0} not found", path_buf.display()));
    }

    let size = path_buf.metadata()?.len();
    let chunk_size = if size > BUFFER_SIZE * S3_PARTS_LIMIT {
        size.div_ceil(S3_PARTS_LIMIT)
    } else {
        BUFFER_SIZE
    };
    let chunk_count = size.div_ceil(chunk_size);

    let uploads_resp = client
        .list_multipart_uploads()
        .bucket(bucket)
        .send()
        .await
        .map_err(|err| anyhow!("{}", err.message().unwrap_or("")))?;

    let upload_id = if let Some(uploads) = uploads_resp.uploads
        && let Some(upload) = uploads.iter().find(|u| u.key() == Some(key))
    {
        upload
            .upload_id()
            .ok_or(anyhow!("Found upload without ID"))?
            .to_string()
    } else {
        create_new_upload(client, bucket, key).await?
    };

    // Get already uploaded parts if resuming
    let uploaded_parts = get_uploaded_parts(client, bucket, key, &upload_id).await?;
    let mut upload_parts: Vec<CompletedPart> = uploaded_parts.clone();

    tx.send(S3Update::UploadProgress(FileUpdate::new(
        bucket.to_string(),
        key.to_string(),
        Ok(FileProgress::new(
            chunk_size * uploaded_parts.len() as u64,
            size,
        )),
    )))?;

    for chunk_index in 0..chunk_count {
        let part_number = (chunk_index as i32) + 1;

        // Skip if already uploaded
        if uploaded_parts
            .iter()
            .any(|p| p.part_number() == Some(part_number))
        {
            continue;
        }

        let offset = chunk_index * chunk_size;
        let curr_chunk_size = std::cmp::min(chunk_size, size - offset);

        let stream = ByteStream::read_from()
            .path(&path_buf)
            .offset(offset)
            .length(Length::Exact(curr_chunk_size))
            .build()
            .await?;

        let upload_part_res = client
            .upload_part()
            .key(key.to_string())
            .bucket(bucket)
            .upload_id(&upload_id)
            .body(stream)
            .part_number(part_number)
            .send()
            .await?;

        upload_parts.push(
            CompletedPart::builder()
                .e_tag(upload_part_res.e_tag.unwrap_or_default())
                .part_number(part_number)
                .build(),
        );

        tx.send(S3Update::UploadProgress(FileUpdate::new(
            bucket.to_string(),
            key.to_string(),
            Ok(FileProgress::new(chunk_size * part_number as u64, size)),
        )))?;
    }

    // Complete the multipart upload
    let completed_upload = CompletedMultipartUpload::builder()
        .set_parts(Some(upload_parts))
        .build();

    client
        .complete_multipart_upload()
        .bucket(bucket)
        .key(key.to_string())
        .upload_id(&upload_id)
        .multipart_upload(completed_upload)
        .send()
        .await?;

    Ok(())
}

pub async fn get_object_info(
    client: &Client,
    bucket: &str,
    key: &str,
) -> anyhow::Result<HeadObjectOutput> {
    Ok(client.head_object().bucket(bucket).key(key).send().await?)
}

pub async fn create_bucket(client: &Client, bucket: &str) -> anyhow::Result<()> {
    client
        .create_bucket()
        .bucket(bucket)
        .send()
        .await
        .map_err(|err| anyhow!("Could not create bucket: {}", err))?;
    Ok(())
}

pub async fn create_folder(client: &Client, bucket: &str, key: &str) -> anyhow::Result<()> {
    client
        .put_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await
        .map_err(|err| anyhow!("Could not create folder: {}", err))?;
    Ok(())
}

pub async fn delete_multipart_upload(
    client: &Client,
    bucket: String,
    key: String,
    upload_id: String,
) -> anyhow::Result<()> {
    client
        .abort_multipart_upload()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .send()
        .await
        .map_err(|err| anyhow!("{}", err))?;
    Ok(())
}

async fn create_new_upload(client: &Client, bucket: &str, key: &str) -> anyhow::Result<String> {
    client
        .create_multipart_upload()
        .bucket(bucket)
        .key(key)
        .send()
        .await?
        .upload_id()
        .ok_or(anyhow!("Could not create multipart upload"))
        .map(|id| id.to_string())
}

async fn get_uploaded_parts(
    client: &Client,
    bucket: &str,
    key: &str,
    upload_id: &str,
) -> anyhow::Result<Vec<CompletedPart>> {
    let parts_resp = client
        .list_parts()
        .bucket(bucket)
        .key(key)
        .upload_id(upload_id)
        .send()
        .await?;

    Ok(parts_resp
        .parts()
        .iter()
        .map(|p| {
            CompletedPart::builder()
                .e_tag(p.e_tag().unwrap_or_default())
                .part_number(p.part_number().unwrap_or_default())
                .build()
        })
        .collect())
}
