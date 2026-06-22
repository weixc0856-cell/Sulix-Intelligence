/// 全局 HTTP Client 单例
///
/// 使用 OnceLock 确保 Client 只初始化一次，复用连接池。
/// 所有模块应通过此函数获取 Client，而非各自创建。
///
/// 获取带默认超时（30秒）的全局 HTTP Client
#[allow(dead_code)]
pub fn global_client() -> &'static reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .user_agent("SulixIntel/2.0 (Global Pipeline)")
            .build()
            .expect("failed to build global HTTP client")
    })
}

/// 创建带自定义超时的 HTTP Client
#[allow(dead_code)]
pub fn http_client_with_timeout(timeout_secs: u64) -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .user_agent("SulixIntel/2.0 (Global Pipeline)")
        .build()
        .expect("failed to build HTTP client")
}
