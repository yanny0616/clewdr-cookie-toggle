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

## What's New
- Use native incognito mode (`is_temporary`) for non-preserved chats (PR #145 by @GottenHeave)
- Web usage endpoint for enterprise accounts
- Update TLS emulation to Chrome 145

## Bug Fixes
- Fix `clewdr.toml` file permissions on Unix: now created with `0600` instead of default umask (#122)
- Add fallback for unknown `ContentBlock` types to prevent 422 deserialization errors (#97)
- Fix OAI `ImageUrl` to Claude `Image` format conversion in Claude Code proxy (PR #121 by @DragonFSKY)
- Fix enterprise usage tracking (PR #144 by @GottenHeave)
- Work around a bug in `tower-serve-static` (#147)

## Improvements
- Unify HTTP client construction across codebase
- Always use mimalloc as default allocator
- Unpin `tracing-subscriber`, allow ANSI color output
- Update dependencies to latest versions
- Use distroless Docker image
