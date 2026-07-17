# 🦀 Proxy Pool

[![Rust](https://img.shields.io/badge/Rust-1.89%2B-orange?logo=rust)](https://rust-lang.org)
[![License](https://img.shields.io/badge/License-MIT-green)](LICENSE)
[![Docker](https://img.shields.io/badge/Docker-ghcr.io-blue?logo=docker)](https://github.com/fivif/proxy-pool/pkgs/container/proxy-pool)

高性能异步代理池 — 自动拉取、测活、评分、淘汰，永远保持一个**活的**代理池。

## ✨ 特性

- 🔍 **10个公开代理源** 自动拉取（每5分钟），支持上游代理
- 🩺 **高并发测活**（256并发），真实验证 + 内容校验
- 📊 **智能评分**：成功率(40%) + 匿名度(15%) + 延迟(25%) + 新鲜度(20%)，EMA平滑
- ❄️ **冷却+淘汰**：连续3次失败→冷却2分钟→淘汰；10次内成功率<5%→直接清除
- 💾 **崩溃恢复**：每60s快照持久化，重启秒恢复
- 🌐 **RESTful API**：加权随机取代理、全量列表、统计面板

## 🚀 快速部署

### Docker（推荐）

```bash
# 直接拉镜像
docker run -d \
  --name proxy-pool \
  -p 3000:3000 \
  -e PROXY_UPSTREAM_PROXY=http://your-proxy:1080 \
  ghcr.io/fivif/proxy-pool:latest

# 或 docker-compose
curl -O https://raw.githubusercontent.com/fivif/proxy-pool/master/docker-compose.yml
docker compose up -d
```

> 🌍 国内部署建议设置 `PROXY_UPSTREAM_PROXY` 指向你的代理（如 clash 的 10808 端口），否则无法拉取 GitHub 上的代理源。

### 手动编译

```bash
git clone https://github.com/fivif/proxy-pool.git
cd proxy-pool
cargo build --release
./target/release/proxy-pool
```

## 📡 API

| 端点 | 说明 |
|------|------|
| `GET /` | 服务信息 + 仪表盘直链 |
| `GET /api/proxies?limit=100` | 活跃代理列表（按分数降序）|
| `GET /api/proxy/random` | 加权随机取一个优质代理 |
| `GET /api/stats` | 池统计（总数/活跃/冷却/均分）|
| `GET /health` | 健康检查 |

```bash
# 拿一个随机代理
curl http://localhost:3000/api/proxy/random

# 看池子状态
curl http://localhost:3000/api/stats
```

## ⚙️ 环境变量

| 变量 | 默认值 | 说明 |
|------|--------|------|
| `RUST_LOG` | `info` | 日志级别 |
| `PROXY_MAX_POOL` | `5000` | 最大池容量 |
| `PROXY_CHECK_INTERVAL` | `60` | 测活间隔（秒）|
| `PROXY_FETCH_INTERVAL` | `300` | 拉源间隔（秒）|
| `PROXY_VALIDATION_CONCURRENCY` | `256` | 校验并发数 |
| `PROXY_VALIDATION_TIMEOUT` | `8` | 校验超时（秒）|
| `PROXY_UPSTREAM_PROXY` | 空 | 拉源上游代理，如 `http://127.0.0.1:10808` |

## 🏗️ 架构

```
启动 → 拉源(走上游代理) → 入库5000 → 测活 → 死代理冷却2分钟 → 淘汰 → 拉新 → 循环
         │                                                    │
         └──────────── 每300秒 ────────────────────────────────┘
                              每60秒测活 │ 120秒冷却TTL
```

```
src/
├── config.rs          # 全局配置
├── main.rs            # 入口：后台任务 + API
├── pool/
│   ├── proxy.rs       # 代理数据结构（原子计数）
│   ├── cooldown.rs    # 冷却池（TTL淘汰）
│   ├── scorer.rs      # 多维度智能评分
│   └── mod.rs         # 池核心（DashMap 并发哈希表）
├── checker/mod.rs     # 异步测活（多URL容灾）
├── fetcher/
│   ├── sources.rs     # 10个公开代理源 + 解析器
│   └── mod.rs         # 多源并发拉取
├── api/mod.rs         # Axum HTTP API
└── storage/mod.rs     # JSON快照持久化
```

## 📦 构建 Docker 镜像

```bash
docker build -t proxy-pool .
# 或交叉编译 arm64
docker buildx build --platform linux/amd64,linux/arm64 -t proxy-pool .
```

## 📄 License

MIT © [ZAY](https://github.com/fivif)
