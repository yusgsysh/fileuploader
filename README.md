# FileUploader

自托管的流式文件下载 → S3 / R2 上传管理系统。输入 HTTP/HTTPS 或 MEGA 分享链接，后端自动识别来源，把数据以有界流水线写入任意 S3 兼容对象存储。

> **核心原则**  
> HTTP 使用零落盘流式传输；MEGA 使用官方 `mega-get` 下载到隔离临时目录；二者共用同一个低内存、有界并发的 S3 Multipart Upload 引擎。

---

## 功能

- **HTTP/HTTPS 真流式**：Reqwest 响应流 → 固定 Part Buffer → 有界 Channel → S3 UploadPart，不把整个响应读入内存，也不先保存到本地。
- **MEGA 官方工具**：调用官方 MEGAcmd 的 `mega-get`，不使用非官方实现、WebDAV、FTP 或自研 MEGA 协议。
- **S3 Multipart**：Create / UploadPart / Complete / Abort，Part 级重试，分片并发可配置，完成后 HeadObject 校验对象大小。
- **S3 兼容存储**：Cloudflare R2、AWS S3、MinIO、Backblaze B2 S3 API、Wasabi 等。
- **有界背压**：下载与上传之间使用有界队列；分片大小、待上传 Part 数、上传并发均有限制。
- **断点续传**：HTTP 流中断后从已接收字节位置发起 Range 请求；服务器不支持 Range 时明确报错，不伪造进度。
- **MEGA 临时目录**：每任务独立 `/app/tmp/mega/<task-id>/`，成功后自动删除；上传失败保留以便重试。
- **任务系统**：SQLite 持久化、全局/来源并发限制、取消、重试、历史记录、重启后中断标记。
- **实时进度**：SSE 推送下载/上传字节、速度、百分比、ETA、阶段、Part 状态、MEGA 临时目录信息。
- **Web UI**：React + TypeScript + Vite + Tailwind，包含新建任务、任务列表、任务详情、设置。
- **安全**：SSRF 防护（重定向逐跳检查、DNS 解析时过滤、禁止 localhost/私有网段/云元数据）、路径穿越防护、S3 Secret 只写不读、MEGA 分享密钥不写入日志。
- **Docker**：多阶段构建、官方 MEGAcmd 包、非 root、持久化 SQLite 与 MEGA 会话、健康检查、资源限制。

---

## 架构

```text
HTTP / HTTPS
   │  reqwest bytes_stream
   ▼
固定 Part Buffer ──► 有界 mpsc ──► N 个 UploadPart Worker ──► S3 Multipart
                                                        │
MEGA 分享链接                                            ├─ CompleteMultipartUpload
   │  mega-get（官方 MEGAcmd）                            └─ HeadObject 大小校验
   ▼
/app/tmp/mega/<task-id>/
   │  tokio::fs::File + 固定 Buffer
   ▼
同一个有界 S3 UploadPart 引擎
```

### 后端模块

| 路径 | 作用 |
|---|---|
| `src/services/http.rs` | SSRF 安全请求、重定向、Range 恢复、流式分片 |
| `src/services/mega.rs` | MEGAcmd 检测/启动、`mega-get` 进程组管理、目录扫描、磁盘限制 |
| `src/services/s3.rs` | S3 客户端、Multipart 上传、重试、Abort、大小校验 |
| `src/services/task_manager.rs` | 调度、并发门、取消、重试、临时文件清理、任务流水线 |
| `src/progress.rs` | 原子进度、速度/ETA、SSE 事件、周期持久化 |
| `src/security.rs` | SSRF/DNS 过滤、文件名校验、对象 Key 规范化、MEGA 链接校验 |
| `src/api/` | REST + SSE |
| `frontend/` | React 管理界面 |

### 任务阶段

`pending → preparing_upload / downloading → uploading → finalizing → completed`

失败为 `failed`，取消为 `cancelled`。任务状态与阶段分别持久化。

---

## 快速开始（Docker Compose）

前置条件：Docker Engine 24+、Docker Compose v2。

```bash
cp .env.example .env
chmod 600 .env
# 至少填写 S3_ACCESS_KEY / S3_SECRET_KEY / S3_BUCKET，
# 或者先留空，启动后在 Settings 页面添加。
docker compose up -d --build
```

打开 `http://localhost:8080`。

健康检查：

```bash
curl http://localhost:8080/healthz
docker compose ps
docker compose logs -f fileuploader
```

数据卷：

- `fileuploader_data` → `/app/data`：SQLite、MEGAcmd HOME/会话、服务日志。
- `fileuploader_mega_tmp` → `/app/tmp/mega`：MEGA 任务临时文件。

停止：

```bash
docker compose down
# 连同临时文件/数据一起删除（谨慎）：
# docker compose down -v
```

---

## Cloudflare R2 配置

1. 在 Cloudflare Dashboard 创建 R2 Bucket。
2. 创建 **R2 API Token**（Object Read & Write），记录 Access Key ID 和 Secret Access Key。
3. 在 Web UI → **Settings → S3 & R2** 点击 **Cloudflare R2**：
   - **Endpoint**：`https://<ACCOUNT_ID>.r2.cloudflarestorage.com`
   - **Region**：`auto`
   - **Path-style URL**：开启
   - **TLS**：开启
   - **Bucket**：目标 Bucket
   - **Access Key ID / Secret Access Key**
4. 点击 **Test**；成功后保存。
5. 新建任务时选择这个配置，Bucket 可填写任意该 Token 有权限的 Bucket。

也可以首次启动时通过环境变量注入（只需成功执行一次，之后以数据库/UI 为准）：

```dotenv
S3_CONFIG_ID=default
S3_CONFIG_NAME=Cloudflare R2
S3_ENDPOINT=https://<ACCOUNT_ID>.r2.cloudflarestorage.com
S3_REGION=auto
S3_ACCESS_KEY=...
S3_SECRET_KEY=...
S3_BUCKET=downloads
S3_PATH_STYLE=true
S3_USE_TLS=true
```

> R2 的 `r2.dev` 公网开发地址不是 S3 API Endpoint，请使用 `*.r2.cloudflarestorage.com`。

### 其他 S3 示例

**MinIO**

```text
Endpoint: https://minio.example.com
Region: us-east-1
Path style: true
TLS: true
```

**AWS S3**

```text
Endpoint: (留空)
Region: us-east-1
Path style: false
TLS: true
```

**Backblaze B2 S3 API**

```text
Endpoint: https://s3.<region>.backblazeb2.com
Region: <region>
Path style: false
```

---

## MEGA / MEGAcmd

### 使用的命令

后端执行的是：

```text
mega-get --ignore-quota-warn <share-link> <task-directory>
```

参数与当前官方 MEGAcmd 2.x 的 `get [-m] [-q] [--ignore-quota-warn] exportedlink [localpath]` 形式一致。命令通过 `tokio::process::Command` 以参数数组传递，**不经过 Shell**，URL 与路径不会被拼接进命令字符串。

### 公开分享链接

公开文件/文件夹链接通常**无需登录**即可下载。系统默认尝试直接调用 `mega-get`。

支持：

- `https://mega.nz/file/<id>#<key>`
- `https://mega.nz/folder/<id>#<key>`
- `https://mega.nz/#!<id>!<key>`
- `https://mega.nz/#F!<id>!<key>`
- 当前 MEGAcmd 支持的同类 `mega.nz` 分享格式

分享密钥完整保留在 URL 中，但不会写入应用日志；mega-get 的输出在保存为任务错误前会做链接/密钥脱敏。

### 登录（可选）

受保护内容或账号内路径需要 MEGAcmd 会话。会话保存在 MEGAcmd HOME（默认 `/app/data/mega-home`）的 `.megaCmd` 目录，Docker 卷会持久化。

```bash
docker compose exec -u app \
  -e HOME=/app/data/mega-home \
  fileuploader mega-login 'you@example.com' 'password'
```

应用本身**不接收、不保存 MEGA 密码**。不要在 `docker-compose.yml`、Dockerfile 或 `.env` 中硬编码 MEGA 密码。

检查：

```bash
docker compose exec -u app -e HOME=/app/data/mega-home fileuploader mega-whoami
```

退出：

```bash
docker compose exec -u app -e HOME=/app/data/mega-home fileuploader mega-logout
```

### MEGAcmd Server 生命周期

- 后端在任务开始前检查 `mega-whoami`；若 server 不可用且 `MEGA_AUTO_START_SERVER=true`，会以非 root 用户、指定 HOME 启动 `mega-cmd-server`。
- Server 的 stdout/stderr 写入 `$MEGA_HOME/server.log`。
- 容器重启后，应用会重新启动 server；`.megaCmd` 会话从数据卷恢复，通常无需重新登录。
- 未使用 Supervisor：单容器中由应用管理 server，避免多进程管理器的额外复杂度；如需完全外部管理，设置 `MEGA_AUTO_START_SERVER=false` 并在自定义 entrypoint/独立容器中启动 server。
- 取消 MEGA 任务时，`mega-get` 以独立进程组启动，取消会向整个进程组发送 `SIGKILL`，并等待回收，避免子进程残留。

### 临时目录

```text
/app/tmp/mega/<task-id>/
  .fileuploader-complete
  ...下载文件/目录...
```

- 任务之间完全隔离。
- 下载完成后写 marker，再进入上传；重试上传时可复用已下载文件，无需重新下载。
- 上传成功：自动删除目录并清空数据库临时字段。
- 上传失败/取消上传：保留目录，允许重试。
- 下载失败/取消下载：删除未完成目录。
- 启动时扫描残留目录；超过 `MEGA_RETENTION_SECS` 且不属于运行中任务的目录会被安全删除。

---

## 本地开发

### 后端

```bash
cp .env.example .env
mkdir -p data tmp/mega
# 可选：修改 DATABASE_URL、STATIC_DIR 或 S3 初始配置
cargo run
```

默认 `http://127.0.0.1:8080`，同时提供 `/api/*` 与 `frontend/dist` 静态文件。

### 前端

```bash
cd frontend
npm install
npm run dev
```

Vite 在 `5173` 启动，`/api` 和 `/healthz` 代理到 `8080`。

生产构建：

```bash
cd frontend && npm run build
cd .. && cargo build --release
```

### 校验

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cd frontend && npm run build
```

---

## 本地 Podman 构建

仓库同时提供 `Containerfile`（Podman/Buildah 约定）和 `Dockerfile`（GitHub Actions 使用），两者内容保持一致。

```bash
# 推荐：使用构建脚本
./scripts/podman-build.sh

# 等价命令
podman build --layers -f Containerfile -t fileuploader:local .

# 不缓存重建
./scripts/podman-build.sh --no-cache

# 构建完成后运行
podman run --rm --name fileuploader \
  -p 8080:8080 \
  --env-file .env \
  -v fileuploader-data:/app/data \
  -v fileuploader-tmp:/app/tmp/mega \
  fileuploader:local
```

本地命名卷：

```bash
podman volume create fileuploader-data
podman volume create fileuploader-tmp
```

> Podman 使用 `.containerignore`；Docker/GitHub Actions 使用 `.dockerignore`。两者都排除了 `target/`、`node_modules/`、数据库和 `.env`。
> 本地构建默认使用当前 CPU 架构；需要 arm64 时请使用原生 arm64 主机或 Podman 的跨架构配置。

---

## 配置

所有传输参数都可在 Web UI → Settings 修改。环境变量仅用于**首次启动**写入默认设置；之后以 SQLite 中的设置为准。

### 环境变量

| 变量 | 默认值 | 说明 |
|---|---:|---|
| `APP_HOST` / `APP_PORT` | `0.0.0.0` / `8080` | 监听地址 |
| `DATA_DIR` | `./data` | 数据目录 |
| `DATABASE_URL` | `sqlite://<DATA_DIR>/fileuploader.db` | SQLite |
| `STATIC_DIR` | `./frontend/dist` | 前端构建产物 |
| `LOG_FORMAT` | `pretty` | `pretty` / `json` |
| `SEED_DEFAULT_S3` | `true` | 是否尝试从环境变量写入首个 S3 配置 |
| `S3_*` | — | 首次启动 S3 配置与环境凭据 |
| `MEGA_HOME` | `./data/mega-home` | MEGAcmd HOME |
| `MEGA_TEMP_DIR` | `./tmp/mega` | 临时目录根 |
| `MEGA_AUTO_START_SERVER` | `true` | 是否由应用启动 server |
| `PART_SIZE_BYTES` | `16777216` | 默认 16 MiB |
| `UPLOAD_CONCURRENCY` | `2` | 每任务 UploadPart 并发 |
| `HTTP_BUFFER_BYTES` | `1048576` | HTTP 处理缓冲窗口 |
| `MAX_PENDING_PARTS` | `2` | 下载/上传之间有界队列容量 |
| `MAX_HTTP_TASKS` | `3` | HTTP 并发任务 |
| `MAX_MEGA_TASKS` | `1` | MEGA 并发任务 |
| `MAX_QUEUED_TASKS` | `50` | 队列上限 |
| `MAX_RETRIES` | `3` | 网络/分片重试 |
| `HTTP_TIMEOUT_SECS` | `30` | 连接/流空闲超时 |
| `MAX_REDIRECTS` | `10` | HTTP 重定向上限 |
| `MAX_FILE_BYTES` | `107374182400` | 100 GiB，0 为不限 |
| `TASK_TIMEOUT_SECS` | `86400` | 单任务最长运行时间 |
| `MEGA_MAX_TASK_BYTES` | `53687091200` | 单任务临时磁盘上限 |
| `MEGA_TEMP_MAX_BYTES` | `214748364800` | 总临时目录上限 |
| `MEGA_RETENTION_SECS` | `86400` | 失败临时文件保留时间 |
| `MEGA_MAX_DOWNLOADS` | `1` | MEGA 下载并发上限 |
| `MEGA_DOWNLOAD_TIMEOUT_SECS` | `21600` | mega-get 超时 |

### 运行时设置页面

- **Transfer**：Part Size、上传并发、HTTP Buffer、待上传 Part、HTTP/MEGA 并发、队列、重试、超时、文件大小限制、任务运行时间。
- **MEGA**：`mega-get` / server / whoami 路径、HOME、临时目录、自动启动、磁盘限制、保留时间、超时。
- **S3 & R2**：多配置、Endpoint、Region、Access Key、Secret、Bucket、默认路径、Path Style、TLS、自定义 metadata。
- Secret Key 只写：读取 API 只返回 `secret_set: true/false`。

---

## REST API

```text
POST   /api/tasks
GET    /api/tasks?limit=200
GET    /api/tasks/{id}
POST   /api/tasks/{id}/cancel
POST   /api/tasks/{id}/retry
DELETE /api/tasks/{id}

GET    /api/events                     # SSE

GET    /api/settings
PUT    /api/settings

GET    /api/s3/configs
POST   /api/s3/configs
PUT    /api/s3/configs/{id}
DELETE /api/s3/configs/{id}
GET    /api/s3/buckets?config_id=...
POST   /api/s3/test

GET    /api/system/status
GET    /api/system/mega/status
POST   /api/system/mega/test
GET    /healthz
```

创建任务：

```json
{
  "url": "https://mega.nz/file/abc123#key123",
  "source_type": "auto",
  "s3_config_id": "default",
  "bucket": "downloads",
  "object_key": "archive/",
  "filename": "file.zip",
  "allow_overwrite": false
}
```

可选的下载请求字段：`user_agent`、`referer`、`cookie`、`headers`（对象）、`auth_username`、`auth_password`。

SSE 事件：

- `task`：完整任务快照（包含实时进度、速度、Parts）
- `deleted`：任务 ID
- `resync`：客户端应重新拉取
- `hello` / keep-alive

---

## 安全设计

### SSRF

- 仅允许 `http` / `https`。
- 拒绝 URL 内嵌凭据（使用 Basic Auth 字段）。
- 禁止 localhost、`.localhost`、`.internal`、私有/回环/链路本地/CGNAT/组播/保留地址。
- 显式阻断云元数据地址：
  - `169.254.169.254`
  - `100.100.100.200`
  - `168.63.129.16`
- 每次重定向都重新校验目标并重新解析 DNS。
- 自定义 reqwest DNS Resolver 在**实际连接时**再次过滤地址，关闭 DNS Rebinding 窗口。
- 禁用环境代理，避免绕过解析检查。
- 跨源重定向不携带 Authorization / Cookie。

### 文件与对象

- 文件名清理：去除路径分隔符、控制字符、`.`/`..`，限制长度。
- S3 对象路径拒绝 `..`、绝对路径逃逸。
- MEGA 下载禁用符号链接；目录扫描限定在任务根目录。
- 默认禁止覆盖已有对象。上传前 HeadObject 检查；同一任务的 Part/对象可安全重传（通过 `fileuploader-task-id` metadata 识别）。
- 文件夹下载中若多个文件名映射到同一对象 Key 会直接失败，避免覆盖。
- 可配置单文件大小、单任务临时磁盘、总临时磁盘和任务运行时间。

### 凭据

- S3 Secret Key 不通过 API 返回；更新时留空保留旧值。
- MEGA 密码不由本应用采集或存储；使用 MEGAcmd 自己的加密/会话机制。
- MEGA 分享密钥不写入应用日志；任务错误中的链接和输出会被脱敏。
- SQLite 文件权限 `0600`，数据目录 `0700`。
- 命令不使用 Shell，不拼接用户输入。

### 传输

- 有界通道、有界 Part 数、有界 UploadPart 并发。
- 取消：关闭 HTTP 流/终止 mega-get 进程组/AbortMultipartUpload。
- 失败重试：HTTP 连接、Range 恢复、Part 上传、Complete 均有预算；超过后任务失败。
- 完成前校验字节数，完成后 HeadObject 校验对象大小。
- 启动时标记被中断任务并尽力清理遗留 Multipart Upload 与临时目录。

---

## 测试

单元测试覆盖安全地址过滤、文件名/对象路径、Content-Disposition、MEGA 链接格式、进度解析等：

```bash
cargo test
```

S3 Multipart 集成测试（自动启动 MinIO、创建 Bucket、上传 20 MiB 两分片并校验对象大小）：

```bash
./scripts/run-s3-integration.sh
```

该测试在未设置 `TEST_S3_*` 环境变量时会自动跳过，因此普通 `cargo test` 不需要额外服务。MEGA 与完整 HTTP 端到端测试仍按下方手工验收清单执行。

### 本次开发已实际验证

- HTTP 1 MiB / 40 MiB 经 Moto S3 上传成功，对象大小与 metadata 校验通过。
- 40 MiB 文件产生 3 个 Part；取消上传后 AbortMultipartUpload 成功，列表无残留。
- 同一对象 Key 的并发任务被串行化，第二个任务明确失败而不是静默覆盖。
- GitHub 302 重定向到 `raw.githubusercontent.com` 成功。
- 官方 MEGAcmd 2.7.1：公开文件 339 MiB 分片上传成功；公开文件夹 4 个文件保留嵌套目录结构上传成功。
- MEGA 取消会同时终止 `mega-get` 进程组和 MEGAcmdServer 侧的 transfer tag；临时目录清理，`mega-transfers` 无残留。
- SSE、深链接 SPA 回退、桌面与移动端 UI 均通过浏览器自动化检查，无控制台错误。

### MinIO 手工集成验收

```bash
docker run -d --name minio-test \
  -p 9000:9000 -p 9001:9001 \
  -e MINIO_ROOT_USER=minioadmin \
  -e MINIO_ROOT_PASSWORD=minioadmin \
  -v /tmp/minio-test:/data \
  minio/minio server /data --console-address ":9001"
```

在 UI 中创建 S3 配置：

```text
Endpoint: http://127.0.0.1:9000
Region: us-east-1
Path style: true
TLS: off
Access Key: minioadmin
Secret Key: minioadmin
Bucket: 先在 http://127.0.0.1:9001 创建 downloads
```

验收点：

1. 小文件 HTTP 下载 → `mc stat` 校验大小。
2. 大文件（> 50 MiB）观察容器内存不随文件线性增长。
3. 未知 Content-Length 的 chunked 源：只有已传字节与速度，无假百分比。
4. 重定向、Range 恢复、404、网络中断。
5. SSRF：`http://127.0.0.1`、`http://169.254.169.254` 必须创建失败。
6. MEGA 公开文件、文件夹、带 key 链接；无效链接；取消后 `ps` 无 mega-get 残留。
7. 上传中取消：S3 控制台无未完成 Multipart（或 Abort 成功）。
8. 上传失败后重试：MEGA 临时目录复用；HTTP 从 Range 续传或明确报“不支持断点续传”。
9. 重启容器：运行中任务标记为失败，可手动重试；遗留临时目录按保留策略清理。

### 内存压力测试建议

```bash
# 连续创建多个大文件任务，观察 RSS 与临时磁盘
docker stats fileuploader
```

默认 16 MiB Part × (2 待上传 + 2 上传并发 + SDK/HTTP 缓冲) 时，流式部分通常只占几十 MiB；实际峰值受 TLS/AWS SDK 和并发任务数影响。生产环境请以自己的文件规模压测后调整 Part Size/并发。

---

## GitHub Actions 发布公共镜像（GHCR）

仓库包含 [`.github/workflows/docker-publish.yml`](.github/workflows/docker-publish.yml)：

- push 到 `main` 或推送 `v*.*.*` tag 时自动构建并发布。
- `main` 分支和 PR 默认只构建 `linux/amd64`，速度优先；PR 不推送。
- `v*.*.*` tag 自动构建 `linux/amd64` + `linux/arm64` 多架构镜像。
- `workflow_dispatch` 可手动选择是否同时构建 arm64。
- Rust 依赖层独立缓存，源码变更时不再重新编译全部依赖。
- Release profile 关闭 LTO、使用 16 个 codegen units，缩短 CI 编译时间。
- 同一分支新推送会取消旧构建，避免排队浪费。
- 使用 GitHub 内置 `GITHUB_TOKEN`（`packages: write`）登录 `ghcr.io`。
- 使用 `gh api` 尝试把新建 GHCR 包切换为 public；无权限时只发出 warning，不阻断发布。
- 生成 BuildKit SBOM 与 max provenance，使用 GitHub Actions 缓存。

镜像地址：

```text
ghcr.io/<github-owner>/<github-repository>:latest
```

常用命令：

```bash
# 首次创建公开 GitHub 仓库并推送（使用 GitHub CLI）
gh repo create yusgsysh/fileuploader --public --source=. --remote=origin --push

# 手动触发镜像构建（仅 amd64，最快）
gh workflow run "Publish container image" --ref main -f multiarch=false

# 手动构建多架构镜像
gh workflow run "Publish container image" --ref main -f multiarch=true

# 查看运行
gh run list --workflow "Publish container image"

# 拉取公共镜像（包为 public 时无需登录）
docker pull ghcr.io/yusgsysh/fileuploader:latest
```

### 首次发布后让包公开

GHCR 新建包默认可能是 private，即使源码仓库是 public。工作流会自动调用 `gh api` 尝试切换，但 GitHub 对部分组织/包命名空间不开放该 API 权限。

如果工作流日志出现 `Could not set the package to public automatically`：

1. 打开 `https://github.com/users/<owner>/packages/container/<repo>/settings`
2. **Danger Zone → Change visibility → Public**
3. 后续推送会保持公开，无需重复操作。

或在仓库 Secrets 中添加 `GHCR_ADMIN_TOKEN`：

- Classic PAT，至少包含 `write:packages`；组织包通常还需要 `admin:org` 或对应包管理权限。
- 工作流会优先使用该 secret 执行 `gh api` 可见性切换。

> 发布镜像不需要 PAT；只有自动修改包可见性可能需要。

### 构建速度建议

1. **日常 push 保持 `linux/amd64` 单架构**：arm64 在 QEMU 下模拟编译 Rust 依赖，通常是主要耗时来源。
2. **只在正式 tag 构建多架构**：`v1.2.3` 这类 tag 自动启用 arm64；日常 main 使用 amd64。
3. **依赖层缓存**：Dockerfile 先构建占位 crate，再复制真实源码。`Cargo.toml` / `Cargo.lock` 不变时，GitHub Actions 会命中 Rust 依赖缓存。
4. **关闭 LTO**：本项目以网络 I/O 为主，`lto = "thin"` 对运行收益很小，却显著延长链接时间。
5. **保留 GHA 构建缓存**：`cache-from/to: type=gha,mode=max` 缓存前端、apt、Rust 依赖与 Docker 层。
6. **本地开发优先 `cargo check`**；需要二进制时用 `cargo build`，只在部署前执行 `cargo build --release`。
7. 如果必须频繁发布 arm64，建议后续使用 GitHub 原生 `ubuntu-24.04-arm` runner 做矩阵构建，再合并 manifest，避免 QEMU。

---

## 资源与限制

- S3 Multipart：最多 10,000 Parts，Part Size 5 MiB–5 GiB，对象最大 5 TiB。系统会在 Part 数超限时失败并提示增大 Part Size。
- 部分 S3 兼容服务对 checksum 扩展支持不一致；客户端配置为仅在必需时计算/校验 checksum。
- 对象大小校验依赖 HeadObject 权限。
- “禁止覆盖”会进行一次 HeadObject；若凭据没有 HeadObject 权限，上传会失败。
- 浏览器端文件大小/速度显示来自服务端实时快照；刷新页面不会保留历史速度曲线（曲线由页面会话采集）。
- MEGA 临时目录位于独立卷；请确保 `MEGA_TEMP_MAX_BYTES` 与卷容量匹配。
- 修改 `MEGAcmd HOME` 后需要重启容器/应用，以便 server 与新 HOME 使用同一会话目录。
- MEGA 分享链接检查接口验证主机、路径格式和密钥完整性；由于官方 MEGAcmd 没有无下载的公开链接探测命令，远程文件是否存在会在任务真正开始时确认。
- 本程序不自带用户认证。请部署在可信网络，或通过反向代理（Caddy/Nginx/Authelia 等）增加身份认证与 TLS。

---

## 故障排查

### S3 连接失败

- 检查 Endpoint 是否包含 scheme（`https://`）。
- R2：Region 必须是 `auto`，Path style 开启。
- MinIO：使用 `http://` + TLS off，或正确 CA 的 `https://`。
- 测试按钮会调用 `ListBuckets`；若凭据只有对象权限，可能无法列出，但能否上传取决于 Head/Get/Put 权限。
- 检查系统时间；SigV4 对时钟偏差敏感。

### MEGA 任务失败

```bash
docker compose exec -u app -e HOME=/app/data/mega-home fileuploader mega-get --version
docker compose exec -u app -e HOME=/app/data/mega-home fileuploader mega-whoami
docker compose exec -u app cat /app/data/mega-home/server.log
```

- 公开链接仍失败：在同目录手工执行 `mega-get --ignore-quota-warn '<link>' /tmp/test` 验证链接。
- 受保护链接：用 `mega-login` 建立会话。
- 磁盘满/限额：检查 `/app/tmp/mega`、`MEGA_MAX_TASK_BYTES`、`MEGA_TEMP_MAX_BYTES`。
- server 未启动：确认 `MEGA_AUTO_START_SERVER=true`，或外部启动 `mega-cmd-server`。

### HTTP 无法断点续传

源站未返回 `Accept-Ranges: bytes`，或对 Range 请求返回 `200` 而非 `206`。系统会保留已下载的 Part 上下文并做过有限重试，最终明确报告无法续传；不会把部分内容标记为完成。

---

## 项目结构

```text
.
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── .env.example
├── src/
│   ├── api/
│   ├── services/
│   ├── main.rs
│   ├── config.rs
│   ├── db.rs
│   ├── models.rs
│   ├── progress.rs
│   ├── security.rs
│   └── state.rs
├── frontend/
│   ├── src/
│   ├── package.json
│   └── vite.config.ts
└── README.md
```

---

## License

MIT
