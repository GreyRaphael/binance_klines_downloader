# Binance Klines Downloader

从 Binance 下载交割合约 K 线数据，保存为 IPC 格式（Polars / Pandas 可直接读取）。

## 配置

`config.toml` 已加入 `.gitignore`，不会被提交到仓库。

复制 `config.toml.example` 为 `config.toml`，按需修改：

| 字段 | 说明 |
|------|------|
| `proxy` | HTTP 代理地址 |
| `output_dir` | 数据输出目录 |
| `intervals` | K 线间隔，如 `["5m", "15m", "30m", "1h"]` |
| `symbols` | 交易对列表 |
| `gotify_url` | Gotify 服务器地址（为空则不推送） |
| `gotify_token` | Gotify 应用 Token（与 url 同时非空才推送） |
| `ntfy_url` | Ntfy 主题地址，如 `https://ntfy.sh/mytopic`（为空则不推送） |
| `ntfy_token` | Ntfy 访问令牌（可选，公开主题留空） |
| `log_dir` | 日志文件输出目录（为空则不写文件，按天轮转） |

## 用法

### 每日调度（推荐 UTC 13:15 运行）

```bash
binance_klines_downloader daily-scheduler
```

### 每月调度（推荐每月 3 日 UTC 00:00 运行）

```bash
binance_klines_downloader monthly-scheduler
```

### 手动下载

```bash
binance_klines_downloader download -f daily -i 1h -d 2026-06-25
binance_klines_downloader download -f monthly -i 1h -d 2026-05
```

### 回填历史数据

```bash
binance_klines_downloader backfill -s 2022-10-03 -e 2026-06-25 -i 1h
```

完整月份下载为月度数据，最后不完整月份按天下载。最大并发 16。

## 构建

```bash
cargo build --release
```

## 通知

支持 [Gotify](https://gotify.net/) 和 [Ntfy](https://ntfy.sh/) 推送通知。

- 推送级别：`INFO` 及以上（包含 `INFO` / `WARN` / `ERROR`）
- 任一服务的 URL 为空则自动跳过该服务

## Release

通过 GitHub Actions 自动构建多平台二进制。

每次发布时，`config.toml.example` 会随二进制一同打包，方便用户直接使用。
