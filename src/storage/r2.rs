//! Cloudflare R2 上传客户端
//!
//! 使用 S3 兼容 API（aws-sdk-s3）将内容资产上传到 Cloudflare R2。
//! 凭证通过环境变量注入：R2_ACCESS_KEY_ID / R2_SECRET_ACCESS_KEY / R2_ENDPOINT
//!
//! # 使用
//! ```ignore
//! let r2 = R2Client::from_config(&config).await?;
//! r2.upload_dir(&mdx_path, "daily/", "md").await;
//! r2.upload_json("manifest.json", &data).await?;
//! ```

use anyhow::{Context, Result};
use aws_credential_types::Credentials;
use aws_sdk_s3::config::{BehaviorVersion, Region};
use aws_sdk_s3::primitives::ByteStream;
use aws_sdk_s3::Client;
use std::path::Path;

/// 批量上传结果
pub struct UploadResult {
    pub uploaded: Vec<String>,
    pub failed: Vec<(String, String)>,
}

/// R2 上传客户端
pub struct R2Client {
    client: Client,
    bucket: String,
}

impl R2Client {
    /// 从配置 + 环境变量构建
    ///
    /// 凭证优先级：
    ///   1. R2_ACCESS_KEY_ID / R2_SECRET_ACCESS_KEY（专用）
    ///   2. AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY（兼容 fallback）
    pub async fn from_config(config: &crate::config::R2Config) -> Result<Self> {
        let access_key = std::env::var("R2_ACCESS_KEY_ID")
            .or_else(|_| std::env::var("AWS_ACCESS_KEY_ID"))
            .context("需要 R2_ACCESS_KEY_ID 或 AWS_ACCESS_KEY_ID")?;
        let secret_key = std::env::var("R2_SECRET_ACCESS_KEY")
            .or_else(|_| std::env::var("AWS_SECRET_ACCESS_KEY"))
            .context("需要 R2_SECRET_ACCESS_KEY 或 AWS_SECRET_ACCESS_KEY")?;
        let endpoint = config.endpoint.clone();

        let creds = Credentials::new(&access_key, &secret_key, None, None, "r2-env");

        let aws_config = aws_sdk_s3::Config::builder()
            .behavior_version(BehaviorVersion::latest())
            .region(Region::new("auto"))
            .endpoint_url(&endpoint)
            .credentials_provider(creds)
            .force_path_style(true)
            .build();

        let client = Client::from_conf(aws_config);

        Ok(Self {
            client,
            bucket: config.bucket.clone(),
        })
    }

    /// 上传单个文件到 R2
    pub async fn upload(&self, key: &str, content: &[u8], content_type: &str) -> Result<()> {
        let body = ByteStream::from(content.to_vec());

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(body)
            .content_type(content_type)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("R2 上传失败 [{}]: {:?}", key, e))?;

        log::info!("☁️ R2 上传: {} ({})", key, content_type);
        Ok(())
    }

    /// 上传 JSON 数据
    pub async fn upload_json(&self, key: &str, content: &[u8]) -> Result<()> {
        self.upload(key, content, "application/json").await
    }

    /// 批量上传目录中的指定扩展名文件
    ///
    /// `local_base`: 本地基础路径（如 output/）
    /// `r2_prefix`: R2 前缀（如 "daily/"）
    /// `ext`: 文件扩展名过滤（如 "md"）
    pub async fn upload_dir(&self, local_base: &Path, r2_prefix: &str, ext: &str) -> UploadResult {
        let mut uploaded = Vec::new();
        let mut failed = Vec::new();

        let dir = local_base.join(r2_prefix);
        if !dir.exists() {
            log::debug!("☁️ R2 upload_dir: {} 不存在，跳过", dir.display());
            return UploadResult { uploaded, failed };
        }

        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) => {
                failed.push((r2_prefix.to_string(), format!("read_dir: {}", e)));
                return UploadResult { uploaded, failed };
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                continue;
            }
            // 过滤扩展名
            if let Some(e) = path.extension() {
                if e != ext {
                    continue;
                }
            } else {
                continue;
            }
            let file_name = match path.file_name().and_then(|n| n.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            let r2_key = format!("{}{}", r2_prefix, file_name);
            let content_type = if ext == "json" {
                "application/json"
            } else {
                "text/markdown; charset=utf-8"
            };

            match std::fs::read(&path) {
                Ok(data) => match self.upload(&r2_key, &data, content_type).await {
                    Ok(_) => uploaded.push(r2_key),
                    Err(e) => failed.push((r2_key, e.to_string())),
                },
                Err(e) => failed.push((r2_key, format!("read: {}", e))),
            }
        }

        UploadResult { uploaded, failed }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upload_result_new() {
        let result = UploadResult {
            uploaded: vec!["daily/test.md".into()],
            failed: vec![],
        };
        assert_eq!(result.uploaded.len(), 1);
        assert!(result.failed.is_empty());
    }
}
