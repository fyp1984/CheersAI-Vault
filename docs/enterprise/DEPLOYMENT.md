# CheersAI Vault Pro · 部署与技术说明

面向：项目维护者 / 开发者 / 部署管理员。浏览器端使用说明见 [OPERATION_GUIDE.md](./OPERATION_GUIDE.md)；四项客户 API 说明见 [API_REFERENCE.md](./API_REFERENCE.md)；Linux 交付材料（systemd/Nginx/环境变量/smoke 脚本）见 [`../../deploy/linux/README.md`](../../deploy/linux/README.md)。

> 本文档里大部分命令与检查方式都在本机（macOS，Apple Silicon）实际执行过一次，文中直接给出真实输出；未标注来源的具体数字（版本号、路径、哈希值）均来自这次实测，不是照抄推测。凡本文档未实际验证的步骤，会明确标注「**未验证**」。**第 0 节描述的 Linux 内网客户测试部署，其中标注"隔离 Linux 主机/虚拟机/容器"的步骤本轮未能在真实 Linux 环境验证** —— 本机开发机没有可用的 Linux 虚拟化/容器工具，详见交付 result 文档的头条披露；不得把本文档中的 macOS 结果当作 Linux 验收证据。

## 0. 客户入口：原 Vault 根前端 + Linux 内网部署（本文档的主线）

**面向客户交付时，浏览器端入口是仓库根目录的原 Vault 前端（本仓库根 `vite.config.ts` 构建的 React 应用），不是 `apps/vault-pro-web`。** `apps/vault-pro-web` 仍保留在仓库中，但本轮明确不作为客户入口，也未做任何扩建（老师决策，见 `.codex-local/规则/决策说明.md`）。

```text
浏览器 / 企业内网系统
        ↓
Nginx（内网地址 + 客户 CIDR 白名单，唯一入口）
  ├─ /          → 仓库根目录 dist/（原 Vault 前端，pnpm exec vite build 产物）
  └─ /api/      → 127.0.0.1:8787 Runtime（apps/vault-runtime-api，只监听 loopback）
```

- Runtime 继续只监听 `127.0.0.1`，不提供监听公网/其他网卡的开关。
- Nginx 是唯一的内网入口：托管原 Vault 前端静态资源、反向代理 `/api/`，并用**显式客户 CIDR 白名单**做访问控制（默认只放行 loopback，部署管理员必须自行加入真实客户网段；禁止 `allow all`/`0.0.0.0/0`）。
- **本版本没有应用层鉴权**——没有 API Key、登录、会话、权限体系；Nginx CIDR 白名单是本次隔离测试内网唯一的访问边界，`VAULT_RUNTIME_CORS_ORIGINS` 只影响浏览器同源检查，不能替代它。**这不是正式生产安全部署**，仅适用于隔离测试内网。
- systemd 单元、Nginx 站点模板、Runtime 环境变量模板、黑盒 smoke 测试脚本都在 [`deploy/linux/`](../../deploy/linux/) 目录，配套 [`deploy/linux/README.md`](../../deploy/linux/README.md) 给出完整上手顺序。
- 四项客户可调用的 HTTP API（批量提交、进度查询/失败信息、结果下载、健康检查）的完整字段说明见 [API_REFERENCE.md](./API_REFERENCE.md)；Runtime 实际还有更多接口供原 Vault 前端自身使用，但不在本次客户测试的承诺范围内。
- **FileBay 浏览器上传（单系统用户 MVP）**：`VAULT_FILEBAY_URL`/`_TOKEN`/`_OWNER`/`_REPO` 四项由部署管理员在 Runtime 环境文件配置（启动时读取一次，修改后重启生效）；浏览器 `/gitea` 只显示安全状态并允许显式测试连接、创建固定私有仓库，`/files` 只允许确认上传 `Completed` 脱敏 Markdown。配置与操作详见第 5.2 节。

本文档第 1-9 节沿用此前单机 MVP 阶段积累的 Runtime/OCR/LibreOffice/环境变量/排障说明（大多数内容与部署拓扑无关，继续适用），第 1 节的组件关系图与第 5 节的启动命令已更新为反映"原 Vault 根前端 + Nginx + Runtime"这一客户交付拓扑；仍保留 `apps/vault-pro-web` 的构建方式作为该应用自身的开发说明，但不代表客户部署路径。

## 本版本包含什么

本次提交范围：

- 共享脱敏/解析核心（`src-tauri/crates/engine-core`）与 OCR 组件封装（`src-tauri/crates/component-runtime`）。
- 企业端 Runtime（`apps/vault-runtime-api`）；客户浏览器入口为仓库根目录原 Vault 前端，`apps/vault-pro-web` 保留但不是客户入口。
- 桌面端为兼容共享核心所做的必要适配（`src-tauri/src/`、`src/`）。
- 浏览器 FileBay 上传（单系统用户 MVP）：管理员通过 `VAULT_FILEBAY_URL`/`_TOKEN`/`_OWNER`/`_REPO` 四个环境变量配置固定 HTTPS 目标私有仓库；浏览器只查看安全状态，并显式触发测试连接、创建私有仓库、确认上传已完成脱敏 Markdown（见第 5.2 节）。共享 `filebay-core` 供桌面与 Runtime 复用。
- 企业端部署与操作文档（`docs/enterprise/`）及相关配置样例（`.env.example`、`requirements-ocr.txt`）、Linux 交付材料（`deploy/linux/`）。

个人端桌面应用的中文姓名识别规则本轮**不改动**，保持现状。

## 1. 组件关系（客户交付拓扑：原 Vault 前端 + Nginx + Runtime）

```
浏览器 / 企业内网系统
  │  HTTP，经 Nginx（内网地址 + 客户 CIDR 白名单）
  ▼
Nginx
  ├─ /       仓库根目录原 Vault 前端 dist/（pnpm exec vite build 产物，静态资源）
  └─ /api/   反向代理 → 127.0.0.1:8787（Runtime，仅 loopback，不直接对外暴露）
                │
                ▼
     apps/vault-runtime-api   企业 Runtime（Rust / warp，独立 HTTP 服务进程）
       │  依赖（Cargo path 依赖，非网络调用）
       ├─ src-tauri/crates/engine-core        共享脱敏引擎：格式解析、脱敏规则、映射编解码
       │                                      （个人端 Tauri 应用与企业端 Runtime 共用同一份代码）
       ├─ src-tauri/crates/component-runtime  OCR 组件封装：预检、进程调度、结果校验
       │    │ 子进程调用（非网络）
       │    ▼
       │  src-tauri/scripts/pdf_ocr.py        独立 Python 进程：PyMuPDF 文本层提取 + EasyOCR 兜底
       │
       └─ apps/vault-runtime-api/src/legacy_powerpoint.rs
            │ 子进程调用（非网络）
            ▼
          LibreOffice (soffice --headless)    旧版 .ppt → .pptx 转换
```

**关键点**：真正的解析/脱敏/OCR/LibreOffice 调用全部发生在 Runtime 这一个 Rust 进程里，且只监听 `127.0.0.1`（详见第 9 节「已知限制」与 [`deploy/linux/README.md`](../../deploy/linux/README.md) 的安全边界说明）。Nginx 不做任何业务处理，只做静态资源托管、反向代理和 CIDR 访问控制。个人端桌面应用（`src/`、`src-tauri/src/`）与企业端共用 `engine-core`，但企业端的部署与个人端完全独立，本文档不涉及个人端安装。

## 2. 安装要求

| 组件 | 用途 | 本机验证版本 | 说明 |
|---|---|---|---|
| Rust / Cargo | 编译 `engine-core`、`component-runtime`、`vault-runtime-api` | `rustc 1.97.0`、`cargo 1.97.0` | 各 crate `Cargo.toml` 声明 `edition = "2021"`；间接依赖 `lopdf-parang` 要求 `rust-version 1.85`，建议使用 1.85 及以上工具链 |
| Node.js / pnpm | 构建**客户入口**（仓库根目录原 Vault 前端）；也可用于构建 `apps/vault-pro-web`（不用于客户部署，仅该应用自身开发用） | `node v22.16.0`、`pnpm 11.17.0`（由根 `package.json` 的 `"packageManager"` 字段精确声明；未全局安装 pnpm 时用 `corepack pnpm` 调用会自动解析出这个版本） | 根前端构建命令见第 5 节；Vite 7 通常要求 Node 18+（**未在其他版本上验证**，仅记录本机实测版本）。统一用 pnpm，不使用 npm |
| Python | 运行 OCR 组件（`pdf_ocr.py`） | `Python 3.13.3` | 见第 3 节 |
| LibreOffice | 旧版 `.ppt` → `.pptx` 转换 | `LibreOffice 26.2.4.2`（macOS 便携版，路径见第 3.4 节，`soffice --version` 实测输出）；**Linux 上须用发行版包管理器正式安装**（如 `apt install libreoffice` / `dnf install libreoffice`），见第 3.4 节 | 企业端特有依赖，个人端不支持旧版 `.ppt`（见第 9 节已知限制） |
| Nginx | Linux 内网客户测试部署的唯一入口（托管前端 + 反代 Runtime + CIDR 访问控制） | 本机 macOS 补充验证用 `nginx/1.31.3`（Homebrew，仅用于模板 `nginx -t` 语法检查，非 Linux 替代） | 见 [`deploy/linux/nginx-cheersai-vault.conf`](../../deploy/linux/nginx-cheersai-vault.conf)；Linux 生产路径需在隔离 Linux 主机安装发行版自带的 Nginx |
| systemd | Linux 上管理 Runtime 进程的生命周期（启动、重启、SIGTERM 优雅停机） | 目标 Linux 发行版自带 | 见 [`deploy/linux/vault-runtime-api.service`](../../deploy/linux/vault-runtime-api.service) |

## 3. OCR 安装步骤（管理员一次性操作，浏览器用户不需要）

### 3.1 建立 Python 虚拟环境

```bash
python3 -m venv /path/to/ocr-venv
/path/to/ocr-venv/bin/pip install -r src-tauri/scripts/requirements-ocr.txt
```

`requirements-ocr.txt` 精确锁定了实测通过的版本组合（`easyocr==1.7.2`、`torch==2.13.0`、`PyMuPDF==1.28.0`、`Pillow==12.3.0` 及其全部传递依赖），验证用 Python 版本为 `3.13.3`。**实际执行结果**：

```
$ /path/to/ocr-venv/bin/python --version
Python 3.13.3
$ /path/to/ocr-venv/bin/pip freeze
easyocr==1.7.2
... (完整清单见 requirements-ocr.txt)
```

### 3.2 准备 EasyOCR 模型

Runtime 调用的 `pdf_ocr.py` 与预检逻辑全程使用 `download_enabled=False`（`pdf_ocr.py` 的 `_build_reader()` 默认参数、`ocr_with_easyocr()` 显式传入，以及 `component-runtime/src/lib.rs` 的离线预检代码），**运行时绝不会自动联网下载模型**。管理员必须在启动 Runtime 前把模型文件准备好。两种获取方式任选其一：

**方式 A：管理员在安装阶段准备模型，运行时关闭下载**（本次验证采用的方式）：

```bash
/path/to/ocr-venv/bin/python -c "
import easyocr
easyocr.Reader(['ch_sim','en'], gpu=False, model_storage_directory='/path/to/ocr-models', download_enabled=False)
"
```

这条检查只接受已经存在且可离线加载的模型，不会联网下载。部署管理员可以在受控的安装流程中准备模型文件；本任务和 Runtime 本身不安装、下载、复制或移动 OCR 组件。

**方式 B：手动下载官方发布包放置**：从 EasyOCR 官方发布地址下载后解压到同一目录：

- `craft_mlt_25k.pth`：<https://github.com/JaidedAI/EasyOCR/releases/download/pre-v1.1.6/craft_mlt_25k.zip>
- `zh_sim_g2.pth`：<https://github.com/JaidedAI/EasyOCR/releases/download/v1.3/zh_sim_g2.zip>

两种方式都应放到同一个目录（即 `VAULT_OCR_MODEL_DIR` 指向的目录），并用 SHA-256 校验（见第 7 节）确认文件完整、未被篡改，作为两种获取路径的共同校验手段。

### 3.3 确认 OCR 依赖版本

在 3.1 节创建的 venv 中执行以下命令：排除 `requirements-ocr.txt` 里的注释行与空行，两边各自排序后比对：

```bash
diff <(/path/to/ocr-venv/bin/pip freeze | sort) \
     <(grep -v '^#\|^$' src-tauri/scripts/requirements-ocr.txt | sort)
```

**本机实际执行结果**：无任何输出，退出码 `0`——两边完全一致，无需人工判断大小写或顺序差异。

### 3.4 LibreOffice

企业端旧版 `.ppt` 转换依赖 LibreOffice 的 `soffice` 命令行工具。Runtime 按以下顺序查找（`legacy_powerpoint.rs` 注释原文）：

1. `CHEERSAI_LIBREOFFICE_PATH` 环境变量（显式指定，**生产环境推荐**）
2. `/Applications/LibreOffice.app/Contents/MacOS/soffice`（macOS 标准安装位置）
3. `/opt/homebrew/bin/soffice`、`/usr/local/bin/soffice`（Homebrew 安装位置）
4. `PATH` 中的 `soffice`
5. `/tmp/ppt-conversion-feasibility/libreoffice-portable/...`（**仅作为最后兜底，不推荐依赖**——`/tmp` 可能在系统重启后被清空，届时旧版 `.ppt` 支持会失效）

**本机实测状态（macOS 开发机）**：`/Applications` 与 `PATH` 中均未安装 LibreOffice，当前唯一可用的是候选 5（`/tmp` 便携版）。**生产/客户部署强烈建议**：正式安装 LibreOffice，或设置 `CHEERSAI_LIBREOFFICE_PATH` 指向一个不会被清理的正式安装路径，不要让部署依赖 `/tmp`。

每个候选路径在被采用前都会先执行 `soffice --version` 验证可运行；能通过验证的路径会被缓存，验证失败的路径会被跳过并尝试下一个候选，失败结果不缓存（下次调用重新探测）。

**Linux 客户测试部署的 LibreOffice 安装/检查**（本轮未在真实 Linux 环境执行，以下步骤基于 `legacy_powerpoint.rs` 的候选路径逻辑给出，标注为**未验证**）：

1. 用发行版包管理器正式安装（例如 Debian/Ubuntu：`apt install libreoffice`；RHEL/openSUSE 等按各自包管理器），**不要**依赖任何便携版或 `/tmp` 拷贝——候选 5（`/tmp/ppt-conversion-feasibility/...`）是一个 macOS `.app` 包结构的路径，在 Linux 上天然不存在也不会被命中，因此 Linux 部署不会意外落到这条兜底路径。
2. 候选 2/3（`/Applications/...`、`/opt/homebrew/bin/soffice`）是 macOS 专属路径，在 Linux 上同样不存在，会被自动跳过。
3. 正式包管理器安装后，`soffice` 通常已在 `PATH` 上（候选 4，`which soffice`），可直接验证：
   ```bash
   which soffice && soffice --version
   ```
4. 若 `soffice` 不在 `PATH` 上（例如自定义安装路径），显式设置 `CHEERSAI_LIBREOFFICE_PATH` 指向该路径（见第 4.3 节），**推荐生产环境始终显式设置该变量**，不依赖自动探测顺序。
5. 验证方式与 macOS 一致：`"$CHEERSAI_LIBREOFFICE_PATH" --version` 或 `which soffice && soffice --version` 应有正常版本号输出；随后建议提交一份真实旧版 `.ppt` 端到端确认转换可用（见第 8.3 节）。

## 4. 环境变量（共 15 个）

完整示例见 [`apps/vault-runtime-api/.env.example`](../../apps/vault-runtime-api/.env.example)（**不含任何真实值**，复制后自行填写）。

### 4.1 Runtime 运行期变量（9 个，`vault-runtime-api` 进程直接读取）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `VAULT_RUNTIME_DATA_DIR` | 批次/文件状态与产物的持久化目录 | `enterprise-data`（相对路径） | 否 |
| `VAULT_RUNTIME_PORT` | Runtime 监听端口（固定绑定 `127.0.0.1`） | `8787` | 否 |
| `VAULT_RUNTIME_CORS_ORIGINS` | 允许的 CORS 来源，逗号分隔；解析后为空则启动失败（退出码 2） | `http://127.0.0.1:5173,http://localhost:5173` | 否 |
| `VAULT_OCR_PYTHON` | OCR 虚拟环境的 Python 解释器路径 | 无（全缺省时自动发现） | 否；显式模式下必填 |
| `VAULT_OCR_SCRIPT` | `pdf_ocr.py` 的路径 | 无（全缺省时自动发现） | 否；显式模式下必填 |
| `VAULT_OCR_MODEL_DIR` | EasyOCR 模型目录 | 无（自动发现时先用安装目录 `model/`，其次用当前用户 `~/.EasyOCR/model`） | 否；显式模式下必填 |
| `VAULT_OCR_TIMEOUT` | 单次 OCR 子进程超时秒数 | `300` | 否 |
| `VAULT_OCR_MAX_PAGES` | 单次 OCR 最多处理页数 | `200` | 否 |
| `VAULT_OCR_MAX_PIXELS_PER_PAGE` | 单页 300 DPI 渲染像素上限（默认覆盖 Letter/A4/Legal 整页并留余量） | `12000000` | 否 |

当任一路径变量被设置时进入显式模式：缺少任一必需路径、路径不存在或模型不完整会返回真实的 `unavailable`/`invalid`，不会回退到另一套安装。只有三个路径变量全部未设置时，Runtime 与桌面才进入同一套自动发现：按当前系统用户的标准 `com.cheersai.vault/ocr-package` 候选查找既有 Python/`pdf_ocr.py`，模型优先使用安装目录 `model/`，其次使用当前用户的标准 `~/.EasyOCR/model`。该发现不跨用户、不递归扫描、不联网；深度预检和实际 OCR 始终使用 `download_enabled=False`。

### 4.2 前端构建期变量（1 个，`vite build`/`vite dev` 读取，**不是** Runtime 进程环境变量）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `VITE_RUNTIME_API_URL` | 前端访问 Runtime API 的基地址 | 未设置时，生产构建走**同源 `/api`**（由 Nginx 反代到 Runtime，见第 0 节拓扑），本机开发默认回落到 `http://127.0.0.1:8787` | 客户 Linux 部署**不应设置**此变量——前端应始终用同源 `/api` 经 Nginx 访问 Runtime，不要在前端里写死服务器 IP。仅本机开发或需要跨源直连 Runtime 调试时才显式设置；设置时必须是 `http://` + loopback 地址（`127.0.0.1`/`localhost`/`[::1]`），否则前端启动即抛错（`src/lib/runtime/client.ts` `validateBaseUrl()`） |

> `apps/vault-pro-web` 有自己独立的同名变量与 `client.ts`（`apps/vault-pro-web/src/api/client.ts`），只影响该应用自身，与客户交付路径（仓库根前端）无关。

### 4.3 LibreOffice 覆盖变量（1 个，Runtime 进程读取）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `CHEERSAI_LIBREOFFICE_PATH` | 显式指定 `soffice` 路径，跳过自动探测 | 无（走第 3.4 节的自动探测顺序） | 否；生产环境推荐显式设置 |

### 4.4 FileBay 浏览器上传变量（4 个，Runtime 进程启动时读取一次）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `VAULT_FILEBAY_URL` | FileBay/Gitea 服务器 origin，必须是**裸 HTTPS 根 origin**（无用户名/密码/查询串/片段/非根路径）；证书校验始终开启 | 无 | 否（四项同时配置时才启用，见下） |
| `VAULT_FILEBAY_TOKEN` | FileBay/Gitea 访问令牌，Runtime 上传时使用的身份 | 无 | 否（同上） |
| `VAULT_FILEBAY_OWNER` | 目标仓库属主（用户或组织），限定 Gitea 接受的标识符字符 | 无 | 否（同上） |
| `VAULT_FILEBAY_REPO` | 目标仓库名；不存在时浏览器“创建仓库”动作只创建**私有**仓库 | 无 | 否（同上） |

- 四项只由 **Runtime 进程环境**提供，`filebay.rs` 在**启动时读取一次**（`FileBaySession::from_env()`），之后无热加载、无管理 API；**修改后必须重启 Runtime 才生效**。
- Token 只写入部署管理员维护的 Runtime 环境文件：环境文件 `0600`、属主为 Runtime 服务账户、systemd `UMask=0077`、不提交版本库、不放 Nginx Web root；不提供把 Token 写入命令行参数、URL、前端、截图、日志、README 示例真实值或浏览器 storage 的做法。
- 配置状态语义（与实现一致）：
  - 四项全部未设置 → `unconfigured`（未配置）：浏览器 FileBay 状态显示未配置，FileBay 相关动作被禁用，但 **Runtime 的脱敏、OCR、恢复、日志、沙箱/PIN 与客户批量 API 全部正常运行**。
  - 四项全部设置且校验通过 → `configured`（已配置）。
  - 只设置部分、或 URL/owner/repo 校验失败 → `invalid`（配置无效）：FileBay 写入类动作一律安全失败（固定错误码），不展示底层响应正文、Token 或服务器路径。
- 浏览器不能读取、回显或修改 Token（`GET /api/v1/filebay/status` 只返回是否存在 Token 的 `has_token` 布尔值与必要目标元数据，不返回 Token 本身），不能设置任意目标 URL/owner/repo。

## 5. 启动顺序与命令

### 5.1 客户部署启动顺序（原 Vault 根前端 + Nginx + systemd，Linux 目标环境）

以下是客户交付路径的启动顺序，与第 0 节拓扑对应。**Nginx/systemd 相关步骤本轮未在真实 Linux 环境执行**，标注为**未验证**；构建命令本身在 macOS 上实测通过（见下）。完整模板与逐步说明见 [`deploy/linux/README.md`](../../deploy/linux/README.md)。

```bash
# 1. 构建 Runtime release 二进制
cargo build --release --manifest-path apps/vault-runtime-api/Cargo.toml

# 2. 构建仓库根目录原 Vault 前端（客户入口，不是 apps/vault-pro-web）
pnpm install --frozen-lockfile
pnpm exec vite build
# 产物在仓库根 dist/，交给 Nginx 托管，不要单独用 `pnpm dev`/临时静态服务器对外提供服务

# 3. 【Linux，未验证】按 deploy/linux/vault-runtime-api.service 安装 systemd 单元并启动 Runtime
#    systemctl daemon-reload && systemctl enable --now vault-runtime-api

# 4. 【Linux，未验证】按 deploy/linux/nginx-cheersai-vault.conf 配置 Nginx（填入真实客户 CIDR、
#    dist/ 路径），nginx -t 验证后 reload

# 5. 【Linux，未验证】运行黑盒 smoke 测试
#    ./deploy/linux/smoke-test.sh http://<Nginx内网地址>/api/v1
```

**本机 macOS 实测**（构建命令本身，OS 无关部分）：`cargo build --release --manifest-path apps/vault-runtime-api/Cargo.toml` 与 `pnpm exec vite build`（仓库根目录）均以退出码 `0` 完成；`pnpm exec tsc --noEmit` 同样退出码 `0`。Nginx 语法层面额外用本机 Homebrew 安装的 `nginx -t` 对 `deploy/linux/nginx-cheersai-vault.conf` 模板做过语法检查（替换占位符后语法通过），**这只验证模板语法正确，不能替代在真实 Linux 主机上的实际启动、CIDR 访问控制、与 Runtime 联调等验证**。

### 5.2 FileBay 浏览器上传配置（管理员 + 单一系统用户）

本版本按**单一系统用户**交付：一台 Runtime 对应一个系统身份、一个受信任服务器
工作区、一套由管理员配置的 FileBay 目标（固定 HTTPS 私有仓库）和一个共享 PIN。
所有能访问该内网页面的浏览器会话共享这些服务器状态；它们**不构成登录、RBAC、
管理员/普通用户区分或多租户隔离**，页面、手册与部署文档不得作此宣称。“仅企业
内网、不暴露公网”的边界不变（Runtime 只监听 `127.0.0.1`，Nginx CIDR 白名单是
唯一访问边界，见第 0 节）。FileBay 与沙箱/PIN 都是单 Runtime 共享的服务器状态，
不是用户级授权；共享 PIN 只保护沙箱操作，**不是登录凭据或用户身份**。

**管理员配置步骤**：

1. 在 Runtime 环境文件中**同时**配置四个变量（见 4.4），示例占位符见
   [`deploy/linux/runtime.env.example`](../../deploy/linux/runtime.env.example)：
   ```bash
   VAULT_FILEBAY_URL=https://<FILEBAY_SERVER_HOST>
   VAULT_FILEBAY_TOKEN=<由部署管理员安全填写>
   VAULT_FILEBAY_OWNER=<FILEBAY_OWNER>
   VAULT_FILEBAY_REPO=<FILEBAY_REPO>
   ```
   URL 必须是裸 HTTPS 根 origin；Token 由部署管理员用安全方式获取并填写，示例
   值均不可用。环境文件权限 `0600`、属主为 Runtime 服务账户（配合 systemd
   `UMask=0077`），不得提交版本库，不得放入 Nginx Web root。
2. **重启 Runtime** 使配置生效（启动时只读取一次，无热加载）：
   ```bash
   systemctl restart vault-runtime-api
   ```
3. **启动前检查（不含真实凭据）**：确认四个变量都已在环境文件中设置且格式合法
   （URL 为 `https://...` 根 origin；owner/repo 只含 Gitea 接受的标识符字符）；
   确认环境文件权限为 `0600` 且属主正确。不要用 `env`/`printenv`/`set` 批量打印
   环境值；如需要检查，只检查模板文件字段名，不打印真实值。
4. **浏览器状态检查**：以客户内网浏览器打开 `/gitea` 页，确认状态为“已配置”，
   目标地址为管理员配置的 HTTPS origin，仓库为 `owner/repo`，Token 仅显示
   “已配置”（浏览器拿不到 Token 本身）。

**浏览器操作流程**：

- `/gitea`：只展示安全状态，并允许用户**显式**发起两个动作——**测试连接**
  （`POST /api/v1/filebay/test`）与**创建私有仓库**（`POST /api/v1/filebay/repository`，
  仓库固定为私有，已存在则不重复创建）。刷新状态不出站。
- `/files`：只有 Runtime 确认状态为 `Completed` 的脱敏 Markdown 才会出现在上传
  候选（`GET /api/v1/filebay/batches/{batch_id}/candidates`）；上传候选只携带
  `artifact_id`、`display_name` 与服务器生成的远端路径。确认弹窗展示安全文件名、
  目标 origin、owner/repo、远端路径与“只上传脱敏 Markdown、不上传原文/`.cmap`/
  还原产物”提示，浏览器只提交 `artifact_ids`（最多 100 个），远端路径由服务器
  生成；上传结果逐文件返回成功/失败与错误码（`POST /api/v1/filebay/uploads`）。
- 页面加载、处理完成、下载、恢复、取消确认都**不会**自动上传或自动访问 FileBay；
  上传只能由用户在确认弹窗中主动点击“确认上传”触发。状态查询与候选查询不出站，
  只有显式测试连接、创建私有仓库和确认上传允许出站。

**真实 Linux / 真实 FileBay 尚未验收**：本文档第 5.1 节的 Linux systemd/Nginx
步骤与本节 FileBay 远端闭环都尚未在真实 Linux 主机或真实 FileBay 上验收（当前
无隔离 Linux 主机与专用测试凭据），不得把本机 macOS 或 fake transport 测试结果
当作真实远端已通过的证据。

### 5.3 历史参考：`apps/vault-pro-web` 本机构建验证记录（非客户部署路径）

以下内容是本项目更早阶段针对 `apps/vault-pro-web`（企业 Web，单机 macOS MVP 阶段的浏览器前端）积累的构建验证记录，继续保留供该应用自身开发参考，**不代表 Linux 客户测试部署路径**——客户浏览器入口请参见 5.1 节的仓库根前端。

```bash
# 1. 构建并启动 Runtime（先于 Web，Web 依赖 Runtime 的 HTTP API）
cd apps/vault-runtime-api
cargo build --release

VAULT_RUNTIME_DATA_DIR=/path/to/enterprise-data \
VAULT_OCR_PYTHON=/path/to/ocr-venv/bin/python3 \
VAULT_OCR_SCRIPT=/path/to/CheersAI-Vault/src-tauri/scripts/pdf_ocr.py \
VAULT_OCR_MODEL_DIR=/path/to/ocr-models \
CHEERSAI_LIBREOFFICE_PATH=/path/to/soffice \
  ./target/release/vault-runtime-api
# 输出：vault-runtime-api listening on http://127.0.0.1:8787

# 2. 构建企业 Web（另一个终端，从仓库根目录开始）
cd apps/vault-pro-web
pnpm install --frozen-lockfile
VITE_RUNTIME_API_URL=http://127.0.0.1:8787 pnpm build
# 产物在 dist/，用任意静态文件服务器托管，或本地用 `pnpm dev` 直接预览
```

企业 Web 统一使用 pnpm（app-local `package.json` + `pnpm-lock.yaml` + `pnpm-workspace.yaml`），不使用 npm。`apps/vault-pro-web/package.json` 精确声明了 `"packageManager": "pnpm@11.17.0"`；若本机未全局安装 pnpm，可用 `corepack pnpm ...` 代替上述 `pnpm ...` 命令——Node.js 自带的 corepack 会读取这个字段，自动获取并使用**该精确版本**的 pnpm，而不是"某个可用版本"，因此任何人 clone 仓库后用 `corepack pnpm` 执行都会得到与本文档实测相同的 pnpm 版本。

**本机实际验证**（全新隔离临时目录，仅复制 `apps/vault-pro-web` 的 `package.json`（含 `packageManager` 字段）、`pnpm-lock.yaml`、`pnpm-workspace.yaml`、`index.html`、`src/`、`public/`、`tsconfig.json`、`vite.config.ts`，不复用仓库内已有的 `node_modules`）：

```
$ node --version
v22.16.0

$ corepack pnpm --version
11.17.0
```

`corepack pnpm --version` 输出与 `package.json` 里 `"packageManager": "pnpm@11.17.0"` 精确一致——证明 Corepack 确实是按该字段解析出这个版本，而不是解析到本机偶然安装的某个 pnpm。

```
$ corepack pnpm install --frozen-lockfile
Lockfile is up to date, resolution step is skipped
Packages: +78
...
Done in 1.2s using pnpm v11.17.0

$ VITE_RUNTIME_API_URL=http://127.0.0.1:8787 corepack pnpm build
$ tsc --noEmit && vite build
✓ 1764 modules transformed.
dist/index.html                   0.54 kB
dist/assets/index-*.css          12.75 kB
dist/assets/index-*.js          258.46 kB
✓ built in 1.23s
```

三条命令退出码均为 `0`，证明 `pnpm-lock.yaml` 与 `package.json` 一致（冻结安装未触发任何依赖版本变化）、`pnpm-workspace.yaml` 的 `allowBuilds.esbuild: true` 配置有效（esbuild 的 postinstall 脚本被正确批准执行）。

**本机实测**：按上述方式启动 Runtime（含 OCR 与 LibreOffice 覆盖变量），`curl http://127.0.0.1:8787/api/v1/health` 返回 `{"status":"ready","version":"0.1.0"}`。

## 6. 数据目录与 `.cmap` 隐私边界

`VAULT_RUNTIME_DATA_DIR` 下按批次 ID / 产物 ID 组织，本机实测目录结构：

```
enterprise-data/
├── input/<batch_id>/      原始上传文件
├── output/<batch_id>/     脱敏后的 Markdown 产物
└── mapping/<batch_id>/<artifact_id>.cmap   服务器内部脱敏映射（原文 ↔ 占位符）
```

- **`.cmap` 是服务器内部的明文 JSON 敏感映射文件（MVP 格式），不做静态加密**（`engine-core/src/mapping.rs` 中 `encode_server_cmap()` 直接 `serde_json::to_vec` 序列化，没有 AES/KMS/口令等任何加密或密钥派生步骤）。**不得声称已加密、使用 KMS 或口令保护**——当前实现没有这些能力。
- `.cmap` 文件**只保存在服务器本地磁盘**，前端「反脱敏」页面**没有任何下载入口**（已核对页面全部可交互元素：仅一个文件选择列表 + 一个「开始反脱敏」按钮），用户只能在服务器上通过 API 触发"恢复"得到还原后的文本，无法拿到映射文件本身。
- 当前 MVP 对 `.cmap` 的保护完全依赖**部署方的操作系统层面控制**：运行 Runtime 进程的账号隔离，以及 `VAULT_RUNTIME_DATA_DIR`（含其下 `mapping/` 子目录）的文件系统权限设置（例如只允许 Runtime 运行账号读写，禁止其他本机账号访问）。部署方需自行确保这些权限设置到位；本项目代码本身不做额外加密或访问控制。
- 数据库（SQLite，位于 `VAULT_RUNTIME_DATA_DIR` 下）不持久化原文内容或本机文件系统路径。
- 部署方应将 `VAULT_RUNTIME_DATA_DIR` 设在有访问控制的磁盘位置，并纳入自己的备份/权限策略——本项目本身不提供额外的访问控制层（见第 9 节已知限制）。

## 7. 模型来源与校验

| 文件 | 用途 | 大小 | SHA-256（本机实测） | 官方来源 |
|---|---|---|---|---|
| `craft_mlt_25k.pth` | 文字检测模型 | 83,152,330 字节（约 79 MB） | `4a5efbfb48b4081100544e75e1e2b57f8de3d84f213004b14b85fd4b3748db17` | <https://github.com/JaidedAI/EasyOCR/releases/download/pre-v1.1.6/craft_mlt_25k.zip> |
| `zh_sim_g2.pth` | 简体中文识别模型 | 21,951,421 字节（约 21 MB） | `cb678fdef09d651e7763ca551ad790dc89f0b2e3d2a640484330e338fb574c7a` | <https://github.com/JaidedAI/EasyOCR/releases/download/v1.3/zh_sim_g2.zip> |

计算方式（本机实测命令）：

```bash
shasum -a 256 /path/to/ocr-models/craft_mlt_25k.pth /path/to/ocr-models/zh_sim_g2.pth
```

**许可证**：两个模型文件随 `easyocr` 包分发，`easyocr` 本体许可证为 **Apache License 2.0**（来源：`pip show easyocr` 的 `License` 字段，仓库 <https://github.com/jaidedai/easyocr>）。

无论采用第 3.2 节的方式 A 还是方式 B 获取模型，都应用上表的 SHA-256 校验文件完整性，这是两种获取路径的共同校验手段。

## 8. 就绪检查与排障

### 8.1 OCR 就绪检查

```bash
curl -s http://127.0.0.1:8787/api/v1/ocr/status
```

**本机实测输出**（配置正确、模型就绪时）：

```json
{"status":"ready","model_ready":true,"timeout_secs":300,"max_pages":200}
```

`status` 可能的取值：

| `status` | 含义 |
|---|---|
| `ready` | 可以处理扫描件 |
| `invalid` | Python/脚本存在，但依赖或模型不完整（例如显式模型路径未就绪，或自动发现到的共享安装模型不完整） |
| `unavailable` | 全部路径变量缺省且没有发现当前用户的共享安装，或显式模式下 Python/脚本路径不可用 |

### 8.2 LibreOffice 就绪确认

Runtime 本身没有单独的"就绪检查" HTTP 接口；确认方式是直接验证被选中的 `soffice` 是否可运行（与 Runtime 内部探测逻辑一致）：

```bash
"$CHEERSAI_LIBREOFFICE_PATH" --version
```

**本机实测输出**（针对当前 `/tmp` 便携版路径）：

```
$ "$CHEERSAI_LIBREOFFICE_PATH" --version
LibreOffice 26.2.4.2 0229ac93fcf0d7cbc6376066c6f35021cef002dc
```

提交一份真实旧版 `.ppt` 更能端到端确认可用性，见第 8.3 节测试记录。

### 8.3 排障表

| 错误码 | 常见原因 | 处理方式 |
|---|---|---|
| `OCR_COMPONENT_REQUIRED` | 没有发现可用 OCR，或文件确实是无文字层的扫描件但 OCR 未启用 | 管理员按第 3 节准备 OCR，或确认同一系统用户的共享安装目录可被自动发现后重启 Runtime |
| `OCR_COMPONENT_INVALID` | Python 环境存在但依赖/模型不完整；或 OCR 子进程返回了未分类的内部错误 | 检查显式路径或共享安装目录中的 `requirements-ocr.txt`、模型；查看 Runtime 启动日志中的 `OCR component status` 一行 |
| `OCR_NO_TEXT` | OCR 已运行，但图像上确实识别不到任何文字（例如空白页） | 确认原始扫描件本身有可辨识文字；非 Runtime 配置问题 |
| `OCR_TIMEOUT` | 单次 OCR 超过 `VAULT_OCR_TIMEOUT` 秒数 | 提高 `VAULT_OCR_TIMEOUT`，或确认文件页数/分辨率未过大 |
| `INPUT_LIMIT_EXCEEDED` | 超出大小、页数或 `VAULT_OCR_MAX_PIXELS_PER_PAGE` 像素上限 | 确认文件是否超出 `OPERATION_GUIDE.md` 第 6 节列出的限制；扫描件像素超限可评估是否需要提高 `VAULT_OCR_MAX_PIXELS_PER_PAGE`（默认值已覆盖 Letter/A4/Legal，谨慎调大） |
| `LEGACY_CONVERTER_UNAVAILABLE` | 未找到可运行的 `soffice`（第 3.4 节全部候选路径都失败） | 安装 LibreOffice 或设置 `CHEERSAI_LIBREOFFICE_PATH`；用 `soffice --version` 手动验证 |

### 8.4 FileBay 状态检查与安全排障

**状态检查**（不出站，浏览器 `/gitea` 页也可直接查看）：

```bash
curl -s http://127.0.0.1:8787/api/v1/filebay/status
```

返回的 `status` 取值为 `unconfigured` / `configured` / `invalid`，另含 `configured`
布尔值、`has_token`、`target_origin`、`owner`、`repo`（**永不返回 Token 值**）。
状态查询只读本进程内存，不触发任何 FileBay 出站请求。

**安全排障表**（错误码均为固定安全文案，不展示底层响应正文、Token 或服务器路径）：

| 错误码 | 常见原因 | 处理方式（管理员） |
|---|---|---|
| `FILEBAY_NOT_CONFIGURED` | 四项环境变量全部未设置 | 按第 5.2 节配置四项后重启 Runtime；此状态下脱敏/OCR/恢复/日志/沙箱/客户 API 不受影响 |
| `FILEBAY_CONFIG_INVALID` | 只设置了部分变量，或 URL/owner/repo 校验失败 | 检查 `VAULT_FILEBAY_URL` 是否为裸 HTTPS 根 origin、owner/repo 是否为合法标识符，补齐四项后重启 |
| `FILEBAY_AUTH_FAILED` | Token 缺失/失效，或远端返回认证失败 | 用安全方式从管理员侧获取/更新 Token，写入环境文件后重启 |
| `FILEBAY_CONNECTION_FAILED` | 目标地址不可达、TLS/证书失败或远端未就绪 | 确认目标 HTTPS 可达、证书有效、防火墙/Nginx 放行；Runtime 不提供关闭证书校验的开关 |
| `FILEBAY_REPOSITORY_NOT_FOUND` | 目标仓库不存在 | 在浏览器 `/gitea` 页显式执行“创建私有仓库”（只会创建私有仓库） |
| `FILEBAY_REPOSITORY_CREATE_FAILED` | 创建私有仓库失败（如权限不足） | 检查 Token 对目标 owner 是否有创建仓库的权限 |
| `FILEBAY_UPLOAD_FAILED` | 上传中途失败（可重试） | 确认网络与磁盘后让用户在 `/files` 确认弹窗重新上传 |
| `FILEBAY_REQUEST_INVALID` / `FILEBAY_UPLOAD_DENIED` | 提交了空/超量/重复 artifact ID，或产物不在上传白名单 | 属于正常防护，检查浏览器仅提交了 `Completed` 脱敏 Markdown 的 artifact ID |

## 9. 已知限制

- **旧版 `.ppt` 依赖 LibreOffice**；个人端桌面应用（`src-tauri` Tauri 打包）**不支持**该格式（双端能力差异，非缺陷）。
- **扫描件 OCR 需要管理员预先安装**（第 3 节），浏览器用户无法自行启用，也没有联网自动下载的兜底路径。
- 工程内当前**同时存在两份 PDF 解析库**：`lopdf 0.32`（用于结构校验：加密检测、页数限制）与 `lopdf-parang 0.39.1`（`parangi` 内部用于文本提取）。二者对同一份 PDF 的判断在个别边缘场景可能不完全一致（已知一例：某些 PDF 的加密检测边界情况曾被误判，已在 `parse_pdf()` 中增加 trailer 兜底检查缓解）。
- `parangi 0.1.0`（PDF 文本提取库）目前是其**唯一发布版本**，建议关注上游维护活跃度。
- Runtime 进程本身固定绑定 `127.0.0.1`，`VITE_RUNTIME_API_URL` 显式设置时也被前端强制校验为 loopback 地址；Linux 内网客户测试部署通过 Nginx 反向代理获得内网可达性（见第 0 节），但**这不等于正式生产多用户/多租户能力**——本版本不含权限管理、用户登录、操作审计签名、多租户隔离。
- **单系统用户边界**：本版本按单一系统用户交付，FileBay 与沙箱/PIN 都是单 Runtime 共享的服务器状态（一台 Runtime、一个受信任工作区、一套 FileBay 配置、一个共享 PIN）。共享 PIN 只保护沙箱操作，共享 FileBay 配置也不是用户级授权；不提供“个人账号/管理员账号/普通用户/每用户仓库、Token、PIN、目录”，也不提供 RBAC 或多租户隔离。单用户不等于公网匿名开放——Linux 内网、Nginx CIDR 与 Runtime loopback 边界继续保留。
- **真实 Linux / 真实 FileBay 尚未验收**：浏览器 FileBay Runtime 适配已通过 Review（基于 fake transport 与受控浏览器响应，未连接真实 FileBay），Linux systemd/Nginx 部署与 FileBay 远端闭环仍需在提供隔离 Linux 主机与专用测试凭据后另行独立验收；不得把本机 macOS 或 fake 测试结果当作真实远端已通过的证据。
- **Linux 内网客户测试部署没有应用层鉴权**：唯一的访问控制是 Nginx 的客户 CIDR 白名单（见 [`deploy/linux/nginx-cheersai-vault.conf`](../../deploy/linux/nginx-cheersai-vault.conf)），任何能连通 Nginx 监听地址的主机都能调用全部四项客户 API（见 [API_REFERENCE.md](./API_REFERENCE.md)）。**不得将本部署对公网开放，不得将其描述为正式安全生产部署**。数据目录的访问控制完全依赖部署方自己的操作系统权限设置（用户账号隔离 + 文件权限）。
