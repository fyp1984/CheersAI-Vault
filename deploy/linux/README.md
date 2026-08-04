# `deploy/linux/` — Linux 内网客户测试部署材料

本目录提供把已通过 Review 的 `apps/vault-runtime-api` Runtime 与仓库根目录
原 Vault React 前端，整理为 Linux 内网客户测试部署所需的模板与脚本。这是
一次性的隔离测试内网交付，**不是正式生产安全部署**（见下方“安全边界”）。

完整的部署与运维说明见 [`../../docs/enterprise/DEPLOYMENT.md`](../../docs/enterprise/DEPLOYMENT.md)
和 [`../../docs/enterprise/API_REFERENCE.md`](../../docs/enterprise/API_REFERENCE.md)；
本文件只覆盖本目录内文件的用途和最小上手顺序。

## 目录内容

| 文件 | 用途 |
|---|---|
| `vault-runtime-api.service` | systemd 单元模板（非 root 用户、EnvironmentFile、`Restart=on-failure`、`UMask=0077`） |
| `nginx-cheersai-vault.conf` | Nginx 站点模板（`/` 托管原 Vault 前端 dist/，`/api/` 反代 Runtime，默认仅 loopback，需显式加客户 CIDR） |
| `runtime.env.example` | Runtime 环境变量模板（Linux 部署实际需要设置的子集，含 FileBay 浏览器上传四项） |
| `smoke-test.sh` | 黑盒 smoke 测试脚本，只用虚构数据验证提交→轮询→失败信息→下载闭环 |

## 架构

```text
浏览器 / 企业内网系统
        ↓
Nginx（内网地址 + 客户 CIDR 白名单，唯一入口）
  ├─ /          → 仓库根目录原 Vault 前端 dist/（pnpm exec vite build 产物）
  └─ /api/      → 127.0.0.1:8787 vault-runtime-api（只监听 loopback）
```

## 最小上手顺序（隔离 Linux 主机/虚拟机/容器）

1. **构建**：
   ```bash
   cargo build --release --manifest-path apps/vault-runtime-api/Cargo.toml
   pnpm install --frozen-lockfile
   pnpm exec vite build
   ```
2. **Runtime**：按 `vault-runtime-api.service` 顶部注释创建服务账户、数据
   目录、环境变量文件（参考 `runtime.env.example`），安装 systemd 单元并
   启动。
3. **Nginx**：把 `nginx-cheersai-vault.conf` 中的占位符替换为真实监听地址、
   `server_name`、前端 `dist/` 绝对路径、客户 CIDR，`nginx -t` 验证后
   reload。
4. **验证**：
   ```bash
   ./deploy/linux/smoke-test.sh http://127.0.0.1:8787/api/v1        # 直连 Runtime
   ./deploy/linux/smoke-test.sh http://<Nginx内网地址>/api/v1        # 经 Nginx
   ```
   两次都应打印 `ALL SMOKE CHECKS PASSED` 并以退出码 0 结束；任何一步失败
   都会打印 `FAIL: ...` 并以非零退出码结束。
5. **浏览器**：从客户 CIDR 白名单内的主机访问 Nginx 地址，确认原 Vault 页面
   可打开、批量脱敏流程可运行，浏览器控制台无新增错误。
6. **停止验证**：`systemctl stop vault-runtime-api` 后确认监听端口已释放
   （`ss -ltnp | grep <端口>` 应无输出），`journalctl -u vault-runtime-api`
   应看到 Runtime 自行打印的 SIGTERM 接收与停止日志。
7. **重启持久化**：重新 `systemctl start vault-runtime-api`（使用同一数据
   目录），确认此前批次/文件状态/产物仍可通过
   `GET /api/v1/batches/{batch_id}` 和 `GET /api/v1/artifacts/{artifact_id}`
   读取。
8. **FileBay（可选，按需启用）**：在 `runtime.env.example` 中填写
   `VAULT_FILEBAY_URL`/`VAULT_FILEBAY_TOKEN`/`VAULT_FILEBAY_OWNER`/
   `VAULT_FILEBAY_REPO` 四项后重启 Runtime（启动时只读取一次，无热加载），
   再在浏览器 `/gitea` 页确认状态为“已配置”，测试连接并按需创建固定私有
   仓库；随后可在 `/files` 的已完成批次中勾选脱敏 Markdown 并确认上传
   （详见下文“FileBay 浏览器上传（单系统用户 MVP）”）。

## FileBay 浏览器上传（单系统用户 MVP）

本版本按**单一系统用户**交付：一台 Runtime 对应一个系统身份、一个受信任
服务器工作区、一套由管理员配置的 FileBay 目标（固定 HTTPS 私有仓库）和一个
共享 PIN。所有能访问该内网页面的浏览器会话共享这些服务器状态——它们**不构成
登录、RBAC、管理员/普通用户区分或多租户隔离**，页面与文档不得作此宣称；
“仅企业内网、不暴露公网”的边界不变。

### 管理员配置

1. 四项变量（`VAULT_FILEBAY_URL`/`VAULT_FILEBAY_TOKEN`/`VAULT_FILEBAY_OWNER`/
   `VAULT_FILEBAY_REPO`）必须**同时完整**配置，Runtime 只在**启动时读取一次**，
   修改后必须**重启**才生效。
2. Token 只写入部署管理员维护的 Runtime 环境文件（`runtime.env.example` 复制
   出的真实文件），文件权限 `0600`、属主为 Runtime 服务账户，systemd 继续使用
   `UMask=0077`；该环境文件**不得提交版本库**，也不得放入 Nginx Web root。
3. 目标必须是**裸 HTTPS 根 origin**；浏览器不能修改 URL、Token、owner 或 repo。
4. 配置状态语义（与实现一致）：
   - 四项全部未设置 → `未配置`（`unconfigured`）：FileBay 相关动作被禁用，但
     Runtime 的脱敏、OCR、恢复、日志、沙箱/PIN 和客户 API 都正常运行。
   - 四项全部设置且校验通过 → `已配置`（`configured`）。
   - 只设置部分、或 URL/owner/repo 非法 → `配置无效`（`invalid`）：FileBay
     写入类动作一律安全失败，返回固定错误码，不展示底层响应正文、Token 或
     服务器路径。

### 浏览器操作

- `/gitea`：只展示安全状态（未配置/已配置/无效 + 目标地址 + 仓库 + 是否已配
  Token），允许用户**显式**发起“测试连接”和“创建私有仓库”（仓库固定为**私有**）。
- `/files`：只有 Runtime 确认状态为 `Completed` 的脱敏 Markdown 才出现在上传
  候选列表；确认弹窗展示目标 origin、owner/repo、远端路径与“只上传脱敏
  Markdown、不上传原文/`.cmap`/还原产物”的提示，浏览器只提交
  `artifact_ids`，远端路径由服务器生成；上传结果逐文件返回成功/失败与错误码。
- 页面加载、处理完成、下载、恢复、取消确认都**不会**自动上传或自动访问
  FileBay；上传只能由用户在确认弹窗中主动点击“确认上传”触发。
- 沙箱/PIN 与 FileBay 一样是单 Runtime 共享的服务器状态：一个服务器沙箱目录、
  一个共享 PIN、一套 FileBay 配置；共享 PIN 只保护沙箱操作，**不是登录凭据或
  用户身份**。

## 安全边界（必读，勿省略）

- **本版本没有应用层鉴权**：没有 API Key、登录、会话、权限体系。唯一的
  访问控制是 Nginx 的客户 CIDR 白名单。任何能连通 Nginx 监听地址的主机
  都能调用全部四项 API（见 API_REFERENCE.md）。
- Nginx 模板默认只放行 `127.0.0.1`；**部署管理员必须**在
  `nginx-cheersai-vault.conf` 中显式添加真实客户网段，模板本身不含、也
  不应被改成 `allow all;` 或 `0.0.0.0/0`。
- Runtime 固定只监听 `127.0.0.1`；不要尝试通过环境变量或命令行参数让它
  监听其他网卡或 `0.0.0.0`——本版本不提供这样的开关。
- CORS（`VAULT_RUNTIME_CORS_ORIGINS`）只影响浏览器同源检查，**不是接口
  鉴权**，不能替代 Nginx CIDR 白名单。
- 数据目录（SQLite、原始输入、脱敏产物、映射文件）必须与 Nginx Web root
  物理隔离，绝不能放进前端 `dist/` 目录或任何 Nginx 静态目录下。
- FileBay Token 只存在于部署管理员维护的 Runtime 环境文件中：环境文件
  `0600`、属主为服务账户、systemd `UMask=0077`、不提交版本库、不放 Web root；
  浏览器从不读取/修改 Token（状态接口只返回是否存在 Token 的 `has_token`
  布尔值与必要目标元数据，不返回 Token 本身），Token 也不得写入
  命令行参数、URL、前端、截图、日志或浏览器 storage。
- FileBay 只上传 Runtime 确认为 `Completed` 的脱敏 Markdown 且由用户在确认
  弹窗中主动确认；原文、`.cmap`、反脱敏还原产物、未完成产物、任意本地路径
  与自动上传一律禁止。
- `smoke-test.sh` 只使用脚本内生成的虚构手机号/邮箱，不读取、不上传任何
  真实文件；请勿改用真实客户数据运行本脚本。

## 已知限制 / 开放问题

- 本目录不含 TLS/HTTPS 配置、日志轮转、监控告警、备份恢复模板——这些不
  在本次客户测试范围内。
- 客户的真实 IP、域名、CIDR、Linux 发行版信息未知，全部模板用占位符
  表示，不得凭空猜测填入。
- 本目录的部署与运维说明已经按当前已通过的浏览器 FileBay Runtime 适配和
  单系统用户口径更新：FileBay 与沙箱/PIN 均为单 Runtime 共享的服务器状态，
  不构成登录、RBAC 或多用户隔离。**真实 Linux 实机与真实 FileBay 尚未验收**：
  本文档的所有 Linux systemd/Nginx 步骤与 FileBay 远端闭环都需要在提供隔离
  Linux 主机与专用测试凭据后另行独立验收，不得把本机 macOS 或 fake 测试结果
  当作真实 Linux/真实 FileBay 已通过的证据。
