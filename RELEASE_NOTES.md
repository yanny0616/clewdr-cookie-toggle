# v0.12.27

## 本次更新
- Claude Code 客户端标识升级至 `2.1.280`，修复新模型要求更高客户端版本的错误。
- 模型列表新增 `claude-opus-5-5`（Opus 5.5）。
- 包含此前已部署的请求超时、终态日志、缓存用量显示和日志界面改进。
- 修正 Windows 安装包的 `.exe` 文件打包路径。

## 下载说明
- Windows：选择 `clewdr-windows-x86_64.zip`；ARM 设备选择 `aarch64`。
- macOS：Apple Silicon 选择 `clewdr-macos-aarch64.zip`；Intel 选择 `x86_64`。
- Linux：按架构选择 `linux` 包；需要 musl 静态构建时选择 `musllinux`。
- Android：`clewdr-android-aarch64.zip` 是命令行程序及运行库，不是 APK。
- 升级前备份现有 `clewdr.toml`，不要用空配置覆盖现有配置。

---

# Earlier release notes

## Security

- **Distrust compromised emSign certificate chains.** Update the bundled CA
  certificates (`webpki-root-certs` 1.0.9 → 1.0.10), removing
  `emSign ECC Root CA - G3`. This responds to the disclosure of intermediate
  CA private keys belonging to India's eMudhra/emSign and prevents ClewdR's
  bundled trust store from accepting chains through those intermediates.
  The update protects upstream HTTPS connections against public-network
  man-in-the-middle tools using the disclosed keys to forge certificates.
- **Upgrade recommended.** Install v0.13.6 and restart ClewdR; container users
  should pull the updated image and recreate their containers. Updating the
  operating system's CA bundle alone does not update the roots embedded in
  existing ClewdR binaries.

References: [RUSTSEC-2026-0383 / CVE-2026-111943](https://rustsec.org/advisories/RUSTSEC-2026-0383.html),
[upstream root removal](https://github.com/rustls/webpki-roots/pull/133).

## 安全更新

- 因印度 CA 机构 eMudhra/emSign 的中间 CA 私钥泄露，将内置 CA 包
  `webpki-root-certs` 从 1.0.9 更新至 1.0.10，移除
  `emSign ECC Root CA - G3` 信任根，防范公网中间人工具利用泄露密钥
  伪造证书、拦截 ClewdR 的上游 HTTPS 通信。
- 建议升级到 v0.13.6 并重启；容器用户请拉取新镜像并重建容器。
  仅更新操作系统 CA 包不会更新已有 ClewdR 二进制内嵌的信任根。
