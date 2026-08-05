# Dependencies and License Inventory

本文件用于记录 CheersAI Vault 当前仓库的依赖来源、许可证信息、兼容性判断与合规风险。

## 1. 适用范围

本清单覆盖以下内容：

- 根前端 `package.json` 的直接依赖与开发依赖
- `src-tauri/` 桌面端与共享 Rust 核心的直接与间接依赖
- `apps/vault-runtime-api/` Runtime 的直接与间接依赖
- `src-tauri/scripts/requirements-ocr.txt` 对应的可选 OCR 依赖链
- 由本仓库生成的源码分发、Docker 镜像、桌面安装包、客户交付包所需关注的许可证义务

本文件不替代法律意见，但可作为工程合规基线和发布检查依据。

## 2. 结论摘要

当前结论可以先概括为三条：

1. JavaScript 与 Rust 主体依赖以 MIT、Apache-2.0、BSD、ISC 等宽松许可证为主，与本仓库主许可证 `Apache-2.0` 的兼容性整体良好。
2. 本仓库本地 crate 现已补齐 SPDX 元数据，Cargo 清单不再存在 `NOASSERTION`。
3. 可选 OCR 依赖链存在需要单独决策的高风险项，尤其是 `PyMuPDF` 的 `AGPL-3.0 / 商业许可` 双许可。

因此：

- **核心源码仓库可按 Apache-2.0 管理**
- **默认源码发布可以保留 OCR 接入代码与安装说明**
- **任何内置 OCR 运行环境的镜像、安装包或客户交付物，在额外审查前都不应视为合规闭环完成**

## 3. 盘点方法

本次清点使用了以下来源：

- Node / pnpm：
  - `package.json`
  - `pnpm-lock.yaml`
  - `pnpm licenses list --json`
- Rust / Cargo：
  - `src-tauri/Cargo.toml`
  - `apps/vault-runtime-api/Cargo.toml`
  - `cargo metadata --locked`
- Python / OCR：
  - `src-tauri/scripts/requirements-ocr.txt`
  - 本地 OCR venv 中的 `pip show`

机器生成的原始结果保存在：

- `compliance/generated/pnpm-licenses.json`
- `compliance/generated/pnpm-license-summary.tsv`
- `compliance/generated/js-direct-dependencies.tsv`
- `compliance/generated/cargo-metadata-tauri.json`
- `compliance/generated/cargo-metadata-runtime.json`
- `compliance/generated/cargo-license-summary-tauri.tsv`
- `compliance/generated/cargo-license-summary-runtime.tsv`
- `compliance/generated/python-ocr-direct-dependencies.tsv`
- `compliance/generated/python-ocr-pip-show.txt`

## 4. 主许可证兼容性判断

当前仓库主许可证为 `Apache-2.0`。

按当前盘点结果：

- `MIT`、`Apache-2.0`、`Apache-2.0 OR MIT`、`BSD-2-Clause`、`BSD-3-Clause`、`ISC`、`Zlib`、`0BSD`、`Unicode-3.0` 一般可与主许可证共存
- `MPL-2.0` 为文件级弱 copyleft，通常可与 Apache-2.0 项目共存，但若修改了 MPL 文件本身，需要履行对应义务
- `LGPL`、`GPL`、`AGPL` 需要单独看打包方式、链接方式、分发方式和网络服务方式，不能直接按“天然兼容”处理
- 数据类或元数据类许可证，如 `CC-BY-4.0`，应保留归属说明，不应简单忽略

## 5. JavaScript 直接依赖

来源：

- `package.json`
- `compliance/generated/js-direct-dependencies.tsv`

### 5.1 运行时直接依赖

| 依赖 | 版本 | 许可证 |
|---|---|---|
| `@radix-ui/react-dialog` | `1.1.15` | `MIT` |
| `@radix-ui/react-dropdown-menu` | `2.1.16` | `MIT` |
| `@radix-ui/react-label` | `2.1.8` | `MIT` |
| `@radix-ui/react-progress` | `1.1.8` | `MIT` |
| `@radix-ui/react-scroll-area` | `1.2.10` | `MIT` |
| `@radix-ui/react-select` | `2.2.6` | `MIT` |
| `@radix-ui/react-separator` | `1.1.8` | `MIT` |
| `@radix-ui/react-slot` | `1.2.3` / `1.2.4` | `MIT` |
| `@radix-ui/react-switch` | `1.2.6` | `MIT` |
| `@radix-ui/react-tabs` | `1.1.13` | `MIT` |
| `@radix-ui/react-tooltip` | `1.2.8` | `MIT` |
| `@tauri-apps/api` | `2.10.1` | `Apache-2.0 OR MIT` |
| `@tauri-apps/plugin-dialog` | `2.7.0` | `MIT OR Apache-2.0` |
| `@tauri-apps/plugin-fs` | `2.5.0` | `MIT OR Apache-2.0` |
| `@tauri-apps/plugin-opener` | `2.5.3` | `MIT OR Apache-2.0` |
| `@tauri-apps/plugin-shell` | `2.3.5` | `MIT OR Apache-2.0` |
| `class-variance-authority` | `0.7.1` | `Apache-2.0` |
| `clsx` | `2.1.1` | `MIT` |
| `lucide-react` | `0.577.0` | `ISC` |
| `react` | `19.2.4` | `MIT` |
| `react-dom` | `19.2.4` | `MIT` |
| `react-dropzone` | `15.0.0` | `MIT` |
| `react-router-dom` | `7.13.1` | `MIT` |
| `tailwind-merge` | `3.5.0` | `MIT` |
| `uuid` | `13.0.0` | `MIT` |
| `zustand` | `5.0.11` | `MIT` |

### 5.2 开发依赖

| 依赖 | 版本 | 许可证 |
|---|---|---|
| `@tauri-apps/cli` | `2.10.1` | `Apache-2.0 OR MIT` |
| `@types/node` | `25.5.0` | `MIT` |
| `@types/react` | `19.2.14` | `MIT` |
| `@types/react-dom` | `19.2.3` | `MIT` |
| `@types/uuid` | `11.0.0` | `MIT` |
| `@vitejs/plugin-react` | `4.7.0` | `MIT` |
| `autoprefixer` | `10.4.27` | `MIT` |
| `postcss` | `8.5.8` | `MIT` |
| `tailwindcss` | `3.4.19` | `MIT` |
| `tailwindcss-animate` | `1.0.7` | `MIT` |
| `typescript` | `5.8.3` | `Apache-2.0` |
| `vite` | `7.3.1` | `MIT` |

### 5.3 JavaScript 间接依赖概况

当前 `pnpm` 盘点共识别到 `207` 个依赖条目，许可证分布以宽松许可证为主：

- `MIT`： `183`
- `ISC`： `9`
- `Apache-2.0`： `5`
- `MIT OR Apache-2.0`： `4`
- `Apache-2.0 OR MIT`： `3`
- `CC-BY-4.0`： `1`
- `BSD-3-Clause`： `1`
- `0BSD`： `1`

### 5.4 JavaScript 风险点

- `caniuse-lite` 处于 `CC-BY-4.0` 分类。它通常用于构建期浏览器兼容性数据，不构成主代码 copyleft 风险，但应保留归属与许可证记录。

## 6. Rust 本地工作区组件

以下为本仓库自身 Rust 组件，现已统一声明为 `Apache-2.0`：

| 组件 | 路径 | 版本 | 许可证 |
|---|---|---:|---|
| `cheersai-vault` | `src-tauri/` | `0.1.40` | `Apache-2.0` |
| `vault-runtime-api` | `apps/vault-runtime-api/` | `0.1.0` | `Apache-2.0` |
| `engine-core` | `src-tauri/crates/engine-core` | `0.1.0` | `Apache-2.0` |
| `component-runtime` | `src-tauri/crates/component-runtime` | `0.1.0` | `Apache-2.0` |
| `filebay-core` | `src-tauri/crates/filebay-core` | `0.1.0` | `Apache-2.0` |
| `sandbox-core` | `src-tauri/crates/sandbox-core` | `0.1.0` | `Apache-2.0` |
| `service-contracts` | `src-tauri/crates/service-contracts` | `0.6.0` | `Apache-2.0` |

## 7. Rust 直接依赖

### 7.1 `src-tauri` 直接第三方依赖

| 依赖 | 版本 | 许可证 |
|---|---|---|
| `tauri` | `2.10.3` | `Apache-2.0 OR MIT` |
| `serde` | `1.0.228` | `MIT OR Apache-2.0` |
| `serde_json` | `1.0.150` | `MIT OR Apache-2.0` |
| `tokio` | `1.52.4` | `MIT` |
| `pbkdf2` | `0.12.2` | `MIT OR Apache-2.0` |
| `hmac` | `0.12.1` | `MIT OR Apache-2.0` |
| `regex` | `1.13.1` | `MIT OR Apache-2.0` |
| `csv` | `1.4.0` | `Unlicense/MIT` |
| `calamine` | `0.24.0` | `MIT` |
| `uuid` | `1.24.0` | `Apache-2.0 OR MIT` |
| `rand` | `0.8.7` | `MIT OR Apache-2.0` |
| `sha2` | `0.10.9` | `MIT OR Apache-2.0` |
| `zeroize` | `1.9.0` | `Apache-2.0 OR MIT` |
| `base64` | `0.22.1` | `MIT OR Apache-2.0` |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` |
| `thiserror` | `1.0.69` | `MIT OR Apache-2.0` |
| `anyhow` | `1.0.102` | `MIT OR Apache-2.0` |
| `zip` | `0.6.6` | `MIT` |
| `encoding_rs` | `0.8.35` | `(Apache-2.0 OR MIT) AND BSD-3-Clause` |
| `sqlx` | `0.8.6` | `MIT OR Apache-2.0` |
| `chrono` | `0.4.45` | `MIT OR Apache-2.0` |
| `reqwest` | `0.11.27` | `MIT OR Apache-2.0` |
| `url` | `2.5.8` | `MIT OR Apache-2.0` |
| `lopdf` | `0.32.0` | `MIT` |
| `warp` | `0.3.7` | `MIT` |
| `hyper` | `0.14.32` | `MIT` |

### 7.2 `apps/vault-runtime-api` 直接第三方依赖

| 依赖 | 版本 | 许可证 |
|---|---|---|
| `bytes` | `1.12.1` | `MIT` |
| `cfb` | `0.7.3` | `MIT` |
| `chrono` | `0.4.45` | `MIT OR Apache-2.0` |
| `csv` | `1.4.0` | `Unlicense/MIT` |
| `once_cell` | `1.21.4` | `MIT OR Apache-2.0` |
| `serde` | `1.0.228` | `MIT OR Apache-2.0` |
| `serde_json` | `1.0.150` | `MIT OR Apache-2.0` |
| `sha2` | `0.10.9` | `MIT OR Apache-2.0` |
| `sqlx` | `0.8.6` | `MIT OR Apache-2.0` |
| `tempfile` | `3.27.0` | `MIT OR Apache-2.0` |
| `thiserror` | `1.0.69` | `MIT OR Apache-2.0` |
| `tokio` | `1.52.4` | `MIT` |
| `uuid` | `1.24.0` | `Apache-2.0 OR MIT` |
| `warp` | `0.3.7` | `MIT` |

### 7.3 Rust 间接依赖概况

当前机器生成结果显示：

- `src-tauri` 依赖图共 `724` 个包
- `apps/vault-runtime-api` 依赖图共 `356` 个包

其中 `src-tauri` 依赖图的主要许可证分布为：

- `MIT OR Apache-2.0`： `323`
- `MIT`： `175`
- `Apache-2.0 OR MIT`： `64`
- `MIT/Apache-2.0`： `44`
- `Unicode-3.0`： `18`
- `Apache-2.0`： `10`
- `MPL-2.0`： `8`

### 7.4 Rust 风险点

当前需要重点关注的 Rust 间接依赖主要是：

- `cssparser` / `cssparser-macros`： `MPL-2.0`
- `selectors`： `MPL-2.0`
- `webpki-roots`： `MPL-2.0`
- `r-efi`： `MIT OR Apache-2.0 OR LGPL-2.1-or-later`

当前判断：

- 这些条目目前未构成阻断 Apache-2.0 发布的直接冲突
- 但若未来修改了 MPL 文件本身，或在特定静态打包场景中引入额外义务，仍需复核
- `r-efi` 由于存在 permissive 备选许可，可按 `MIT` / `Apache-2.0` 路径理解，不视为当前阻断项

## 8. Python OCR 直接依赖

来源：

- `src-tauri/scripts/requirements-ocr.txt`
- `compliance/generated/python-ocr-direct-dependencies.tsv`
- `compliance/generated/python-ocr-pip-show.txt`

### 8.1 直接依赖

| 依赖 | 版本 | 许可证判断 | 结论 |
|---|---|---|---|
| `PyMuPDF` | `1.28.0` | `AGPL-3.0` 或商业许可 | **高风险** |
| `Pillow` | `12.3.0` | `MIT-CMU` | 可接受 |
| `easyocr` | `1.7.2` | `Apache-2.0` | 可接受 |
| `torch` | `2.13.0` | `Apache-2.0` + bundled permissive notices | 可接受，但需保留 notices |
| `torchvision` | `0.28.0` | `BSD` | 可接受 |
| `opencv-python-headless` | `5.0.0.93` | `Apache-2.0` | 可接受 |
| `scikit-image` | `0.26.0` | 以 `BSD-3-Clause` 为主，含 `BSD-2-Clause`、`MIT` 文件 | 可接受 |
| `scipy` | `1.17.1` | 以 `BSD-3-Clause` 为主，wheel 可能带额外 notices | 需注意 wheel 内容 |
| `Shapely` | `2.1.2` | `BSD-3-Clause` | 可接受 |
| `python-bidi` | `0.6.11` | LGPL 家族 | **中风险** |
| `pyclipper` | `1.4.0` | `MIT` | 可接受 |
| `numpy` | `2.4.6` | `BSD-3-Clause` 与 bundled permissive notices | 可接受 |

### 8.2 OCR 风险判断

#### 高风险项： `PyMuPDF`

`PyMuPDF` 当前声明为：

- `AGPL-3.0`
- 或单独商业许可

这意味着：

- 若只是仓库中存在可选安装说明，尚不等于仓库整体必须改为 AGPL
- 但若你对外分发“已内置 PyMuPDF 的镜像、安装包、客户运行环境”，就不能把它当作普通 permissive 依赖处理
- 如需对外分发 OCR-enabled 产物，应在以下路线中二选一：
  - 获取对应商业许可
  - 替换为许可证更适合当前主线的 PDF / OCR 方案

#### 中风险项： `python-bidi`

当前元数据和公共发布页显示其处于 `LGPL` 家族语境。虽然这通常不直接阻断产品使用，但需要结合实际打包和分发方式判断：

- 是否仅作为 wheel 的独立组件存在
- 是否与其他部分形成需要额外履约的 bundled 形式
- 是否在客户交付或镜像中被一起分发

#### Wheel-level notice 风险： `scipy`

当前 `scipy` wheel 元数据中包含额外 bundled notices，涉及：

- `OpenBLAS`
- `LAPACK`
- `GCC runtime library`
- `libquadmath`

因此：

- 若你只是记录源码依赖，可保留为风险提醒
- 若你打包并分发 OCR venv、Docker 镜像或桌面安装包，则必须把 wheel 内的额外 notices 一并带出

## 9. 发布与交付要求

针对不同交付形态，建议最低要求如下：

### 9.1 源码仓库发布

- 根目录保留 `LICENSE`
- 根目录保留 `NOTICE`
- 保留 `DEPENDENCIES.md`
- 机器生成的依赖清单可随仓库一起发布

### 9.2 Docker 镜像发布

- 检查镜像里实际复制了哪些第三方组件
- 将镜像的 NOTICE 与第三方许可证文本打包到镜像或随附文档
- 若镜像内包含 OCR venv，必须完成 OCR 额外许可证审查

### 9.3 桌面安装包发布

- 安装包或 About 页面应能访问许可证与第三方声明
- 若包含 OCR、模型、字体、转换组件，需单独确认 bundled 内容的义务是否完整履行

### 9.4 客户交付包

- 必须包含当前交付物实际 bundled 内容对应的 NOTICE
- 不要只复用源码仓库中的 NOTICE 而不做分发差异检查
- 若合同承诺中出现“全部自有知识产权”之类表述，必须先由法务和合规负责人复核

## 10. 当前风险评级

| 风险项 | 级别 | 当前判断 | 建议动作 |
|---|---|---|---|
| JavaScript 主依赖 | 低 | 宽松许可证为主 | 常规维护 |
| Rust 主依赖 | 低 | 宽松许可证为主 | 常规维护 |
| Rust MPL 间接依赖 | 中 | 当前不阻断，但需关注修改义务 | 发布前复核 |
| `caniuse-lite` `CC-BY-4.0` | 低 | 主要为构建数据归属 | 保留记录 |
| `PyMuPDF` | 高 | OCR-enabled 分发存在明确合规风险 | 商业许可或替换 |
| `python-bidi` | 中 | LGPL 家族义务待结合分发确认 | 单独复核 |
| `scipy` wheel notices | 中 | wheel 内容可能带额外声明要求 | 分发时补 notices |

## 11. 后续维护要求

以下情况发生时，必须同步更新本文件：

- 新增或移除第三方依赖
- 更换 OCR 组件
- 新增桌面安装包或 Docker 交付方式
- 根许可证路线变更
- NOTICE 规则变更
- 仓库内本地 crate 的许可证元数据变更

建议在 CI 或发布流程中加入：

- Node 依赖许可证导出
- Cargo 依赖许可证导出
- Python OCR 依赖许可证导出
- SBOM 导出
- NOTICE 差异检查
