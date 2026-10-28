# Motrix 维护手册

开发、发布或遇到问题时按目录查阅。项目结构见 [架构](architecture.md)，接口字段见 [API 契约](api-contract.md)，当前未完成事项见 [开发计划](future-development-plan.md)。

## 目录

- [开发与验证](#开发与验证)：命令、测试边界、清理
- [固定引擎候选测试](#固定引擎候选测试)
- [打包与发布](#打包与发布)：构建、版本、Actions
- [实机验收](#实机验收)
- [平台依据与未验证边界](#平台依据与未验证边界)
- [统一网关实验](#统一网关实验)
- [备份与回滚](#备份与回滚)
- [故障排查](#故障排查)：日志、登录、启停、下载
- [远程接入](#远程接入)：Lucky / Cloudflare 与 JSON-RPC

## 开发与验证

在仓库根目录执行命令，先 `pnpm install`。Node.js 以 `.node-version`、pnpm 以 `package.json` 的 `packageManager`、Rust 以 `rust-toolchain.toml` 为准；CI 使用 `pnpm install --frozen-lockfile`。命令的唯一清单是 `package.json`，变更行为时同步更新本手册。

### 命令速查

表内命令均省略 `pnpm run` 前缀。写入版本、发布或清理前先检查工作区。

| 命令 | 用途与副作用 |
| --- | --- |
| `dev` | 只启动 Vite，不启动后端；API/SSE 经开发代理连接另行启动的 Rust server |
| `typecheck` | 只运行 Vue / TypeScript 类型检查，不生成 Web 产物 |
| `build` / `preview` | 类型检查后构建 `dist/`；preview 预览已有产物，不验证 fnOS 行为 |
| `test:unit` | 前端独立 `*.spec.ts`，可能写测试缓存 |
| `test:scripts` | Node 脚本测试，使用临时目录，不修改真实版本与发布状态 |
| `verify:pre-commit` | 版本一致性、Rust 格式检查；hook 另查暂存区空白 |
| `verify` | 强制执行一次完整源码测试与编译，包含前端类型检查及生产构建；本地成功后保存可复用记录，保留编译缓存 |
| `test:e2e` | Chromium 桌面 / 移动冒烟，写报告与失败 trace |
| `test:aria2` | 真实 sidecar 测试，使用临时下载目录、session |
| `test:filesystem` | Rust 文件系统故障专项，使用临时目录和 Rust 缓存 |
| `test:stability` | 运行完整稳定性专项：30 次正常退出、崩溃恢复、连接资源采样和真实退出超时；使用临时数据和端口 |
| `verify:extended` | 完整 verify 后运行稳定性专项、E2E 与真实 sidecar 测试，不重复文件系统专项 |
| `verify:fpk` | 解包检查已有且版本匹配的双架构 FPK，缺产物直接失败 |
| `audit:deps` | 锁文件生产依赖审计，不自动升级；需 `cargo-audit 0.22.2`，高危 / 严重或工具错误时失败，中低危报告 |
| `clean:dry-run` / `clean` | 预览 / 删除前端与 FPK 生成物，详见下方清理范围 |
| `clean:rust` | 删除整个 `server/target/`，下次 Rust 构建需完整重建 |
| `version:check` / `version:set <version>` | 检查 / 同步四个版本源；set 不构建、不写日志、不建 commit/tag |
| `release:notes <x.y.z> [--body]` | 只读提取并校验 CHANGELOG，`--body` 只输出正文 |
| `release:prepare <x.y.z>` | 改版本和 CHANGELOG、创建中文提交与 tag；详见发布步骤 |
| `prepare` / `hooks:install` | 配置本仓库 `core.hooksPath=.githooks`；prepare 安装依赖时自动尝试，失败不阻断安装，显式安装失败则报错 |
| `build:server:linux:x64` | cargo-zigbuild 交叉编译 x86 release server，glibc baseline 为 `2.36` |
| `build:web:fpk` | 只做 Vite 构建，清空并重建 `packaging/fnos/app/ui/dist/`，不运行类型检查 |
| `assets:aria2:fetch` | 联网获取固定引擎校验和与缺失二进制，校验 SHA-256 并设执行权限 |
| `stage:aria2:x64` / `stage:aria2:arm64` | 复制 sidecar 到共享 `app/bin/aria2-next`，后一次覆盖前一次 |
| `build:fpk:x64` / `build:fpk:arm64` | 单架构编译、Web 构建、staging、预检及 fnpack；默认清空已有 FPK 输出 |
| `build:fpk:prepare` | 双架构编译、Web 构建一次、独立 staging 与静态预检，不生成 FPK |
| `build:fpk:artifacts` | 仅构建双架构产物，清空输出目录；供 Release 使用，不跑源码测试或类型检查 |
| `build:fpk` | 核对或执行完整 verify → 复用已验证 Web 产物 → 双架构打包 → verify:fpk |
| `build:fpk:release` | 扩展 verify → 复用 Web 产物 → 双架构打包 → verify:fpk |

Rust 后端单独测试：`cargo test --manifest-path server/Cargo.toml`。完整验证成功时压缩输出，失败保留详情；非交互长步骤每 30 秒报告进度。

日常 Rust 验证保留两次正常启停、崩溃恢复、任务与原生状态保留、SSE 退出和端口释放检查；完整重启次数、连接压力和 10 秒真实退出超时由 `test:stability` 执行。每周 / 手动 CI 与本地发布前 `build:fpk:release` 通过 `verify:extended` 包含该专项，不删除原有断言。仅修改该专项或其夹具的推送也会执行 `test:stability`。

### 验证触发与边界

本地推送和 `build:fpk` 使用 `verify --reuse`，按前端、后端、脚本检查三组核对 `.cache/verification.json`。文件用途分类由推送 hook 与缓存共用，先识别运行输入，再判断文档类型；不能仅按扩展名忽略资源。

| 改动 | 重新执行的验证 |
| --- | --- |
| README、普通说明文档 | 版本一致性和现有日志文档断言；复用三组业务结果 |
| 前端代码、测试、样式、网页资源、`CHANGELOG.md` | 前端和脚本检查，后者包含主题、样式及测试分离守卫 |
| Rust 代码、测试、依赖、工具链 | 后端和相关脚本检查 |
| 打包图标与配置 | 脚本检查；`common.sh`、`install_callback`、`reset-web-auth` 及 Aria2 资产同时影响后端验证 |
| 公共依赖、构建 / 验证脚本或未识别文件 | 三组完整验证 |

各组摘要覆盖 Git 跟踪及未忽略的新文件的实际内容、权限、增删、相关依赖安装记录、工具链与环境；`RUSTFLAGS` 等编译参数参与核对，`RUST_LOG` 日志设置不影响复用；生产 `.env` 属于前端输入，Web 产物缺失或改变只使前端记录失效。未提交改动同样参与核对，Git 提交本身不使内容相同的结果失效。重跑前清除对应组记录；验证失败或期间输入变化时不记录成功，无关组记录保留。旧格式或损坏记录首次完整重验。删除 `.cache/verification.json` 可清除全部复用记录。

每次仍核对版本，复用脚本检查时单独运行日志文档断言，避免文档规则漏验或重复运行。`pnpm run verify`、CI 和发布前扩展验证始终完整运行，FPK 每次重新组装并验收。

- 提交 hook：暂存区空白检查后，只有文档 / 媒体资源时跳过版本与 Rust 格式检查，删除资源也同样处理；混有代码或没有暂存文件时执行快速检查。Markdown 允许两个行尾空格换行。
- 推送 hook：按远端旧提交到本次推送提交的完整差异判断，包含删除和重命名前的路径；多个分支合并判断，不能仅检查最后一个 commit。
  - 普通说明文档或无文件变化时跳过业务验证；维护手册和接口文档改动执行已有日志文档断言。网页图片、字体、被程序读取的 Markdown 及打包资源按实际用途验证。只推 tag 或删除远端引用也跳过。
  - 只有测试及其专用夹具时，按类别运行前端、Rust、脚本、浏览器、sidecar 或变更的 Shell 测试，不做完整构建；同一类别只运行一次。
  - 包含业务源码、运行资源、依赖、配置、构建脚本或未识别文件时执行一次 `verify --reuse`，只重跑失效组；首次推送新分支或远端旧提交在本地不可读时同样核对全部组，缺少有效记录则完整验证。
  GitHub `Verify` 对面向 `main` / `develop` 的 PR/push 继续执行完整源码门禁。
- 每周定时或手动 `Verify` 执行扩展验证；定时为北京时间周日 02:17，调度可能延迟。首次本地 E2E 或升级 Playwright 后执行 `pnpm exec playwright install --with-deps chromium`，CI 已配置安装。
- `Dependency Audit` 独立在每周一北京时间 03:23 或手动执行，不属于 verify 或 Release。

| 检查层 | 能说明什么 | 不能代替什么 |
| --- | --- | --- |
| Rust、前端、脚本、Shell、类型检查与构建 | 业务逻辑、API、SQLite、路径、进程模拟、组件与发布脚本的回归 | fnOS 平台生命周期、设备权限与完整真实网络 |
| Playwright | 桌面 / 移动 Chromium 的登录、任务及 SSE 交互 | API 是 fixture，不连接真实 Rust server 或下载引擎，也不是 WebView 验收 |
| 真实 sidecar | HTTP fixture、BT / 磁链 metadata、暂停续传、session、异常退出与代理组合 | 默认使用 Linux 内置二进制，非 Linux 跳过；跳过不算通过 |
| 文件系统故障测试 | 暂存、删除、恢复、重下载、回滚及后台清理 | NAS 真实共享目录、撤权和操作系统权限差异 |
| `verify:fpk` | 双架构包内容、入口、权限、二进制和无运行残留 | 安装器、向导、真实启动升级与回滚 |

测试实现和目录规则见 [AGENTS.md](../AGENTS.md#测试文件)。只读 / 空闲文件稳定性由 `server/tests/idle_file_stability.rs` 回归；其他测试入口按领域查找 `tests.rs`、`server/tests/`、前端 `*.spec.ts` 和 `scripts/tests/`。当前没有覆盖率门槛，数量不代表覆盖率，待补项目见 [后续测试](future-development-plan.md#后续测试补齐)。

### 固定引擎候选测试

用 `MOTRIX_ARIA2_CANDIDATE_DIR` 指向隔离目录，不替换仓库内置资产。目录放宿主对应的 `aria2-next-2.8.6-<平台>-<架构>`，入口核对文件名和固定 hash；候选模式支持 Linux/macOS。Node.js 24+，Rust 专项还需工具链；Linux 矩阵需 `ldd`、GNU `readelf` 及运行库，并以非 root 用户验证权限。

```bash
rtk proxy env MOTRIX_ARIA2_CANDIDATE_DIR=/path/to/verified-candidate node --test \
  --test-name-pattern='290-14|290-02 (HTTP retains|SIGKILL recovery|magnet keeps|uploaded torrent)' \
  scripts/extended/aria2-sidecar.test.mjs
rtk proxy env MOTRIX_ARIA2_CANDIDATE_DIR=/path/to/verified-candidate \
  cargo test --manifest-path server/Cargo.toml --test native_engine_safety -- --ignored --nocapture
```

Linux 运行库、服务 UID 和代理专属用例只在 Linux 执行；macOS 通过不代表 Linux 通过。Rust 安全专项经过真实管理 handler 和引擎 RPC，但不经过 TCP 管理 listener、浏览器或 SSE。使用临时目录和回环 fixture 不等于完成完整网络泄漏检查。异常 Content-Range 的已知失败及未验项目见 [引擎升级限制](future-development-plan.md#已知限制与延期)，不得将组合命令记为全部通过。

### 清理生成物与缓存

1. 先执行 `pnpm run clean:dry-run`，核对范围。
2. `pnpm run clean` 删除根 `dist/`、FPK 临时 server / sidecar / 入口图标 / Web 资源、`.stage/`、FPK 输出、旧固定名称 FPK 及忽略目录外的 `.DS_Store`；保留源码、实机 SQLite、`assets/aria2/` 源资产及 Rust target。
3. 磁盘紧张才执行 `pnpm run clean:rust`，删除整个 `server/target/`；验证不会自动清理编译缓存。

GitHub `Verify` 只缓存 pnpm store 和 Cargo registry，不缓存 `server/target`，以控制占用。`Cleanup Actions Caches` 手动运行默认删除全部缓存，`scope=non-main` 只删非 main 缓存；不影响源码和 Release，只增加下次下载 / 编译时间。

## 打包与发布

### 本地构建

1. 安装 Rust/rustup、两个 Linux GNU target、`cargo-zigbuild` 和 Zig。完整打包需 `fnpack`，未找到时脚本下载已验证的 `1.2.1`。
2. 普通完整打包执行 `pnpm run build:fpk`；发布前需扩展验证时用 `build:fpk:release`。只预检用 `build:fpk:prepare`，它仍会完整编译，不是轻量 lint。
3. 检查 `packaging/fnos/.stage/x86/`、`.stage/arm/` 的 manifest、`MotrixFNOS.sc`、`app/ui/config`、生命周期脚本和二进制架构；两个 `app/data/` 都应为空。
4. 最终产物为 `packaging/fnos/dist/motrix_<version>_x86.fpk` 和 `motrix_<version>_arm.fpk`。x86_64 / ARM64 设备分别安装匹配包，不能交叉替代验收。

server 位于 `server/target/<target>/release/motrix-fnos-server`；target 为 `x86_64-unknown-linux-gnu` 或 `aarch64-unknown-linux-gnu`。真实 fnpack 输入是 `.stage/<target>/`，不要直接对源码态 `packaging/fnos/` 执行 fnpack。

Release 使用 Thin LTO，并保留单编译单元以控制二进制体积。构建参数改变后的首轮交叉编译会重建对应 Release 缓存，后续仍由 Cargo 复用；日常清理保留 `server/target/`。

构建会清空源码 staging 的 `packaging/fnos/app/data/`，该目录只放占位内容，不能保存测试数据。`.stage/`、`dist/`、临时二进制、图标和 Web 产物不提交；`assets/aria2/aria2-next-*` 是 sidecar 源资产，不能当生成物删除。双架构以独立 stage / 最终包为准，不能根据共享 `app/bin/` 判断。

引擎联网维护入口为 `assets:aria2:fetch`，固定资产在 `scripts/build/fetch-aria2-next.mjs`、`assets/aria2/aria2-next-2.8.6-checksums.sha256` 和 `assets/aria2/aria2-next-2.8.6-dependencies.env`。升级需审查版本、hash、双架构字节、许可证与设备兼容性，不是每次构建都下载。支持边界见 [应用信息](api-contract.md#42-应用信息)。

### 版本与本地升级包

四个版本源为 `package.json`、`server/Cargo.toml`、`packaging/fnos/manifest.template` 和 `packaging/fnos/app/ui/config` 的缓存参数。格式接受 `x.y.z` 与 `x.y.z-beta`；GitHub 正式发布只接受前者。Cargo 构建可能同步更新 `server/Cargo.lock`，须检查版本 diff。

本地测试使用 beta 链：确认工作区 → `pnpm run version:set <下一版本>-beta` → `version:check` → `build:fpk` → 实机安装 → `version:set <下一版本>` 验证升级到正式版。同一 beta 不能重复安装，不自动递增，不为本地测试创建正式 tag / Release。

### GitHub Actions 发版

先完成 `feature -> develop -> main` 的测试与发布验收，再从验收后的 main 打开 **Actions → Release FPK → Run workflow → 输入 x.y.z**。

workflow 在同一次运行内准备 CHANGELOG 和版本、更新 Cargo.lock、构建并解包双架构 FPK、生成 SPDX SBOM 与 SHA256SUMS、生成 provenance/attestation、提交中文版本记录、建立 `v<x.y.z>` tag 并发布 Release，不依赖另一个 tag workflow 或自动批准 PR。

- Release 不重复源码或扩展测试，须有同一业务源码的 verify 证据；本地完整打包仍按上方命令执行源码验证。
- 发布白名单为 `CHANGELOG.md`、`package.json`、`server/Cargo.toml`、`server/Cargo.lock`、`packaging/fnos/manifest.template`、`packaging/fnos/app/ui/config`，出现其他改动立即中止。自动版本提交和内部推送使用 `--no-verify`，防止重复 hooks。
- 上传两个 FPK、各自的 `.fpk.spdx.json` 和 `SHA256SUMS.txt`，并核对构建证明。
- 已有合法目标 CHANGELOG 时直接复用；否则 Cloudflare Workers AI 分批提取 commit subject/body 与本地文件统计，再归并、编辑、独立复核。模型、权限、配额或正文结构错误时中止，可人工预写合法条目后重试，不静默回退逐 commit 日志。

Cloudflare 配置：

| 配置 | 要求 |
| --- | --- |
| Actions Secret `CLOUDFLARE_ACCOUNT_ID` | 32 位 Account ID，不写入仓库 |
| Actions Secret `CLOUDFLARE_API_TOKEN` | 目标账户的 Workers AI Read；Actions 不读 Gateway Logs API |
| AI Gateway | ID 为 `motrix-fnos-release`，Workers AI 使用 Standard billing |
| Gateway 维护 | 创建 / 修改用另一个临时 Token，另授 AI Gateway Edit |
| 分析 / 编辑模型 | 默认均为 `@cf/openai/gpt-oss-120b`；临时调整 workflow 的 `MOTRIX_RELEASE_ANALYSIS_MODEL` / `MOTRIX_RELEASE_EDITOR_MODEL` |
| 请求记录 | 强制保留调用元数据、关闭 payload 存储；凭证不入仓库、日志或产物 |

Job summary 展示版本、CHANGELOG、下载链接、hash 与 SBOM；脚本不读取 Gateway Logs，也不推算 AI 余额，用量以 Workers AI Dashboard 为准。

### 本地发版备用

Actions 异常时，先检查工作区并预写 / 审阅目标 CHANGELOG：

```bash
pnpm run release:prepare <x.y.z> --dry-run
pnpm run release:prepare <x.y.z>
git push --atomic origin HEAD v<x.y.z>
```

prepare 拒绝无关脏工作区，目标必须是更高正式版本；复用现有日志，否则未配置 provider 时生成确定性分类草稿，然后同步版本、提交、打 tag。它不自行完整验证，单次原子推送触发一次 pre-push。`--from <tag>` 可指定起点；`--no-commit` / `--no-tag` 仅供受控排查。

本地用 AI 时设置 `MOTRIX_RELEASE_CHANGELOG_PROVIDER=cloudflare-workers-ai`、`CLOUDFLARE_ACCOUNT_ID`、`CLOUDFLARE_API_TOKEN`，需要 Gateway 时再设 `CLOUDFLARE_AI_GATEWAY_ID=motrix-fnos-release`。本地命令不代替 Actions 权限、双架构上传和发布检查。

### FPK 身份与图标

- 配套身份为 `appname=motrix`、`desktop_appname=motrix.Application`、空 `desktop_applaunchname`，`app/ui/config` 的 `.url` 只有同名 `motrix.Application` iframe 入口。不能只改其中一个字段；空 launchname 时构建器须验证唯一入口。
- `service_port` 与入口 port 为管理端口 `17080`，入口 `protocol=http`、`url=/?v=<version>`，Web 基址保持 `./`。`MotrixFNOS.sc` 为 `17080/tcp,17082/tcp`；回环 `17081` 不进入 manifest、桌面入口或平台端口映射。运行时限制见 [接口约定](api-contract.md#1-运行时约定)。
- 该身份的 FN Connect 地址 `https://motrix.<account>.fnos.net/` 已有实机依据。桌面入口默认 `allUsers=false`，`control.accessPerm=editable` 可由管理员放开；不因此获得可信 `X-Trim-*` 身份。
- 双架构 `os_min_version=1.2.0401`，不承诺支持 `1.2.0302`；正式包声明 `micro_app=true`，精确申请 `trim.file.sharedAccess`、`trim.file.path`。预检拒绝额外 Scope，扫描 Web 产物，禁止 `TRIM_API_TOKEN`、官方 Socket 路径和官方开放 API 的 Authorization Header 拼装代码进入产物。
- 包根 `ICON.PNG` / `ICON_256.PNG`、入口 `app/ui/images/icon_64.png` / `icon_256.png` 及 Web `icon.png`，项目交付均为 **256×256**，由构建器同步校验；保留 `images/icon_{0}.png` 兼容文件名。
- 官方规范仍为包图标和入口图标各有 64 / 256 两档，并要求正方形、sRGB、PNG/JPG、单文件不超过 1024 KB；全用 256 是本项目 2026-07-17 双架构 FPK 实机显示验证后的交付选择。模糊时先看包内尺寸，再强刷 / 清缓存 / 换浏览器，不只改一个图标。

### 实机验收

本地构建、ARM/x86 实机、公网链路是不同结论。记录 fnOS / App 版本、设备架构、FPK SHA-256 和脱敏证据；没有证据继续标待验。当前待验范围由 [开发计划](future-development-plan.md)维护，不在这里记录历史通过数量。

| 场景 | 操作及预期 |
| --- | --- |
| 安装 / 启动 / 停止 | 匹配架构安装；管理页可打开、ready 返回 200；停服后 server 和 sidecar 退出，PID 清理。观察应用中心任务日志、`TRIM_TEMP_LOGFILE`、lifecycle / server 日志 |
| 安装密码 | 新装即能登录、不匹配输入报错；保留数据重装 / 升级沿用密码；SSH 重置、Ctrl+C 取消、终端回显恢复和应用账户数据库写权限正确 |
| 平台密码字段 | 仅用测试密码核查向导字段是否进入平台配置、安装记录或日志；目前官方资料不足以证明平台不会保留字段 |
| 快速重新启用（ARM/x86，待验） | 建 IPv6 连接后停用、立即启用，分别覆盖引擎运行和未运行；不因 TIME_WAIT 报绑定失败，真实进程占用仍须拒绝 |
| 中文 BT（ARM/x86，待验） | 全新中文种子 / 磁链、多层路径、非 ASCII 保存目录，暂停续传、重启和可选删文件；实际路径与记录一致，任务外层目录唯一，清理后不留残余，子进程 `LANG=C.UTF-8` |
| 官方授权 API（x86） | 正式 motrix 身份、fnOS 1.2.0401+ 管理员宿主下授权、刷新、建任务、语义路径及完成文件操作；覆盖普通用户拒绝、撤权、空授权、Token/Socket 不可用及独立浏览器降级 |
| ARM / 飞牛 App WebView | 重复正式身份的授权与文件操作，检查主题、语言、文件选择、SDK、前后台与断网恢复 |
| 配置变更 | 保存普通设置不回写官方授权快照；查看 `accessible-paths.json` 与日志 |
| 升级 / 回滚 / 重启 | 同身份按当前代际规则保留数据与凭据，旧身份不自动迁移；按下节完整备份回滚；应用与设备重启不丢恢复状态 |
| 卸载 | 默认保留应用数据；明确勾选删除时只清应用私有目录，两种情况均不删外部下载文件 |
| 真实公网链路 | IPv4-only 与原生 IPv6 分别验证 DNS、TLS、Lucky、HTTP、CORS、WebSocket 和管理面隔离 |
| UI | 按 [UI 验收](design/DESIGN.md#验收)检查主题、语言、触控、长文本与滚动 |

重复安装可用 `appcenter-cli install-fpk <package>.fpk`、`appcenter-cli start motrix`、`appcenter-cli stop motrix`、`appcenter-cli list`；向导和交互配置优先在应用中心操作。

### 平台依据与未验证边界

沿用既有查证结论；修改平台实现前按 [查证规则](../AGENTS.md#fnos-平台查证)重新核实。

- [Manifest](https://developer.fnnas.com/docs/core-concepts/manifest/) 文档列 `platform=x86|arm|all`、最低系统版本和单个 service_port，没有文档化 `arch`。现保留 x86 stage 的 `arch=x86_64`，不可仅凭文档未写就删。
- [应用框架](https://developer.fnnas.com/docs/core-concepts/framework/)和 [fnpack](https://developer.fnnas.com/docs/cli/fnpack/)：本地 fnpack 1.2.1 最小工程验证缺 `cmd/main`、install/upgrade/uninstall 脚本会失败，缺 config_init/config_callback 仍可打包；`Packing failed` 可能退出码为 0，须同时校验日志和产物。升级 fnpack 时重验必需脚本、退出码、`--directory` 输出位置和 arch 字段。
- [应用入口](https://developer.fnnas.com/docs/core-concepts/app-entry/)及 Lucky 2.27.2 解包提供 Application 身份布局依据；端口入口已有 2026-07-14 ARM 实证。统一网关曾出现 Socket 直连 200、fnOS 入口 `404 9`，因此不直接替换正式拓扑。
- manifest 只描述单服务端口；`.sc` 多端口格式参考 [conversun/fnos-apps](https://github.com/conversun/fnos-apps) 的 AdGuardHome（3080/tcp、53/tcp、53/udp）和 Forgejo（3005/tcp、2223/tcp）声明实践，正式包防火墙、真实 LAN 对端和卸载清理仍须设备验收。
- [图标规范](https://developer.fnnas.com/docs/core-concepts/icon/)与本项目的高清交付约定分开理解，见上方身份与图标。
- [安装向导](https://developer.fnnas.com/docs/core-concepts/wizard/)和应用框架于 2026-10-07 查证：password 隐藏输入，field 映射生命周期环境变量，install_callback 在文件安装后执行；回调仍需重新校验，服务启动前清除密码环境变量，不把它传给 server / Aria2。应用不持久化或记录明文，平台保留行为待验。
- 卸载 switch 保持不设置 `initValue`：既有实测字符串不能可靠表达默认值，布尔值导致 fnpack 校验失败。`config_callback` 保持空操作，不再同步目录授权；config_init 需完成配置流程验证后才可评估删除。
- 升级框架于 2026-10-22 复核：upgrade_init / upgrade_callback 在升级前后执行，脚本须可重复；真实向导顺序与失败后平台是否自动回退尚未确认。

### 统一网关实验

延期实验只用独立 appname 的最小 FPK，不修改正式 Motrix 包，不阻塞修复或发布。依据：[官方网关注册](https://developer.fnnas.com/docs/core-concepts/gateway-registration/)。

1. 最小包只含 gatewayPrefix、位于 TRIM_APPDEST 的 Unix Socket、固定 HTML 根路由、固定 JSON 健康路由和 SSE / WebSocket 回显。
2. 脱敏记录请求 path 及 `X-Trim-Userid` / `X-Trim-Isadmin` / `X-Trim-Username` 是否存在；验证 trim_sac/open_gateway 注册，已登录请求 200，未登录拒绝。
3. 比较直连 Socket 与经 fnOS nginx 的结果，确认路径前缀保留或剥离；根 HTML、子路径、静态资源和长连接均须可用。
4. 覆盖停启、覆盖升级、卸载重装、设备重启，检查 Socket 与路由清理；记录 fnOS 版本、入口数据库、nginx access/error、注册日志和包 hash。
5. 全部实机通过后才提出迁移方案，更新架构、接口及本手册并明确端口回退；正式候选仍须验证最终包和真实网关链路，直连或 Router 测试不能代替。任一注册、认证、路径或长连接失败，继续当前端口入口。

## 备份与回滚

### 升级前备份与回滚

**2.0.0 不导入、展示、继续或恢复旧代任务，升级后列表重新开始。** 旧任务与旧操作不阻止升级，旧记录、session、恢复状态及下载文件保留，密码、设置、授权和 RPC Token 沿用。需要旧进度时先在旧版完成下载；应用不自动备份或迁移。

1. 在应用中心停止 Motrix，确认 server 和所属 sidecar 都已退出。
2. 完整备份 fnOS 实际提供的 `TRIM_PKGVAR`：一致的应用 SQLite、session、原生状态及种子备份；不要备份运行中的 SQLite / session 作为恢复源。外部下载目录另行保护，私有备份不能还原之后改动的外部文件。
3. 保存对应旧 FPK，记录原任务数量、设置、授权、session 校验值和 Token“是否配置”，不记录 Token 原文。
4. 安装新包，按开发计划核对任务代际、数据和凭据保留；先 NAS 本机验证 RPC，再调整 Lucky，需要 LAN 时另用真实客户端检查。
5. 回滚先停止新版，配套恢复旧 FPK 与升级前完整私有备份，并按旧版本恢复 Lucky 后端地址。不能让旧版直接读取新版数据库 / 引擎状态，也不能只改代理而继续运行不提供对应路由的新 server。缺备份时不承诺无损回退。

当前 native-v2 的 session 在 `$TRIM_PKGVAR/engine/native-v2/session/aria2.session`，原生状态在 `engine/native-v2/state/`，旧 `aria2/aria2.session` 不导入。FPK 不携带 upgrade-checker 或 server 副本；升级脚本只确认停服，正常启动做 schema 更新，只加载当前代际任务、操作和状态。

### 卸载与旧应用身份

- 同一 appname `motrix` 默认保留 `TRIM_PKGVAR`，保留数据重装沿用密码与设置。只有卸载向导明确开启 `MOTRIX_FNOS_DELETE_APP_DATA` 才清私有数据，外部下载文件不在范围内。
- 旧 `motrix.fnos` 改装为 `motrix` 是新应用，不自动迁移旧任务、设置或 Token；先停止或卸载旧应用，避免端口冲突。
- 卸载停服失败不能继续删数据；进程归属或退出无法确认时保留文件并报错。

## 故障排查

### 先从页面收集诊断

1. 无法登录：使用登录页“复制登录排障信息”和“下载登录诊断”，一起附在 Issue。内容包含访问 / 存储 / iframe 环境、脱敏 JWT 传输摘要及认证和生命周期日志尾部，不含凭据原文或下载内容。
2. 已登录：打开“诊断”查看版本、引擎与日志占用，80 MiB 起有文字预警。
3. 难复现时先开“详细日志（30 分钟）”，重现后立即导出诊断包；它临时切换 debug，到期恢复 warn，文件上限不变，引擎未运行时提示下次启动生效。
4. 清 Aria2 文件日志需先停止引擎、二次确认；运行中、切换中或归属未知时返回 `409 aria2_log_in_use`，不会为清理而停 / 启引擎。“清空应用内调试记录”只清内存，不释放文件空间。

诊断 ZIP 在内存生成，固定日志输入总计最多 16 MiB，拒绝符号链接并再次脱敏；不包含 SQLite、session、原生恢复状态、运行态 JSON、设置原文、凭据或用户文件。具体接口见 [日志占用与诊断](api-contract.md#48-日志占用清理与诊断包)。

### 日志在哪里

实机以 `$TRIM_PKGVAR` 为根（常见 `/vol1/@appdata/motrix`）；源码调试对应 `packaging/fnos/app/data/`。

- Rust：`logs/server.log`，单文件上限 10 MiB，当前文件加最多 3 个轮转文件（`.1`～`.3`）。
- 生命周期：`logs/lifecycle.log`，单文件默认上限 1 MiB，最多 3 个轮转文件，包含脚本与进程标准输出。
- Aria2：`aria2/aria2.log`，默认 warn，单文件 10 MiB，当前加最多 2 份历史；原生命名 `aria2.1.log` / `aria2.2.log`，维护也兼容旧 `aria2.log.1` / `.2`。
- 进程身份：`run/motrix-fnos-server.pid`、`run/motrix-fnos-server.starttime` 及 `/proc/<pid>/exe`。不要仅据 PID 或进程名杀进程。

只有确认引擎停止、生命周期稳定且内存 / 磁盘运行态均不存在时，启动门禁才将旧超大 aria2.log 原子保留最后 10 MiB，并在新旧命名中合计保留 2 份历史；无法证明安全时警告并跳过，不为清理唤醒 / 停止引擎。停引擎后可从页面清空已识别日志，不能把原生恢复状态当日志清理。

### 常见问题

| 现象 | 检查与处理 |
| --- | --- |
| 安装失败 | 核对 CPU 架构、最低 fnOS 版本和应用中心任务日志 |
| 启动失败 | 先 lifecycle.log，再 server.log；查真实端口占用和进程归属，不按端口强杀 |
| Web 打不开 | 查 cmd/status、入口 `http://<设备>:<service_port>/?v=<version>`、根 HTML 和 `/api/app/ping`；staged config 不应有 gatewayPrefix / gatewaySocket |
| FN Connect 返回 HTML 403 | 先核对上游 fnOS 登录态及 HTTP/SSE 的 same-origin 凭据；Issue #21 反馈 1.9.3 正常、1.9.4 的 476a3bf 改为 omit 后出现 403，具体 fnOS / 补丁影响仍待对照，不把它误当 JWT JSON 错误 |
| 下载失败 | 核对当前授权、文件归属、引擎兼容状态、代理和诊断；授权目录不可用时不手拼路径绕过 |
| 暂停一直失败 | 按 [已知引擎限制](future-development-plan.md#已知限制与延期)停服重启并核对，不继续暂存或删文件 |
| 中文目录变点号或删除留残余 | 查下方 UTF-8 专项；旧异常目录先保全，不猜名称删除 |
| 升级 / 重装任务不见或还在 | 按 [代际与身份规则](#备份与回滚)判断，先核对实际 TRIM_PKGVAR，不删库重建 |
| 图标模糊 | 查 FPK 内尺寸，再强刷 / 清缓存 / 换浏览器 |

### 忘记密码

1. 在应用中心停止 Motrix。
2. NAS SSH 交互终端执行 `sudo -u motrix_fnos /var/apps/motrix/cmd/reset-web-auth`，隐藏输入并确认新密码。
3. 成功后启动应用，用新密码登录。取消或验证失败保留原密码，成功后旧 JWT 失效，任务 / 设置 / 授权 / RPC Token / 下载文件保留。

密码规则为 8–128 个 Unicode 字符，不自动裁剪，允许空格；不要直接以 root 运行 server 以免改变数据所有权。本机命令通过安装目录 target / var 链接定位，不要求 SSH 预设 TRIM 环境变量。未初始化实例可保留数据卸载后重装修正版并向导设密，或停服后本机重置；网页初始化和 bootstrap-web-auth 已移除。

### 启停与 IPv6

源码调试先 prepare，使用 `packaging/fnos/cmd/start` / `status` / `stop`；实机优先应用中心。

- status：就绪返回 0，进程存在但 ready 未返回 200 为 1，未运行或身份不匹配为 3。查询不建目录、不追加或轮转日志、不删陈旧 PID；清理由明确启停动作负责。
- stop：先优雅退出，再对身份仍匹配的进程有界升级 TERM / KILL，lifecycle.log 记录 INT / TERM / KILL 阶段和最终结果。启动时只清可证明归属的孤儿；短暂 nohup → exec 窗口用启动时间确认本次进程，确认退出后才删 PID。
- IPv6 快速停启：2026-10-23 ARM 日志出现引擎未运行但 `[::]:17080` 绑定失败、等待后恢复；本地 IPv6 连接复现与 TIME_WAIT 相关。现绑定前启用 SO_REUSEADDR，真实 listener 仍占用时拒绝启动；修复后的 fnOS 快速启停仍待实机验收。
- ready 只表示 listener 已绑定、启动门禁完成且未退出，不代表已登录或允许下载；每次查询只读内存，不实时查 SQLite。

### 中文 BT 路径

server 统一为引擎子进程设 `LANG=C.UTF-8`，不改 NAS 全局环境。[libtorrent 2.1.1 路径实现](https://github.com/arvidn/libtorrent/blob/v2.1.1/src/escape_string.cpp#L531)按 LANG 的 `.UTF-8` 后缀决定 UTF-8；既有 ARM 排查确认缺失时中文目录变点号，内容可能落在专属目录外。

本地回归覆盖缺失 LANG、C、en_US.utf8 的覆盖行为，但不能代替 Linux/fnOS 验收。使用全新目录验证；已有点号目录不自动改名、接管或删除，先确认停止写入，再证明归属，不同中文名称可能变成相同点号名。

### 远程接入

公网反代只提供 `/jsonrpc`，用于外部解析站 / 工具提交任务，完整任务管理仍使用 Motrix UI。方法与 Token 规则见 [RPC 契约](api-contract.md#6-json-rpc-兼容入口)，端口 / 来源见 [运行约定](api-contract.md#1-运行时约定)。

```text
外部客户端 -> wss://motrix.example.com:8443/jsonrpc
  -> Cloudflare 橙云 -> 家庭可入站 IPv6 -> Lucky TLS 8443
  -> http://127.0.0.1:17081/jsonrpc -> Motrix -> Aria2 内部 RPC
```

FN Connect 子域名依赖 fnOS 登录态，第三方解析站不带该 Cookie，不能替代公网 RPC 代理。不要直接公开管理 17080、LAN 17082 或内部 6800。17081 只在 NAS 本机可访问，不添加端口映射。

配置步骤：

1. 在设置页配置公网 JSON-RPC Token，先在 NAS 本机验证：

   ```bash
   curl -i http://127.0.0.1:17081/jsonrpc -H 'Content-Type: application/json' \
     -d '{"jsonrpc":"2.0","id":"test","method":"aria2.getVersion","params":[]}'
   ```

2. Lucky 与 Motrix 须处于同一网络命名空间或具备 NAS 内部可达链路；后端指向 `http://127.0.0.1:17081`，不能指向管理端口。无法访问回环时先解决部署网络，不公开回环入口绕过限制。
3. DNS 通常只维护同名的一条可入站全局 IPv6 AAAA，开启橙云；IPv4 无入站能力时删除无效 A，不用 fe80::。橙云后的 dig 返回 Cloudflare 地址正常，源站以控制台 / Lucky 为准。Cloudflare Token 只给目标 Zone 读取与 DNS 编辑，多级域名在 Lucky 配好自定义后缀。
4. Lucky HTTPS 前端监听 8443（或 443），TLS 1.2+，绑定覆盖域名的证书；Cloudflare 使用 Full (strict)。Origin CA 仅用于 Cloudflare 回源，灰云浏览器直连需公共受信证书。DNS、端口、协议、TLS 与 Host 逐项核对。
5. 解析站填写 `wss://motrix.example.com:8443/jsonrpc`、公网 Token 和已授权保存路径；443 可省略。HTTPS 页面使用 wss，不用 ws。不要填 NAS 的四个内部端口作为公网地址。

外网验证（替换示例域名与端口）：

```bash
curl -i https://motrix.example.com:8443/
curl -i https://motrix.example.com:8443/api/settings
curl -i https://motrix.example.com:8443/jsonrpc -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":"test","method":"aria2.getVersion","params":[]}'
curl -i -X OPTIONS https://motrix.example.com:8443/jsonrpc \
  -H 'Origin: https://sender.example.com' \
  -H 'Access-Control-Request-Method: POST' \
  -H 'Access-Control-Request-Headers: content-type'
npx wscat -c wss://motrix.example.com:8443/jsonrpc
```

根路径和 `/api/settings` 应为 404 或上游 403，不能打开管理面；RPC 返回版本（引擎停止且无缓存时允许 unknown），OPTIONS 含必要跨域响应头，管理 API 不因此开放跨域。WebSocket 连接后发送同样的 getVersion JSON。再按 [addUri 示例](api-contract.md#6-json-rpc-兼容入口)使用已授权目录或子目录和测试 URL 添加任务，成功返回 GID，结束后清理测试任务与文件。

getVersion 匿名可用不代表写鉴权关闭；写方法及 multicall 子调用各自带正确 Token。磁链仍需在 UI 确认文件，BT 使用总代理而非任务字段；`-32005` 时按 `/api/app/compatibility` 诊断。两类 RPC Token、Web 密码和内部 secret 不可混用，不把凭据、私钥、Cloudflare Token 写进仓库、日志或截图。

局域网可选：真实 RFC1918 IPv4 客户端访问 `http://NAS地址:17082/jsonrpc`，使用独立 LAN Token；100.64.0.0/10 需显式允许，默认关闭。不伪造 X-Forwarded-For；验证关闭、来源拒绝和 Token 错误。局域网端口冲突降级后需释放端口并重启应用。

| 现象 | 处理 |
| --- | --- |
| 本机 17081 失败 | 查 ready，从 NAS 本机访问 127.0.0.1，不用局域网 IP 代替 |
| 公网根页面或 API 能打开 | 错误代理到了管理端口，立即改回回环 RPC |
| zone not found | 查 Token Zone 权限及 Lucky 域名后缀 |
| 526 | 查证书域名、有效期、私钥匹配和绑定；临时 Full 只用于定位，修复后恢复 strict |
| 522 / 超时 | 查可入站 IPv6、NAS/Lucky 防火墙、AAAA 与不可入站 A |
| CORS / WebSocket 失败 | 按 OPTIONS → POST → wscat 定位，核对 https / wss |
| addUri Token 错误 | 核对公网 Token，不能用 LAN Token 或 Aria2 secret |

参考：[Cloudflare DDNS](https://developers.cloudflare.com/dns/manage-dns-records/how-to/managing-dynamic-ip-addresses/)、[Origin CA](https://developers.cloudflare.com/ssl/origin-configuration/origin-ca/)、[代理端口](https://developers.cloudflare.com/fundamentals/reference/network-ports/)、[Lucky DDNS](https://lucky666.cn/docs/modules/ddns)。
