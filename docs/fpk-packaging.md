# FPK 打包与交付说明

## 目的

记录飞牛 fnOS 下的 FPK 包结构、`fnpack` 使用方式、构建输入、安装调试和排障流程。

## 当前状态

阶段 4 已完成，当前已经建立 FPK 打包链路并可生成 `.fpk` 产物。阶段 5 正在进行飞牛实机安装和基础功能验证。

默认构建命令 `pnpm run build:fpk` 会连续构建 x86 与 ARM 两个 FPK：

- x86 FPK：`packaging/fnos/dist/motrix.fnos_0.1.0_x86.fpk`
- ARM FPK：`packaging/fnos/dist/motrix.fnos_0.1.0_arm.fpk`
- x86 server 产物：`server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server`
- ARM server 产物：`server/target/aarch64-unknown-linux-gnu/release/motrix-fnos-server`

注意：x86 包不能安装到 ARM 飞牛设备。OES / A311D 等 ARM 设备应安装 ARM 包；如需只构建 ARM 包：

```bash
rtk pnpm run build:fpk:arm64
```

输出：

```text
packaging/fnos/dist/motrix.fnos_0.1.0_arm.fpk
```

## 架构与产物对应关系

| 飞牛设备架构 | `uname -m` | Rust target | FPK platform | FPK 输出 |
| --- | --- | --- | --- | --- |
| x86_64 | `x86_64` | `x86_64-unknown-linux-gnu` | `x86` | `motrix.fnos_0.1.0_x86.fpk` |
| ARM64 | `aarch64` / `arm64` | `aarch64-unknown-linux-gnu` | `arm` | `motrix.fnos_0.1.0_arm.fpk` |

如果平台不匹配，飞牛应用中心会拒绝安装，并提示类似“应用包不符合系统要求”。这不是 FPK-first 架构问题，而是包内 `manifest platform` 与设备 CPU 架构不匹配。

## FPK 目录结构

当前 FPK 目录位于 `packaging/fnos/`：

```text
packaging/fnos/
  manifest
  ICON.PNG
  ICON_256.PNG
  cmd/
    main
    start
    stop
    status
    common.sh
    install_init
    install_callback
    uninstall_init
    uninstall_callback
    upgrade_init
    upgrade_callback
    config_init
    config_callback
  config/
    resource
    privilege
  wizard/
    install
    config
  app/
    bin/
      motrix-fnos-server
      aria2-next
    data/
    ui/
      config
      images/
  ui/
    dist/
```

说明：

- `manifest` 定义应用名、版本、平台、服务端口、Web 入口和 stop 控制能力。
- `cmd/main` 统一分发 `start` / `stop` / `status`。
- `cmd/start` 启动 Rust server，并注入数据目录、监听地址和 Aria2 sidecar 路径。
- `cmd/stop` 通过 `SIGINT` 触发 server 统一退出流程，并最多等待 20 秒。
- `cmd/status` 返回服务运行状态，运行中返回 0，未运行返回 1。
- `app/bin/motrix-fnos-server` 是 Rust 后端服务。
- `app/bin/aria2-next` 是 Linux Aria2 Next sidecar。
- `ui/dist/` 是 Vue Web UI 静态资源。
- `app/data/` 是运行时数据目录，打包前会清理本地残留。

## 构建入口

安装依赖：

```bash
rtk pnpm install
```

双架构预组装验证，不执行 `fnpack build`：

```bash
rtk pnpm run build:fpk:prepare
```

同时构建 x86 与 ARM FPK：

```bash
rtk pnpm run build:fpk
```

只构建 x86 FPK：

```bash
rtk pnpm run build:fpk:x64
```

只构建 ARM FPK：

```bash
rtk pnpm run build:fpk:arm64
```

指定端口构建：

```bash
rtk node scripts/build-fpk-all.mjs --service-port 17080
```

脚本参数：

| 参数 | 说明 |
| --- | --- |
| `--target <triple>` | 低层单架构脚本 `scripts/build-fpk.mjs` 使用；指定 Rust target，默认 `x86_64-unknown-linux-gnu` |
| `--prepare-only` | 只完成构建和组装，不执行 `fnpack build` |
| `--keep-dist` | 低层单架构脚本内部参数；双架构构建时保留已有 dist 产物 |
| `--service-port <port>` | 改写 `manifest service_port` 和 Web UI 入口端口 |
| `--fnpack <path>` | 使用指定 `fnpack` 可执行文件 |

## 脚本分层

- `scripts/build-fpk-all.mjs`：双架构入口，默认构建 `x86_64-unknown-linux-gnu` 和 `aarch64-unknown-linux-gnu`。
- `scripts/build-fpk.mjs`：单架构底层入口，负责构建指定 target、改写临时 manifest、执行 `fnpack build` 并输出对应平台 FPK。
- `scripts/build-server-linux.mjs`：构建 Linux server。
- `scripts/build-web-ui-fpk.mjs`：构建并同步 Web UI。
- `scripts/stage-aria2-sidecar.mjs`：按 target 放置 Aria2 Next sidecar。

## 构建输入

### Rust server

标准构建入口：

```bash
rtk pnpm run build:server:linux:x64
```

底层脚本：

```bash
rtk node scripts/build-server-linux.mjs --target x86_64-unknown-linux-gnu
rtk node scripts/build-server-linux.mjs --target aarch64-unknown-linux-gnu
```

非 Linux x86_64 主机默认通过 `cargo-zigbuild` 执行交叉构建；Linux x86_64 主机在 x86 目标下可直接使用 `cargo build`。脚本会自动检查并安装缺失的 Rust target；如果 `ziglang` 只提供 `python-zig`，脚本会生成临时 `zig` 包装器供 `cargo-zigbuild` 使用。

交叉构建依赖示例（Homebrew Python 如遇 PEP 668 限制，需要保留 `--break-system-packages`）：

```bash
rtk python3 -m pip install --user --break-system-packages cargo-zigbuild ziglang
```

### Web UI

```bash
rtk pnpm run build:web:fpk
```

输出会同步到：

```text
packaging/fnos/ui/dist/
```

### Aria2 Next sidecar

```bash
rtk pnpm run stage:aria2:x64
rtk pnpm run stage:aria2:arm64
```

统一输出到：

```text
packaging/fnos/app/bin/aria2-next
```

## 运行时环境变量

`cmd/common.sh` 会设置以下运行时变量：

| 变量 | 说明 |
| --- | --- |
| `TRIM_APPDEST` | fnOS 注入的应用安装目录；未设置时回退到本地 `packaging/fnos/app` |
| `TRIM_PKGVAR` | fnOS 注入的应用数据目录；未设置时回退到本地 `packaging/fnos/app/data` |
| `TRIM_SERVICE_PORT` | fnOS 注入的服务端口；未设置时默认 `17080` |
| `MOTRIX_FNOS_APP_DATA_DIR` | server 数据目录 |
| `MOTRIX_FNOS_HTTP_ADDR` | server 监听地址，默认 `127.0.0.1:17080` |
| `MOTRIX_FNOS_ARIA2_PATH` | Aria2 sidecar 路径 |
| `MOTRIX_FNOS_SERVER_BIN` | 本地调试时可覆写 server 二进制路径 |

## 本地脚本调试

在本机调试 `cmd/start` / `cmd/stop` / `cmd/status` 时，可先执行预组装：

```bash
rtk pnpm run build:fpk:prepare
```

再运行：

```bash
rtk packaging/fnos/cmd/start
rtk packaging/fnos/cmd/status
rtk packaging/fnos/cmd/stop
```

日志位置：

```text
packaging/fnos/app/data/logs/server.log
```

PID 位置：

```text
packaging/fnos/app/data/run/motrix-fnos-server.pid
```

## 实机安装排障

| 现象 | 优先判断 | 处理 |
| --- | --- | --- |
| 安装失败：“应用包不符合系统要求” | FPK 平台与设备架构不匹配 | ARM 设备使用 `--target aarch64-unknown-linux-gnu` 重新构建 ARM 包 |
| 安装失败但架构匹配 | manifest、权限或 fnpack 产物格式问题 | 查看 fnOS 安装日志，并检查 `manifest platform`、`service_port`、`desktop_uidir` |
| 启动失败 | server 或 sidecar 不可执行、路径错误、端口占用 | 查看 `logs/server.log` 和 `cmd/start` 输出 |
| Web UI 打不开 | 服务未运行或端口配置不一致 | 检查 `cmd/status`、`manifest service_port`、`app/ui/config` |
| 下载失败 | 保存目录权限、Aria2 sidecar 或网络问题 | 查看诊断日志和 server 日志 |

## 与其他文档关系

- 总体架构边界见 `docs/architecture.md`。
- 目标架构专题见 `docs/fnos-fpk-architecture.md`。
- 实机验证步骤见 `docs/fnos-manual-test-checklist.md`。
