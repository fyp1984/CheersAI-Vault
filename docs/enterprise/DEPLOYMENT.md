# CheersAI Vault 企业版 · 部署与技术说明

面向：项目维护者 / 开发者 / 部署管理员。浏览器端使用说明见 [OPERATION_GUIDE.md](./OPERATION_GUIDE.md)。

> 本文档里的每一条命令与检查方式都在本机（macOS，Apple Silicon）实际执行过一次，文中直接给出真实输出。未标注来源的具体数字（版本号、路径、哈希值）均来自这次实测，不是照抄推测。凡本文档未实际验证的步骤，会明确标注「**未验证**」。

## 本版本包含什么

本次提交范围：

- 共享脱敏/解析核心（`src-tauri/crates/engine-core`）与 OCR 组件封装（`src-tauri/crates/component-runtime`）。
- 企业端 Runtime（`apps/vault-runtime-api`）与企业端 Web（`apps/vault-pro-web`）。
- 桌面端为兼容共享核心所做的必要适配（`src-tauri/src/`、`src/`）。
- 企业端部署与操作文档（`docs/enterprise/`）及相关配置样例（`.env.example`、`requirements-ocr.txt`）。

个人端桌面应用的中文姓名识别规则本轮**不改动**，保持现状。

## 1. 组件关系

```
浏览器
  │  HTTP (127.0.0.1，仅本机 loopback)
  ▼
apps/vault-pro-web   企业 Web（Vite + React，构建为静态资源，浏览器直接调用 Runtime API）
  │  HTTP (127.0.0.1:8787 默认)
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

**关键点**：Web 只是浏览器里跑的静态页面，真正的解析/脱敏/OCR/LibreOffice 调用全部发生在 Runtime 这一个 Rust 进程里，且只监听 `127.0.0.1`（详见第 8 节「单机 MVP 边界」）。个人端桌面应用（`src/`、`src-tauri/src/`）与企业端共用 `engine-core`，但企业端的部署与个人端完全独立，本文档不涉及个人端安装。

## 2. 安装要求

| 组件 | 用途 | 本机验证版本 | 说明 |
|---|---|---|---|
| Rust / Cargo | 编译 `engine-core`、`component-runtime`、`vault-runtime-api` | `rustc 1.97.0`、`cargo 1.97.0` | 各 crate `Cargo.toml` 声明 `edition = "2021"`；间接依赖 `lopdf-parang` 要求 `rust-version 1.85`，建议使用 1.85 及以上工具链 |
| Node.js / pnpm | 构建 `apps/vault-pro-web` | `node v22.16.0`、`pnpm 11.17.0`（由 `apps/vault-pro-web/package.json` 的 `"packageManager": "pnpm@11.17.0"` 精确声明；未全局安装 pnpm 时用 `corepack pnpm` 调用会自动解析出这个版本） | `package.json` 未声明 `engines` 字段；Vite 7 通常要求 Node 18+（**未在其他版本上验证**，仅记录本机实测版本）。企业 Web 统一用 pnpm，不使用 npm |
| Python | 运行 OCR 组件（`pdf_ocr.py`） | `Python 3.13.3` | 见第 3 节 |
| LibreOffice | 旧版 `.ppt` → `.pptx` 转换 | `LibreOffice 26.2.4.2`（便携版，路径见第 3.4 节，`soffice --version` 实测输出） | 企业端特有依赖，个人端不支持旧版 `.ppt`（见第 9 节已知限制） |

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

**方式 A：首次联网自动下载，之后关闭下载**（本次验证采用的方式）：

```bash
/path/to/ocr-venv/bin/python -c "
import easyocr
easyocr.Reader(['ch_sim','en'], gpu=False, model_storage_directory='/path/to/ocr-models', download_enabled=True)
"
```

首次运行会联网下载到 `/path/to/ocr-models/`；之后的所有 Runtime 调用固定使用 `download_enabled=False`，不会再联网。

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

**本机实测状态**：`/Applications` 与 `PATH` 中均未安装 LibreOffice，当前唯一可用的是候选 5（`/tmp` 便携版）。**生产部署强烈建议**：正式安装 LibreOffice 到 `/Applications`（macOS）或通过包管理器安装，或设置 `CHEERSAI_LIBREOFFICE_PATH` 指向一个不会被清理的正式安装路径，不要让部署依赖 `/tmp`。

每个候选路径在被采用前都会先执行 `soffice --version` 验证可运行；能通过验证的路径会被缓存，验证失败的路径会被跳过并尝试下一个候选，失败结果不缓存（下次调用重新探测）。

## 4. 环境变量（共 11 个）

完整示例见 [`apps/vault-runtime-api/.env.example`](../../apps/vault-runtime-api/.env.example)（**不含任何真实值**，复制后自行填写）。

### 4.1 Runtime 运行期变量（9 个，`vault-runtime-api` 进程直接读取）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `VAULT_RUNTIME_DATA_DIR` | 批次/文件状态与产物的持久化目录 | `enterprise-data`（相对路径） | 否 |
| `VAULT_RUNTIME_PORT` | Runtime 监听端口（固定绑定 `127.0.0.1`） | `8787` | 否 |
| `VAULT_RUNTIME_CORS_ORIGINS` | 允许的 CORS 来源，逗号分隔；解析后为空则启动失败（退出码 2） | `http://127.0.0.1:5173,http://localhost:5173` | 否 |
| `VAULT_OCR_PYTHON` | OCR 虚拟环境的 Python 解释器路径 | 无 | 启用 OCR 时必填 |
| `VAULT_OCR_SCRIPT` | `pdf_ocr.py` 的路径 | 无 | 启用 OCR 时必填 |
| `VAULT_OCR_MODEL_DIR` | EasyOCR 模型目录，必须真实存在且包含所需模型文件 | 无（未设置，或路径不存在时，配置解析阶段静默按未设置处理，不报错、不阻止启动） | 启用 OCR 时事实上必填 |
| `VAULT_OCR_TIMEOUT` | 单次 OCR 子进程超时秒数 | `300` | 否 |
| `VAULT_OCR_MAX_PAGES` | 单次 OCR 最多处理页数 | `200` | 否 |
| `VAULT_OCR_MAX_PIXELS_PER_PAGE` | 单页 300 DPI 渲染像素上限（默认覆盖 Letter/A4/Legal 整页并留余量） | `12000000` | 否 |

`VAULT_OCR_PYTHON`/`VAULT_OCR_SCRIPT` 缺一即视为 OCR 未配置：扫描件失败为 `OCR_COMPONENT_REQUIRED`，前端显示「OCR 不可用」，不会尝试联网或自动安装。

`VAULT_OCR_MODEL_DIR` 未设置或路径不存在时，`component-runtime` 的深度离线预检（`deep_preflight_check`）会直接判定组件状态为 `invalid`——**不存在任何回退到 EasyOCR 默认缓存目录的行为**。Runtime 调用 EasyOCR 时始终传入 `download_enabled=False`，因此模型缺失或目录配置错误无法通过自动下载自愈，必须由管理员按第 3.2 节手动准备好模型后再启动 Runtime。

### 4.2 企业 Web 构建期变量（1 个，`vite build`/`vite dev` 读取，**不是** Runtime 进程环境变量）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `VITE_RUNTIME_API_URL` | 前端访问 Runtime API 的基地址 | `http://127.0.0.1:8787` | 否；但必须是 `http://` + loopback 地址（`127.0.0.1`/`localhost`/`[::1]`），否则前端启动即抛错（`client.ts` `validateBaseUrl()`） |

### 4.3 LibreOffice 覆盖变量（1 个，Runtime 进程读取）

| 变量 | 用途 | 默认值 | 是否必填 |
|---|---|---|---|
| `CHEERSAI_LIBREOFFICE_PATH` | 显式指定 `soffice` 路径，跳过自动探测 | 无（走第 3.4 节的自动探测顺序） | 否；生产环境推荐显式设置 |

## 5. 启动顺序与命令

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
| `invalid` | Python/脚本存在，但依赖或模型不完整（例如 `VAULT_OCR_MODEL_DIR` 未设置或模型未就绪） |
| `unavailable` | 未配置（`VAULT_OCR_PYTHON`/`VAULT_OCR_SCRIPT` 缺失）或 Python/脚本路径不可用 |

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
| `OCR_COMPONENT_REQUIRED` | `VAULT_OCR_PYTHON`/`VAULT_OCR_SCRIPT` 未配置，或文件确实是无文字层的扫描件但 OCR 未启用 | 管理员按第 3 节配置三个 OCR 变量后重启 Runtime |
| `OCR_COMPONENT_INVALID` | Python 环境存在但依赖/模型不完整；或 OCR 子进程返回了未分类的内部错误 | 检查 `requirements-ocr.txt` 是否完整安装、`VAULT_OCR_MODEL_DIR` 是否指向正确目录；查看 Runtime 启动日志中的 `OCR component status` 一行 |
| `OCR_NO_TEXT` | OCR 已运行，但图像上确实识别不到任何文字（例如空白页） | 确认原始扫描件本身有可辨识文字；非 Runtime 配置问题 |
| `OCR_TIMEOUT` | 单次 OCR 超过 `VAULT_OCR_TIMEOUT` 秒数 | 提高 `VAULT_OCR_TIMEOUT`，或确认文件页数/分辨率未过大 |
| `INPUT_LIMIT_EXCEEDED` | 超出大小、页数或 `VAULT_OCR_MAX_PIXELS_PER_PAGE` 像素上限 | 确认文件是否超出第 2 节 `OPERATION_GUIDE.md` 列出的限制；扫描件像素超限可评估是否需要提高 `VAULT_OCR_MAX_PIXELS_PER_PAGE`（默认值已覆盖 Letter/A4/Legal，谨慎调大） |
| `LEGACY_CONVERTER_UNAVAILABLE` | 未找到可运行的 `soffice`（第 3.4 节全部候选路径都失败） | 安装 LibreOffice 或设置 `CHEERSAI_LIBREOFFICE_PATH`；用 `soffice --version` 手动验证 |

## 9. 已知限制

- **旧版 `.ppt` 依赖 LibreOffice**；个人端桌面应用（`src-tauri` Tauri 打包）**不支持**该格式（双端能力差异，非缺陷）。
- **扫描件 OCR 需要管理员预先安装**（第 3 节），浏览器用户无法自行启用，也没有联网自动下载的兜底路径。
- 工程内当前**同时存在两份 PDF 解析库**：`lopdf 0.32`（用于结构校验：加密检测、页数限制）与 `lopdf-parang 0.39.1`（`parangi` 内部用于文本提取）。二者对同一份 PDF 的判断在个别边缘场景可能不完全一致（已知一例：某些 PDF 的加密检测边界情况曾被误判，已在 `parse_pdf()` 中增加 trailer 兜底检查缓解）。
- `parangi 0.1.0`（PDF 文本提取库）目前是其**唯一发布版本**，建议关注上游维护活跃度。
- 当前是**单机、非远程多用户 MVP**：Runtime 固定绑定 `127.0.0.1`，`VITE_RUNTIME_API_URL` 也被前端强制校验为 loopback 地址，不支持跨机器访问；**不含权限管理、操作审计、多租户隔离**。数据目录的访问控制完全依赖部署方自己的操作系统权限设置。

