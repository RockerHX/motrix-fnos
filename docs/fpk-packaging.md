# FPK 打包说明

## 作用

这份文档统一说明 **开发验证、FPK 构建、产物定位和发布流程**，以及最小调试 / 排障入口。

它不记录项目阶段状态，也不承担产品能力说明；其中命令、路径、产物命名和 manifest 约定必须与仓库脚本保持一致。

## 已查证约束

截至 2026-07-14，当前 FPK 打包约束以飞牛官方文档和本仓库本地验证为准；本轮双监听器交付前已重新读取下列 Manifest、应用框架、fnpack 与应用入口页面：

- 官方 Manifest 文档明确了 `platform=x86|arm|all`、`os_min_version`、`service_port` 等字段，但**没有文档化 `arch` 字段**。当前仓库仍保留 x86 staging 中的 `arch = x86_64`，直到官方资料或实机验证证明可删。
- 官方应用框架文档列出了 `cmd/main`、`install_*`、`upgrade_*`、`uninstall_*`、`config_*` 生命周期脚本。
- 官方应用入口文档允许 `iframe` 入口使用 `protocol`、`port` 与 `url` 打开应用服务端口。Motrix 使用已由 ARM 实机验证的端口入口：`protocol=http`、`port=service_port`、`url=/?v=<version>`。
- 使用**当前已验证版本** `fnpack 1.2.1` 创建最小工程并在本地验证后确认：
  - 缺少 `cmd/main`、`install_*`、`upgrade_*`、`uninstall_*` 时，`fnpack build` 会报告 `Required file ... is missing`。
  - 缺少 `config_init` 或 `config_callback` 时，`fnpack build` 仍可成功。
  - `fnpack build` 在打印 `Packing failed` 时**仍可能返回退出码 0**，因此仓库构建脚本必须额外校验产物和日志，不能只信退出码。
- 2026-07-14 Release 产物与 ARM 实机回归确认：`1.6.4` 使用端口入口且可打开；commit `8f74bf7` 从 `1.6.5` 起切换到统一网关，并把 TCP 端口限制为仅 JSON-RPC，但打包脚本直到 `1.7.1` 仍向最终入口注入 `port=17080`。该混合模型使桌面入口与 Web UI listener 不一致，是首次引入 404 的回归。
- 2026-07-14 ARM 实机进一步确认：移除入口 `port` 的本地测试包中，Unix Socket 根 HTML 与 API 直连均为 `200`，但 fnOS nginx 对桌面入口仍返回 `404 9`，请求没有进入应用 Socket。因此当前交付恢复 `1.6.4` 已验证的端口入口，不再注册统一网关。
- Web UI 构建保持相对基址 `./`，确保从端口入口根路径加载静态资源。
- 桌面入口默认 `allUsers=false`，但 `control.accessPerm=editable`，允许管理员在应用设置中切换“仅管理员 / 设备内所有用户”。端口模式不提供可信 `X-Trim-*` Header，后端不得依赖统一网关身份。
- `config_callback` 当前承担授权目录快照同步职责，不纳入删除候选；`config_init` 只有在完成配置流程验证后才可评估是否移除。
- 官方入口和 manifest 只描述对外服务端口，不提供“仅供应用内本机反代”的第二端口声明。Motrix 因此只把管理端口 `17080` 注册给 fnOS；JSON-RPC 专用端口 `17081` 由 server 在回环地址监听，不注册为平台资源。

如果后续升级 `fnpack`，需要重新验证至少以下行为是否仍成立：

- 缺少哪些生命周期脚本会导致打包失败；
- `fnpack build` 失败时退出码是否可靠；
- `--directory` 模式下 `.fpk` 产物写入位置；
- x86 staging 中 `arch = x86_64` 是否仍有必要保留。

相关官方资料：

- Manifest：https://developer.fnnas.com/docs/core-concepts/manifest/
- 应用框架 / 生命周期：https://developer.fnnas.com/docs/core-concepts/framework/
- fnpack：https://developer.fnnas.com/docs/cli/fnpack/
- 应用入口：https://developer.fnnas.com/docs/core-concepts/app-entry/
- 统一网关：https://developer.fnnas.com/docs/core-concepts/gateway-registration/

## 当前产物

默认命令会同时生成 x86 与 ARM 两个 FPK，`<version>` 来自核心版本源；Release workflow 会校验 `package.json`、`server/Cargo.toml`、`packaging/fnos/manifest.template` 与 `packaging/fnos/app/ui/config` 的入口缓存版本保持一致：

- x86：`packaging/fnos/dist/motrix.fnos_<version>_x86.fpk`
- ARM：`packaging/fnos/dist/motrix.fnos_<version>_arm.fpk`

对应 server 二进制：

- x86：`server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server`
- ARM：`server/target/aarch64-unknown-linux-gnu/release/motrix-fnos-server`

## 双监听器与端口边界

FPK 启动脚本必须向同一个 Rust server 注入两个地址：

| 环境变量 | FPK 默认值 | 平台可见性 |
| --- | --- | --- |
| `MOTRIX_FNOS_HTTP_ADDR` | `0.0.0.0:17080` | manifest、桌面入口和 `MotrixFNOS.sc` 只映射该管理端口 |
| `MOTRIX_FNOS_JSONRPC_ADDR` | `127.0.0.1:17081` | 仅 NAS 本机反向代理可访问，不进入任何 fnOS 端口声明 |

固定规则：

- `manifest.service_port`、`app/ui/config` 的 iframe 入口端口以及 `MotrixFNOS.sc` 的源/目标端口必须都是 `17080`。
- `config/resource` 只引用管理端口协议文件，不得额外注册 `17081`。
- `17081` 不监听 NAS 局域网或公网地址；Lucky 只能在 NAS 本机反向代理到 `http://127.0.0.1:17081`。
- 显式覆盖 `MOTRIX_FNOS_JSONRPC_ADDR` 时，Rust server 仍会拒绝任何非回环地址。
- FPK 日志可以记录两个监听地址，但不得记录 Web 密码、Session、CSRF、JSON-RPC Token 或 Aria2 secret。

设备架构必须匹配：

- `x86_64` 设备安装 x86 包
- `aarch64` / `arm64` 设备安装 ARM 包

## 常用命令

安装依赖：

```bash
rtk pnpm install
```

只做预组装，不执行 `fnpack build`：

```bash
rtk pnpm run build:fpk:prepare
```

同时构建 x86 与 ARM：

```bash
rtk pnpm run build:fpk
```

只构建 x86：

```bash
rtk pnpm run build:fpk:x64
```

只构建 ARM：

```bash
rtk pnpm run build:fpk:arm64
```

升级或重新校验仓库内置的 Aria2 Next 源资产：

```bash
rtk pnpm run assets:aria2:fetch
```

该命令只用于维护 `assets/aria2/` 中固定版本的双架构 sidecar，会从对应 GitHub Release 下载 checksum，校验现有或新下载的二进制并设置可执行权限。它不是日常构建步骤；升级版本时还需审查脚本中的版本号、checksum 文件和两个二进制的变更。

清理本地构建输出和 staged 产物：

```bash
rtk pnpm run clean
```

如需先查看会删除哪些内容：

```bash
rtk pnpm run clean:dry-run
```

清理 Rust 增量编译缓存，保留主要依赖构建缓存：

```bash
rtk pnpm run clean:rust:incremental
```

彻底清理 Rust 构建缓存：

```bash
rtk pnpm run clean:rust
```

`pnpm run verify` / `pnpm run verify:pre-commit` 默认会在验证结束后自动执行 Rust incremental 缓存清理；如需临时保留增量缓存，可追加 `--keep-rust-incremental`。

## 开发与验证

Web UI 类型检查：

```bash
rtk pnpm run typecheck
```

Web UI 生产构建：

```bash
rtk pnpm run build
```

Rust server 测试：

```bash
rtk cargo test --manifest-path server/Cargo.toml
```

提交前快速验证：

```bash
rtk pnpm run verify:pre-commit
```

发布前完整验证：

```bash
rtk pnpm run verify
```

双架构预组装并执行端口预检：

```bash
rtk pnpm run build:fpk:prepare
```

预组装后应分别检查 `.stage/x86` 和 `.stage/arm`：

```bash
rtk rg -n '17080|17081' packaging/fnos/.stage/x86/manifest packaging/fnos/.stage/x86/MotrixFNOS.sc packaging/fnos/.stage/x86/app/ui/config packaging/fnos/.stage/x86/config/resource
rtk rg -n 'MOTRIX_FNOS_(HTTP|JSONRPC)_ADDR' packaging/fnos/.stage/x86/cmd/common.sh
rtk file packaging/fnos/.stage/x86/app/bin/motrix-fnos-server packaging/fnos/.stage/arm/app/bin/motrix-fnos-server
```

预期 `17081` 不出现在任何平台端口声明中，只出现在生命周期脚本的回环监听默认值中；两个 staged `app/data/` 目录均为空。

完整构建后可在临时目录解包，不要直接修改产物：

```bash
mkdir -p /tmp/motrix-fpk-check/x86 /tmp/motrix-fpk-check/arm
tar -xzf packaging/fnos/dist/motrix.fnos_<version>_x86.fpk -C /tmp/motrix-fpk-check/x86
tar -xzf packaging/fnos/dist/motrix.fnos_<version>_arm.fpk -C /tmp/motrix-fpk-check/arm
```

## 打包目录

当前 FPK 主目录：

```text
packaging/fnos/
```

关键内容：

- `manifest.template`：源码态 manifest 模板输入
- `.stage/<target>/manifest`：预组装后的真实 manifest
- `cmd/`：启动、停止、状态脚本
- `config/`：资源与权限声明
- `app/bin/`：server 与 `aria2-next`
- `app/ui/dist/`：Web UI 静态资源
- `app/data/`：运行时数据目录
- `.stage/`：预组装后的打包目录
- `dist/`：最终输出的 `.fpk`

约定：

- 源码态 `packaging/fnos/` **不是**可直接执行 `fnpack build` 的安全输入目录；真实打包输入由 `build:fpk:prepare` / `build:fpk*` 生成到 `.stage/<target>/`。
- 如需手动检查 manifest、入口配置、端口配置或生命周期脚本，请检查 `.stage/x86/` 或 `.stage/arm/`，不要直接在源码态目录执行 `fnpack build`。
- `dist/`、`.stage/`、`app/bin/` 中构建脚本放置的 server / Aria2 二进制、`app/ui/dist/`、`dist/*.fpk` 和 stage 目录内产物都是本地生成产物，不应作为源码态内容长期保留。
- `assets/aria2/aria2-next-*` 是当前 `scripts/stage-aria2-sidecar.mjs` 使用的 sidecar 源资产，不是无用产物；只有未来改成下载缓存或发布资产拉取模式后，才可重新评估是否从仓库移除。

## 本地调试

先执行：

```bash
rtk pnpm run build:fpk:prepare
```

需要检查预组装结果时，查看：

```text
packaging/fnos/.stage/x86/
packaging/fnos/.stage/arm/
```

本地调试源码态脚本时，再执行：

```bash
rtk packaging/fnos/cmd/start
rtk packaging/fnos/cmd/status
rtk packaging/fnos/cmd/stop
```

常看两个位置：

- 日志：`packaging/fnos/app/data/logs/server.log`
- PID：`packaging/fnos/app/data/run/motrix-fnos-server.pid`
- 进程启动时间：`packaging/fnos/app/data/run/motrix-fnos-server.starttime`，与 `/proc/<pid>/exe` 一起用于防止 PID 复用误判。

## 最小排障

- 安装失败：先检查包架构是否与设备一致
- 启动失败：先看 `server.log`
- Web UI 打不开：先看 `cmd/status` 和浏览器请求地址。桌面入口应打开 `http://<设备>:<service_port>/?v=<version>`；同时确认 staged `app/ui/config` 不含 `gatewayPrefix` 或 `gatewaySocket`，Rust server 的同一端口能返回根 HTML 与 `/api/app/ping`。
- 下载失败：先看保存目录权限、Aria2 sidecar 和诊断日志
- 升级后任务或设置丢失：确认 `cmd/uninstall_callback` 默认保留 `TRIM_PKGVAR`，且未收到卸载向导删除数据变量
- 卸载后重装仍有旧任务：这是默认保留数据的预期行为；如需完全清理，卸载时开启“同时删除 Motrix 应用数据”

## 数据保留与卸载向导

fnOS 会在卸载时保留应用 `var` 类用户数据目录；本项目也以保留用户数据为默认策略：

- 升级必须保留 `TRIM_PKGVAR` 中的 SQLite、设置、JSON-RPC 密钥、Aria2 session 和日志。
- 卸载默认保留 `TRIM_PKGVAR`，便于后续重装继续使用原任务和设置。
- 只有卸载向导 `MOTRIX_FNOS_DELETE_APP_DATA` 被用户明确开启时，`cmd/uninstall_callback` 才会清理 `TRIM_PKGVAR`。
- 清理范围仅限 Motrix 应用私有数据；用户下载目录和已下载文件不在清理范围内。
- 卸载向导的 `switch` 不要设置`initValue`初始值，字符串使用什么字符串的初始值都不对，感觉是开发者文档有问题，如果设置布尔值会导致打包失败。

### 升级前备份与回滚

- 实机升级前先停止应用，再备份 fnOS 实际提供的 `TRIM_PKGVAR` 目录；运行中的 SQLite 与 Aria2 session 不作为可靠备份源。
- 记录升级前的任务数量、下载设置、授权目录、Aria2 session 校验值和 JSON-RPC Token“是否已配置”，不要把 Token 原文写入验证记录。
- 新包升级后应先在 NAS 本机确认 `127.0.0.1:17081/jsonrpc`，再修改 Lucky；避免把公网流量提前指向尚未监听的端口。
- 回滚需要恢复旧 FPK 和升级前应用数据备份，并将 Lucky 后端恢复为旧版对应地址；不得只把 Lucky 改回 `17080` 而继续运行不提供该 JSON-RPC 路由的新 server。

### 验证结论分级

- “本地构建通过”只表示自动化测试、双架构交叉编译、FPK 预检、解包和静态内容检查通过。
- “ARM/x86 实机通过”必须分别完成对应架构的安装或升级、启动/停止、监听地址、管理登录、数据保留和恢复命令验证。
- “公网链路通过”还必须在真实 IPv4-only 与原生 IPv6 网络完成 Cloudflare、Lucky、HTTP、CORS 和 WebSocket 矩阵。
- 未取得实机或外网证据时，只能记录为“待验证”，不得由本地构建结果推断通过。

## 生命周期实机验证矩阵

下列项目用于 P6 及后续发布前的 fnOS 实机验证；未完成的项不得在文档中宣称“已验证通过”。

| 场景 | 操作 | 预期结果 | 重点观察 |
| --- | --- | --- | --- |
| 安装 | 安装匹配架构的 `.fpk` | 应用中心安装成功 | 安装界面报错、`TRIM_TEMP_LOGFILE`、应用中心任务日志 |
| 启动 | 在应用中心或 `appcenter-cli start` 启动 | 服务进入运行中，Web UI 可打开 | `cmd/status`、`server.log`、监听端口 |
| 停止 | 在应用中心或 `appcenter-cli stop` 停止 | 服务退出，状态变为未运行 | `cmd/status`、PID 文件是否清理 |
| 配置变更 | 在“应用设置”修改授权目录并保存 | `config_callback` 重新同步 accessible paths | `app/data/accessible-paths.json`、`server.log`、配置保存日志 |
| 升级 | 安装旧版本后升级到新包 | 数据与配置保留，服务可重新启动 | 升级界面日志、任务数据、`server.log` |
| 卸载（默认） | 卸载应用且不勾选删除数据 | `TRIM_PKGVAR` 应用数据保留，不删除用户下载文件 | 卸载向导选项、`cmd/uninstall_callback` 日志、`TRIM_PKGVAR` 内容 |
| 卸载（删除数据） | 卸载应用并勾选“同时删除 Motrix 应用数据” | 仅清理 `TRIM_PKGVAR` 内的 Motrix 应用数据，不删除用户下载文件 | `cmd/uninstall_callback` 日志、数据库/设置/session/log 是否被清理 |

建议实机验证命令：

```bash
appcenter-cli install-fpk <package>.fpk
appcenter-cli start motrix.fnos
appcenter-cli stop motrix.fnos
appcenter-cli list
```

如涉及向导或交互式配置，优先使用应用中心界面完成；`appcenter-cli` 更适合重复安装和脚本化验证。

## 相关文档

- 长期架构：`docs/architecture.md`
- 阶段状态：`docs/development-plan.md`
- 接口契约：`docs/api-contract.md`

## GitHub Actions 自动发版流程

当前默认发版入口是 `Release FPK` workflow。正常路径只需要人工输入一次版本号：

```text
Actions -> Release FPK -> Run workflow -> 输入 x.y.z
```

后续流程自动完成：

```text
Release FPK
  -> 读取 latest tag..HEAD commit log
  -> 优先通过 GitHub Models 生成中文 CHANGELOG
  -> GitHub Models 不可用时回退到 commit log 简单归类
  -> 同步 package / Cargo / FPK manifest / UI cache 版本
  -> 更新 Cargo.lock
  -> 跑完整 `pnpm run verify`
  -> x86 / ARM FPK 构建
  -> 校验产物并生成 SHA256SUMS.txt
  -> 提交 `chore: 发布 x.y.z 版本` 到 main
  -> 创建 `v<x.y.z>` tag
  -> 创建或更新 GitHub Release
```

发版白名单文件：

```text
CHANGELOG.md
package.json
server/Cargo.toml
server/Cargo.lock
packaging/fnos/manifest.template
packaging/fnos/app/ui/config
```

`Release FPK` workflow 会上传：

- `motrix.fnos_<version>_x86.fpk`
- `motrix.fnos_<version>_arm.fpk`
- `SHA256SUMS.txt`

`Release FPK` 在同一个 workflow 内完成验证、构建、提交、打 tag 和上传 Release，不依赖 PR 自动批准、自动合并，也不依赖 `GITHUB_TOKEN` 推送 tag 后再触发另一个 workflow。

### 验证触发策略

- `push main` 默认触发 `Verify`。
- 仅包含发版白名单文件的 `push main` 会跳过 `Verify`，避免 `Release FPK` 提交发版 commit 后重复验证。
- 任意 PR 会触发 `Verify`，用于普通代码审查。
- `Release FPK` 自身会运行完整 `pnpm run verify`，这是发版流程的完整代码验证。

### GitHub Actions 缓存策略

- `Verify` 只缓存 pnpm store 和 Cargo registry，不缓存 `server/target` 编译产物。
- Rust `server/target` 缓存体积容易达到数百 MB 到 1GB，且版本号 / `Cargo.lock` 变化会产生新 key；当前项目优先控制缓存占用，而不是追求最大 CI 加速。
- `Cleanup Actions Caches` 手动执行默认删除全部 Actions caches；如只想删除非 `main` 分支缓存，可在运行 workflow 时将 `scope` 选为 `non-main`。
- 清空缓存不会影响源码或 Release 产物，只会让下一次 CI 重新下载 / 编译依赖。

### 本地发版备用流程

如 GitHub 自动 PR 流程异常，可在本地使用备用命令：

```bash
rtk pnpm run release:prepare <x.y.z>
rtk git push
rtk git push origin v<x.y.z>
```

本地命令会复用 `CHANGELOG.md` 中已填写的目标版本条目；如果未填写，会按 commit log 生成确定性草稿。
