# FPK 打包说明

## 作用

这份文档统一说明 **开发验证、FPK 构建、产物定位和发布流程**，以及最小调试 / 排障入口。

它不记录项目阶段状态，也不承担产品能力说明；其中命令、路径、产物命名和 manifest 约定必须与仓库脚本保持一致。

## 应用身份与 FN Connect 域名

Motrix 当前使用下面这组相互配套的身份字段：

```text
appname               = motrix
desktop_appname       = motrix.Application
desktop_applaunchname =
```

`app/ui/config` 中必须只有一个同名入口：

```json
".url": {
  "motrix.Application": { "type": "iframe", "port": "17080" }
}
```

这四处必须一起保持一致：

1. `appname` 决定 FPK 身份和产物前缀 `motrix_`。
2. `desktop_appname` 与 `.url` 键决定 FN Connect 注册的应用入口身份。
3. 空的 `desktop_applaunchname` 让 FN Connect 使用标准 `Application` 入口，不生成 `-main` 后缀。
4. `service_port` 和入口 `port` 继续使用管理端口 `17080`；`MotrixFNOS.sc` 声明 `17080/tcp,17082/tcp`，不声明 `17081`。

这组配置已在 fnOS 实机验证，访问地址为 `https://motrix.<account>.fnos.net/`。旧的 `motrix.fnos` 身份和 `motrix.fnos.main` 入口会生成带后缀的域名，不能只改其中一个字段。构建脚本和静态测试会阻止身份字段再次分离。

正式包同时声明 `micro_app=true`，并在 `config/resource` 中精确申请 `trim.file.sharedAccess` 与 `trim.file.path`。前者用于确认应用共享授权，后者用于把已确认路径转换为面向用户的语义化路径。`os_min_version` 固定为 `1.2.0401`：安装器会拒绝更低版本的 fnOS，正式包只使用官方 Unix Socket 和 SDK 授权链路，不再维护旧系统的人工授权快照流程。独立浏览器或 SDK 不可用时，页面提示用户改用 fnOS 宿主。

构建和解包校验必须拒绝额外 Scope，并扫描 Web UI 产物，确保不包含 `TRIM_API_TOKEN`、官方 Socket 路径或 Authorization Header 拼装代码。

## 已查证约束

截至 2026-07-17，当前 FPK 打包约束以飞牛官方文档和本仓库本地验证为准；三监听器交付前已重新读取下列 Manifest、应用框架、fnpack、应用入口与图标页面：

- 官方 Manifest 文档明确了 `platform=x86|arm|all`、`os_min_version`、`service_port` 等字段，但**没有文档化 `arch` 字段**。当前仓库仍保留 x86 staging 中的 `arch = x86_64`，直到官方资料或实机验证证明可删。
- 官方应用框架文档列出了 `cmd/main`、`install_*`、`upgrade_*`、`uninstall_*`、`config_*` 生命周期脚本。
- 官方应用入口文档允许 `iframe` 入口使用 `protocol`、`port` 与 `url` 打开应用服务端口。Motrix 使用已由 ARM 实机验证的端口入口：`protocol=http`、`port=service_port`、`url=/?v=<version>`。
- 使用**当前已验证版本** `fnpack 1.2.1` 创建最小工程并在本地验证后确认：
  - 缺少 `cmd/main`、`install_*`、`upgrade_*`、`uninstall_*` 时，`fnpack build` 会报告 `Required file ... is missing`。
  - 缺少 `config_init` 或 `config_callback` 时，`fnpack build` 仍可成功。
  - `fnpack build` 在打印 `Packing failed` 时**仍可能返回退出码 0**，因此仓库构建脚本必须额外校验产物和日志，不能只信退出码。
  - 对用户提供的 Lucky 2.27.2 FPK 进行只读解包后确认：它使用 `appname=Lucky`、`desktop_appname=Lucky.Application`、空 `desktop_applaunchname` 和唯一 `Lucky.Application` 入口。Motrix 按同一规则配置后，已在 fnOS 实机验证 `motrix.<account>.fnos.net` 可用。
- 2026-07-14 Release 产物与 ARM 实机回归确认：`1.6.4` 使用端口入口且可打开；commit `8f74bf7` 从 `1.6.5` 起切换到统一网关，并把 TCP 端口限制为仅 JSON-RPC，但打包脚本直到 `1.7.1` 仍向最终入口注入 `port=17080`。该混合模型使桌面入口与 Web UI listener 不一致，是首次引入 404 的回归。
- 2026-07-14 ARM 实机进一步确认：移除入口 `port` 的本地测试包中，Unix Socket 根 HTML 与 API 直连均为 `200`，但 fnOS nginx 对桌面入口仍返回 `404 9`，请求没有进入应用 Socket。因此当前交付恢复 `1.6.4` 已验证的端口入口，不再注册统一网关。
- Web UI 构建保持相对基址 `./`，确保从端口入口根路径加载静态资源。
- 桌面入口默认 `allUsers=false`，但 `control.accessPerm=editable`，允许管理员在应用设置中切换“仅管理员 / 设备内所有用户”。端口模式不提供可信 `X-Trim-*` Header，后端不得依赖统一网关身份。
- `config_callback` 保留为空操作脚本以满足既有生命周期布局，不再同步授权目录；`config_init` 只有在完成配置流程验证后才可评估是否移除。
- 官方入口和 manifest 只描述单个桌面服务端口，不提供“仅供应用内本机反代”的第二入口声明。Motrix 因此只把管理端口 `17080` 写入 manifest 和桌面入口；回环反代端口 `17081` 不注册为平台资源，局域网端口 `17082` 仅通过已验证的 `.sc` 多端口格式声明。

如果后续升级 `fnpack`，需要重新验证至少以下行为是否仍成立：

- 缺少哪些生命周期脚本会导致打包失败；
- `fnpack build` 失败时退出码是否可靠；
- `--directory` 模式下 `.fpk` 产物写入位置；
- x86 staging 中 `arch = x86_64` 是否仍有必要保留。

相关官方资料：

- Manifest：https://developer.fnnas.com/docs/core-concepts/manifest/
- 图标：https://developer.fnnas.com/docs/core-concepts/icon/
- 应用框架 / 生命周期：https://developer.fnnas.com/docs/core-concepts/framework/
- fnpack：https://developer.fnnas.com/docs/cli/fnpack/
- 应用入口：https://developer.fnnas.com/docs/core-concepts/app-entry/
- 统一网关：https://developer.fnnas.com/docs/core-concepts/gateway-registration/

## 当前产物

默认命令会同时生成 x86 与 ARM 两个 FPK，`<version>` 来自核心版本源；Release workflow 会校验 `package.json`、`server/Cargo.toml`、`packaging/fnos/manifest.template` 与 `packaging/fnos/app/ui/config` 的入口缓存版本保持一致：

- x86：`packaging/fnos/dist/motrix_<version>_x86.fpk`
- ARM：`packaging/fnos/dist/motrix_<version>_arm.fpk`

对应 server 二进制：

- x86：`server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server`
- ARM：`server/target/aarch64-unknown-linux-gnu/release/motrix-fnos-server`

## 三监听器与端口边界

FPK 启动脚本必须向同一个 Rust server 注入三个地址：

| 环境变量 | FPK 默认值 | 平台可见性 |
| --- | --- | --- |
| `MOTRIX_FNOS_HTTP_ADDR` | `0.0.0.0:17080` | manifest、桌面入口和 `MotrixFNOS.sc` 只映射该管理端口 |
| `MOTRIX_FNOS_JSONRPC_ADDR` | `127.0.0.1:17081` | 仅 NAS 本机反向代理可访问，不进入任何 fnOS 端口声明 |
| `MOTRIX_FNOS_LAN_JSONRPC_ADDR` | `0.0.0.0:17082` | 由 `MotrixFNOS.sc` 与管理端口共同声明，manifest 与桌面入口仍不引用 |
| `MOTRIX_TRUSTED_PROXY_IPS` | 空 | 仅填写直接连接到管理 listener 的可信代理 IP；未配置时忽略 `X-Forwarded-For` |
| `MOTRIX_WEB_COOKIE_SECURE` | `false` | HTTPS 终止代理场景显式设为 `true`；直接 HTTP 场景保持 `false` |

固定规则：

- `manifest.service_port` 与唯一 `app/ui/config` iframe 入口端口必须都是 `17080`；`MotrixFNOS.sc` 的源/目标端口必须精确声明 `17080/tcp,17082/tcp`。`desktop_applaunchname` 留空时，构建脚本必须确认 `.url` 中恰好只有一个入口并自动选取它。
- `config/resource` 只引用管理端口协议文件，不得额外注册 `17081`。
- `17081` 不监听 NAS 局域网或公网地址；Lucky 只能在 NAS 本机反向代理到 `http://127.0.0.1:17081`。
- `17082` 始终监听但由服务端开关和 RFC1918 IPv4 来源检查共同保护；局域网 Token 不得在 `17081` 使用。
- 显式覆盖 `MOTRIX_FNOS_JSONRPC_ADDR` 时，Rust server 仍会拒绝任何非回环地址。
- FPK 日志可以记录三个监听地址，但不得记录 Web 密码、Session、CSRF、JSON-RPC Token 或 Aria2 secret。
- 管理 listener 直连时，客户端提交的 `X-Forwarded-For` 不参与登录限速；只有实际对端地址命中 `MOTRIX_TRUSTED_PROXY_IPS` 才能使用该 Header 的第一个合法 IP。
- 反向代理终止 HTTPS 时必须同时确认代理地址已加入 `MOTRIX_TRUSTED_PROXY_IPS`，并显式设置 `MOTRIX_WEB_COOKIE_SECURE=true`。该开关不会验证代理是否真的使用 HTTPS。

多端口声明查证：飞牛官方 manifest 文档目前只定义单个 `service_port`；可验证第三方 FPK 仓库 `conversun/fnos-apps` 的 AdGuardHome 与 Forgejo 分别使用同一 `.sc` 文件声明 `3080/tcp,53/tcp,53/udp` 和 `3005/tcp,2223/tcp`。本项目据此只扩展协议文件，不增加第二桌面入口；正式发布前仍须在目标 fnOS 实机确认安装、防火墙、局域网 TCP 对端地址和卸载清理行为。

设备架构必须匹配：

- `x86_64` 设备安装 x86 包
- `aarch64` / `arm64` 设备安装 ARM 包

## 图标尺寸与高清显示

飞牛官方图标文档：<https://developer.fnnas.com/docs/core-concepts/icon/>。

官方规范仍然区分两组固定文件名和尺寸：

| 用途 | 固定路径 | 官方尺寸 |
| --- | --- | ---: |
| FPK 包图标 | `ICON.PNG` | 64×64 |
| FPK 包图标 | `ICON_256.PNG` | 256×256 |
| 应用入口图标 | `app/ui/images/icon_64.png` | 64×64 |
| 应用入口图标 | `app/ui/images/icon_256.png` | 256×256 |

`app/ui/config` 使用 `images/icon_{0}.png` 时，fnOS 会按入口需要替换 `{0}`。图标还应满足正方形、sRGB、PNG/JPG、单文件不超过 1024 KB 等官方要求。

### 本项目的高清交付约定

官方文件名和入口配置保持不变，但 2026-07-17 对 `1.7.5` FPK 的 ARM 与 x86 产物进行实机验证后，确定项目内所有交付 PNG 统一使用 **256×256**，避免桌面入口和应用中心在高 DPI 或大尺寸显示时放大低分辨率图像：

- `packaging/fnos/ICON.PNG`：256×256；
- `packaging/fnos/ICON_256.PNG`：256×256；
- `app/ui/images/icon_64.png`：256×256，保留官方入口选择所需的文件名；
- `app/ui/images/icon_256.png`：256×256；
- `public/icon.png` 构建为 `app/ui/dist/icon.png`：256×256。

因此，`icon_64.png` 的 `64` 是兼容 fnOS 入口文件名与 `{0}` 选择规则的名称，不代表本项目交付的像素尺寸。`scripts/build/build-fpk.mjs` 会同步这些资源，并在 FPK 预检阶段统一校验为 256×256；不要只修改其中一个尺寸或手工在 `app/ui/images/` 下留下旧生成物。

图标显示模糊时，先确认实际 FPK 包内资源尺寸，再清理浏览器缓存、执行强制刷新或换浏览器复测。`1.7.5` 的实测表明，同一个 FPK 在缓存未更新的浏览器中可能显示模糊，而换浏览器后立即显示高清；这种现象不代表 FPK 中缺少 256 图标。官方 64/256 规范没有改变，全部使用 256 是本项目基于实机显示效果确定的交付约定。

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

该本地入口会先运行一次完整源码验证，再复用已验证的 Web UI 构建双架构 FPK，并在完成后解包验收两个产物。Release workflow 使用内部的 `build:fpk:artifacts`，只构建发布产物。

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

彻底清理 Rust 构建缓存：

```bash
rtk pnpm run clean:rust
```

`pnpm run verify` 会保留 Rust 编译缓存；`pnpm run verify:pre-commit` 只做版本、暂存区空白与 Rust 格式等快速静态检查，不执行前端类型检查，也不写 Rust 构建产物。磁盘空间不足时再执行 `pnpm run clean:rust`。

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

本地完整验证、双架构打包和产物验收：

```bash
rtk pnpm run build:fpk
```

双架构预组装并执行端口预检：

```bash
rtk pnpm run build:fpk:prepare
```

预组装后应分别检查 `.stage/x86` 和 `.stage/arm`：

```bash
rtk rg -n '17080|17081|17082' packaging/fnos/.stage/x86/manifest packaging/fnos/.stage/x86/MotrixFNOS.sc packaging/fnos/.stage/x86/app/ui/config packaging/fnos/.stage/x86/config/resource
rtk rg -n 'MOTRIX_FNOS_(HTTP|JSONRPC|LAN_JSONRPC)_ADDR' packaging/fnos/.stage/x86/cmd/common.sh
rtk file packaging/fnos/.stage/x86/app/bin/motrix-fnos-server packaging/fnos/.stage/arm/app/bin/motrix-fnos-server
```

预期 manifest 和桌面入口只有 `17080`，`MotrixFNOS.sc` 精确包含 `17080/tcp,17082/tcp`，`17081` 只出现在生命周期脚本的回环监听默认值中；两个 staged `app/data/` 目录均为空。

预组装还会拒绝非空的 `app/data/` 和不符合 `motrix_<version>_x86.fpk` / `motrix_<version>_arm.fpk` 的产物名，避免本机 SQLite、日志、PID 或架构错误的 FPK 进入发布目录。

完整构建后可在临时目录解包，不要直接修改产物：

```bash
mkdir -p /tmp/motrix-fpk-check/x86 /tmp/motrix-fpk-check/arm
tar -xzf packaging/fnos/dist/motrix_<version>_x86.fpk -C /tmp/motrix-fpk-check/x86
tar -xzf packaging/fnos/dist/motrix_<version>_arm.fpk -C /tmp/motrix-fpk-check/arm
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
- `ICON.PNG`、`ICON_256.PNG`：FPK 包根图标
- `app/bin/`：server 与 `aria2-next`
- `app/ui/images/`：应用入口图标
- `app/ui/dist/`：Web UI 静态资源
- `app/data/`：运行时数据目录
- `.stage/`：预组装后的打包目录
- `dist/`：最终输出的 `.fpk`

约定：

- 源码态 `packaging/fnos/` **不是**可直接执行 `fnpack build` 的安全输入目录；真实打包输入由 `build:fpk:prepare` / `build:fpk*` 生成到 `.stage/<target>/`。
- 如需手动检查 manifest、入口配置、端口配置或生命周期脚本，请检查 `.stage/x86/` 或 `.stage/arm/`，不要直接在源码态目录执行 `fnpack build`。
- `dist/`、`.stage/`、`app/bin/` 中构建脚本放置的 server / Aria2 二进制、`app/ui/dist/`、`dist/*.fpk` 和 stage 目录内产物都是本地生成产物，不应作为源码态内容长期保留。
- `assets/aria2/aria2-next-*` 是当前 `scripts/build/stage-aria2-sidecar.mjs` 使用的 sidecar 源资产，不是无用产物；只有未来改成下载缓存或发布资产拉取模式后，才可重新评估是否从仓库移除。

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

`cmd/status` 是只读状态查询：服务就绪返回 `0`，进程存在但 ready 接口未返回 HTTP 200 时返回 `1`，未运行或 PID 身份不匹配时返回 `3`。稳定查询不创建运行目录、不轮转或追加日志，也不删除陈旧 PID；PID 清理由 `start`、`stop` 等明确生命周期动作负责。

`cmd/stop` 必须在平台生命周期窗口内完成有界收敛：先请求 server 优雅退出，再对身份仍匹配的记录进程依次使用 `TERM`、`KILL` 兜底。卸载初始化不得忽略停止失败，卸载回调在 server 或所属 Aria2 sidecar 仍存活时不得清理应用数据。启动脚本只清理能够通过 PID、启动时间、UID 和可执行文件归属证明属于当前应用的孤儿进程；无法证明归属的端口占用只允许报告错误。`lifecycle.log` 必须记录 `INT`、`TERM`、`KILL` 各阶段与最终退出结果，便于区分正常优雅退出、超时升级和无法确认归属。

`/api/app/ready` 表示 Rust 管理与 JSON-RPC listener 已绑定、启动门禁已完成且服务未进入退出状态。SQLite 初始化仍由启动门禁保证，但每次 ready 请求不再执行实时数据库查询。

常看位置：

- Rust 业务日志：`packaging/fnos/app/data/logs/server.log`，单文件上限 10 MiB，保留当前文件和最多 3 个轮转文件（`.1`～`.3`）。
- 生命周期日志：`packaging/fnos/app/data/logs/lifecycle.log`，记录启动脚本和进程标准输出，单文件默认上限 1 MiB，保留最多 3 个轮转文件。
- Aria2 原生日志：运行时 `$TRIM_PKGVAR/aria2/aria2.log`（常见为 `/vol1/@appdata/motrix/aria2/aria2.log`），默认级别为 `warn`，单文件上限 10 MiB，总计最多保留当前文件和 2 个历史文件；spdlog 使用 `aria2.1.log`、`aria2.2.log` 等命名，升级维护同时兼容旧 `aria2.log.1`、`aria2.log.2` 命名。
- PID：`packaging/fnos/app/data/run/motrix-fnos-server.pid`
- 进程启动时间：`packaging/fnos/app/data/run/motrix-fnos-server.starttime`，与 `/proc/<pid>/exe` 一起用于防止 PID 复用误判。
- 新进程启动后允许 `nohup` 到 server 可执行文件存在短暂、有限的 exec 过渡窗口；过渡期间使用 PID 启动时间确认仍是本次创建的进程。启动失败时只终止启动时间匹配的进程实例，确认退出后才删除 PID 记录，避免遗留继续占用服务端口的孤儿进程。

启动门禁在完成现有孤儿进程对账后、任何 Aria2 可能启动前维护旧原生日志。只有能够确认 sidecar 未运行、生命周期已停止且内存/磁盘运行态均不存在时，才把超过 10 MiB 的旧 `aria2.log` 原子收敛为最后 10 MiB，并在新旧命名中总计只保留两份历史日志。门禁无法证明安全时只记录警告并跳过，不会为了清理日志停止或唤醒 Aria2。

## 最小排障

- 安装失败：先检查包架构是否与设备一致
- 启动失败：先看 `lifecycle.log`，再看 `server.log`；两者都只保留有限数量的历史文件。
- Web UI 打不开：先看 `cmd/status` 和浏览器请求地址。桌面入口应打开 `http://<设备>:<service_port>/?v=<version>`；同时确认 staged `app/ui/config` 不含 `gatewayPrefix` 或 `gatewaySocket`，Rust server 的同一端口能返回根 HTML 与 `/api/app/ping`。
- 下载失败：先看保存目录权限、Aria2 sidecar 和诊断日志；需要区分应用内调试记录、Rust `server.log`、`lifecycle.log` 与 Aria2 原生文件日志。
- 同一 `motrix` 身份升级后任务或设置丢失：确认 `cmd/uninstall_callback` 默认保留 `TRIM_PKGVAR`，且未收到卸载向导删除数据变量
- 从旧 `motrix.fnos` 安装切换后看不到原数据：这是应用身份变化的预期结果，不属于普通升级；新应用不会自动读取旧身份的 `TRIM_PKGVAR`
- 卸载后重装仍有旧任务：这是默认保留数据的预期行为；如需完全清理，卸载时开启“同时删除 Motrix 应用数据”

### 无需 SSH 的日志排障

普通用户优先在 Motrix 页面完成以下流程，无需访问 `$TRIM_PKGVAR` 或浏览器开发者工具：

登录页本身提供“复制登录排障信息”和“下载登录诊断”两个按钮。它们不要求登录：复制内容只包含访问协议、Origin、是否 iframe、Cookie 是否启用、安全上下文和 User-Agent；下载的 `motrix-fnos-login-diagnostic.zip` 只包含版本/监听地址摘要、Secure Cookie 开关、脱敏鉴权记录和生命周期日志尾部，不包含密码、Cookie、Session、CSRF、Token、SQLite、Aria2 或下载内容。请把复制的文本和 ZIP 一起附在 Issue 中。

已登录时再按以下流程收集完整运行诊断，无需访问 `$TRIM_PKGVAR`：

1. 打开“诊断”，先查看 Aria2、Rust 服务、生命周期和日志总占用；达到 80 MiB 预警线时会显示明确警告。
2. 问题难以复现时，在操作前开启“详细日志（30 分钟）”。该模式只把 Aria2 文件日志临时切换为 `debug`，30 分钟后自动恢复 `warn`，单文件 10 MiB 和最多 3 个文件的上限不会变化。
3. 重现问题后立即点击“导出诊断包”，将下载的 `motrix-fnos-diagnostic-bundle.zip` 随 GitHub Issue 提交。诊断包包含版本/运行状态摘要、应用内调试记录，以及 Rust、生命周期和 Aria2 日志尾部；所有文本会再次脱敏。
4. 需要释放 Aria2 日志空间时先停止引擎，再点击“清理 Aria2 日志”并二次确认。运行中、切换中或归属无法确认时界面禁用操作，服务端也会返回 `409 aria2_log_in_use`，不会自动停止引擎。
5. 调试日志窗口中的“清空应用内调试记录”只清空内存记录，不会释放 Aria2 原生日志文件空间；磁盘占用以诊断页指标为准。

诊断 ZIP 在内存中生成，不创建长期临时文件。它不会包含 SQLite、session、运行态 JSON、设置原文、密码、Token、Cookie、CSRF 或用户下载文件；固定日志输入总计最多 16 MiB，并拒绝符号链接。若升级前已有 50 GiB 等超限 `aria2.log`，下一次安全启动会保留最后 10 MiB；停止引擎后也可用手动按钮清空全部已识别 Aria2 日志。

## 数据保留与卸载向导

fnOS 会在卸载时保留应用 `var` 类用户数据目录；本项目也以保留用户数据为默认策略：

- 同一 `appname=motrix` 身份内升级必须保留 `TRIM_PKGVAR` 中的 SQLite、设置、JSON-RPC 密钥、Aria2 session 和日志。
- 从旧 `appname=motrix.fnos` 切换到 `motrix` 时，fnOS 按两个应用处理，旧数据和 JSON-RPC Token 不会自动迁移；旧应用必须先停止或卸载，避免两个应用争用 `17080`、`17081`、`17082`。
- 卸载默认保留 `TRIM_PKGVAR`，便于后续重装继续使用原任务和设置。
- 只有卸载向导 `MOTRIX_FNOS_DELETE_APP_DATA` 被用户明确开启时，`cmd/uninstall_callback` 才会清理 `TRIM_PKGVAR`。
- 清理范围仅限 Motrix 应用私有数据；用户下载目录和已下载文件不在清理范围内。
- 忘记管理密码时只能在 NAS 本机停止应用后执行 `reset-web-auth`；该命令不得通过公网触发，只清除 Web 鉴权并保留任务、Aria2 session、下载设置、JSON-RPC Token 和授权目录。
- 卸载向导的 `switch` 不设置 `initValue`。当前实测中字符串不能可靠表达默认状态，布尔值会导致 fnpack 校验失败；在官方规则明确前保持省略。

### 升级前备份与回滚

- 实机升级前先停止应用，再备份 fnOS 实际提供的 `TRIM_PKGVAR` 目录；运行中的 SQLite 与 Aria2 session 不作为可靠备份源。
- 记录升级前的任务数量、下载设置、授权目录、Aria2 session 校验值和 JSON-RPC Token“是否已配置”，不要把 Token 原文写入验证记录。
- 新包升级后应先在 NAS 本机确认 `127.0.0.1:17081/jsonrpc`，再修改 Lucky；启用局域网入口后还需从真实 RFC1918 客户端确认 `17082/jsonrpc`，并验证回环、公网、IPv6 来源被拒绝。
- 回滚需要恢复旧 FPK 和升级前应用数据备份，并将 Lucky 后端恢复为旧版对应地址；不得只把 Lucky 改回 `17080` 而继续运行不提供该 JSON-RPC 路由的新 server。

### 验证结论分级

- “本地构建通过”只表示自动化测试、双架构交叉编译、FPK 预检、解包和静态内容检查通过。
- “ARM/x86 实机通过”必须分别完成对应架构的安装或升级、启动/停止、监听地址、管理登录、数据保留和恢复命令验证。
- “公网链路通过”还必须在真实 IPv4-only 与原生 IPv6 网络完成 Cloudflare、Lucky、HTTP、CORS 和 WebSocket 矩阵。
- 未取得实机或外网证据时，只能记录为“待验证”，不得由本地构建结果推断通过。

## FPK 实机验证矩阵

下列项目用于 FPK 发布前及版本升级时的 fnOS 实机验证；未完成的项不得在文档中宣称“已验证通过”。

| 场景 | 操作 | 预期结果 | 重点观察 |
| --- | --- | --- | --- |
| 安装 | 安装匹配架构的 `.fpk` | 应用中心安装成功 | 安装界面报错、`TRIM_TEMP_LOGFILE`、应用中心任务日志 |
| 启动 | 在应用中心或 `appcenter-cli start` 启动 | 服务进入运行中，Web UI 可打开 | `cmd/status`、`lifecycle.log`、`server.log`、监听端口 |
| 停止 | 在应用中心或 `appcenter-cli stop` 停止 | 服务退出，状态变为未运行 | `cmd/status`、PID 文件是否清理 |
| 配置变更 | 在“应用设置”修改其他应用配置并保存 | 不触碰官方 API 授权快照 | `app/data/accessible-paths.json`、`lifecycle.log`、配置保存日志 |
| 同身份升级 | 从旧版 `motrix` 升级到新版 `motrix` | 数据与配置保留，服务可重新启动 | 升级界面日志、任务数据、`server.log` |
| 旧身份切换 | 从 `motrix.fnos` 改装为 `motrix` | 作为新应用安装，不自动迁移旧数据；旧应用不再占用端口 | 两个 appname 的数据目录、JSON-RPC Token、`17080`/`17081`/`17082` 监听进程 |
| 卸载（默认） | 卸载应用且不勾选删除数据 | `TRIM_PKGVAR` 应用数据保留，不删除用户下载文件 | 卸载向导选项、`cmd/uninstall_callback` 日志、`TRIM_PKGVAR` 内容 |
| 卸载（删除数据） | 卸载应用并勾选“同时删除 Motrix 应用数据” | 仅清理 `TRIM_PKGVAR` 内的 Motrix 应用数据，不删除用户下载文件 | `cmd/uninstall_callback` 日志、数据库/设置/session/log 是否被清理 |

建议实机验证命令：

```bash
appcenter-cli install-fpk <package>.fpk
appcenter-cli start motrix
appcenter-cli stop motrix
appcenter-cli list
```

如涉及向导或交互式配置，优先使用应用中心界面完成；`appcenter-cli` 更适合重复安装和脚本化验证。

## 相关文档

- 长期架构：`docs/architecture.md`
- 开发计划与后续清单：`docs/future-development-plan.md`
- 接口契约：`docs/api-contract.md`

## GitHub Actions 自动发版流程

当前默认发版入口是 `Release FPK` workflow。正常路径只需要人工输入一次版本号：

```text
Actions -> Release FPK -> Run workflow -> 输入 x.y.z
```

后续流程自动完成：

```text
Release FPK
  -> 读取 latest tag..HEAD 的 commit subject/body 与本地文件统计
  -> 通过 Cloudflare Workers AI 提取、归并、编辑和复核 CHANGELOG
  -> 同步 package / Cargo / FPK manifest / UI cache 版本
  -> 更新 Cargo.lock
  -> x86 / ARM FPK 构建
  -> 校验产物、生成双架构 SPDX SBOM 和 SHA256SUMS.txt
  -> 对 FPK、SBOM 和 SHA256SUMS.txt 生成 provenance/attestation
  -> 提交 `chore: 发布 x.y.z 版本` 到 main
  -> 创建 `v<x.y.z>` tag
  -> 创建或更新 GitHub Release
```

自动发版不再包含已退役的 GitHub Models 适配层。若 `CHANGELOG.md` 已包含目标版本的合法条目，workflow 会直接复用且不调用模型；否则通过 Cloudflare Workers AI 的 OpenAI-compatible API 分批提取发布事实，再完成归并、编辑和独立复核。模型、凭证或配额异常会中止发布，并提示人工预写目标版本 CHANGELOG，不会静默退回逐 commit 日志。发布正文在创建 Release 前仍会执行严格分类结构校验，格式非法会中止发布。

仓库需要配置两个 Actions Secrets：

- `CLOUDFLARE_ACCOUNT_ID`：Workers AI 所属账户的 32 位 Account ID。
- `CLOUDFLARE_API_TOKEN`：授予该账户 Workers AI Read 权限的自定义 API Token。Actions 只调用模型，不读取 AI Gateway Logs API；创建或修改 Gateway 时使用另一个临时 Token，并额外授予 AI Gateway Edit。

Cloudflare 中还需创建 ID 为 `motrix-fnos-release` 的 AI Gateway，并将 Workers AI Billing 设为 Standard billing。默认分析与编辑模型均为 `@cf/openai/gpt-oss-120b`，请求通过该 Gateway 执行，强制保留调用元数据并关闭 payload 存储。Actions 的 Job summary 会显示版本信息、CHANGELOG、下载链接、SHA256 校验文件和 SBOM；脚本不读取 Gateway Logs，也不推算 Workers AI 剩余额度，账户用量和余额以 Cloudflare Workers AI Dashboard 为准。如需临时更换模型，可修改 workflow 中的 `MOTRIX_RELEASE_ANALYSIS_MODEL` 和 `MOTRIX_RELEASE_EDITOR_MODEL`。不得把 Account ID 或 Token 写入仓库文件。

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

- `motrix_<version>_x86.fpk`
- `motrix_<version>_arm.fpk`
- `motrix_<version>_x86.fpk.spdx.json`
- `motrix_<version>_arm.fpk.spdx.json`
- `SHA256SUMS.txt`

`Release FPK` 在同一个 workflow 内完成版本准备、双架构构建、产物验证、提交、打 tag 和上传 Release，不依赖 PR 自动批准、自动合并，也不依赖 `GITHUB_TOKEN` 推送 tag 后再触发另一个 workflow。

### 验证触发策略

- `pre-commit` 只执行版本、暂存区空白和 Rust 格式检查，不运行前端类型检查、单元测试或生产构建。
- `pre-push` 只在推送分支源码时执行完整 `pnpm run verify`；只推送 tag 时跳过。正常分支推送必须在本地通过全部脚本、Rust、前端测试和构建。
- GitHub `Verify` 只支持 `workflow_dispatch` 手动触发，不随 `main` push 或 PR 自动运行，避免和本地 `pre-push` 重复。
- Release 只允许修改 `CHANGELOG.md` 和固定版本文件；这些发布元数据变化与已经通过本地验证的业务源码视为等价，出现白名单外改动时立即中止。
- `Release FPK` 不重复运行源码测试、依赖审计，也不查询 GitHub `Verify`；它只生成版本文件、构建双架构 FPK，并解包验证、签署和发布产物。
- 自动生成的版本提交和内部推送都使用 `--no-verify`，避免 GitHub runner 因安装本地 hooks 而隐藏重复完整验证。
- `Dependency Audit` 与源码验证和 Release 分离，每周一北京时间 03:23 定时执行。

### GitHub Actions 缓存策略

- `Verify` 只缓存 pnpm store 和 Cargo registry，不缓存 `server/target` 编译产物。
- Rust `server/target` 缓存体积容易达到数百 MB 到 1GB，且版本号 / `Cargo.lock` 变化会产生新 key；当前项目优先控制缓存占用，而不是追求最大 CI 加速。
- `Cleanup Actions Caches` 手动执行默认删除全部 Actions caches；如只想删除非 `main` 分支缓存，可在运行 workflow 时将 `scope` 选为 `non-main`。
- 清空缓存不会影响源码或 Release 产物，只会让下一次 CI 重新下载 / 编译依赖。

### 本地发版备用流程

如 GitHub 自动 PR 流程异常，可在本地使用备用命令：

```bash
rtk pnpm run release:prepare <x.y.z>
rtk git push --atomic origin HEAD v<x.y.z>
```

本地命令会复用 `CHANGELOG.md` 中已填写的目标版本条目。未配置 provider 时按 commit log 生成确定性草稿；需要复用 Cloudflare AI 总结时，在本地环境设置 `MOTRIX_RELEASE_CHANGELOG_PROVIDER=cloudflare-workers-ai`、`CLOUDFLARE_ACCOUNT_ID` 和 `CLOUDFLARE_API_TOKEN`。单次原子推送只触发一次 `pre-push` 完整验证，同时发布版本提交和 tag。
