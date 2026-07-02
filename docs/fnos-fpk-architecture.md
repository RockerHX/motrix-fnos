# FPK-first 目标架构文档

## 目的

沉淀飞牛 fnOS FPK 交付形态下的目标系统架构，作为 `docs/architecture.md` 的专题展开文档。

## 当前状态

阶段 4 已完成，项目已从文档骨架推进到可生成 FPK 的实现状态。当前进入阶段 5：飞牛实机安装和基础功能验证。

当前默认构建命令会同时生成 x86 与 ARM FPK；ARM 飞牛设备（例如 OES / A311D）必须安装 ARM FPK。

## 目标部署形态

```text
fnOS FPK
  ├─ manifest / config / wizard / icons
  ├─ cmd/
  │  ├─ main
  │  ├─ start
  │  ├─ stop
  │  └─ status
  ├─ Rust server: motrix-fnos-server
  │  ├─ Axum HTTP API
  │  ├─ SSE 事件流
  │  ├─ Aria2 Next 进程管理
  │  ├─ SQLite 持久化
  │  └─ 调试日志与运行时状态
  ├─ Web UI
  │  ├─ Vue 3 + Naive UI + Pinia
  │  ├─ 静态资源 dist/
  │  └─ fnOS Web 入口
  └─ Aria2 Next Linux sidecar
```

## FPK 包结构约定

当前目录位于 `packaging/fnos/`：

```text
packaging/fnos/
  manifest
  ICON.PNG
  ICON_256.PNG
  config/
    resource
    privilege
  wizard/
    install
    config
  cmd/
    main
    common.sh
    start
    stop
    status
    install_init
    install_callback
    uninstall_init
    uninstall_callback
    upgrade_init
    upgrade_callback
    config_init
    config_callback
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

职责边界：

- `manifest`：声明应用名、版本、平台、最低系统版本、服务端口、Web 入口和控制能力。
- `config/privilege`：声明当前运行权限。第三方应用默认使用 `run-as: package`，避免申请 root 权限导致应用中心拒绝安装。
- `cmd/main`：fnOS 控制入口，分发到 `start` / `stop` / `status`。
- `cmd/common.sh`：统一解析 fnOS 注入路径、数据目录、端口、日志和 PID 路径。
- `app/bin/`：放置 Rust server 和 Aria2 sidecar。
- `app/data/`：运行时数据目录，保存 SQLite、session、日志、PID 等运行态文件。
- `ui/dist/`：Web UI 静态资源。
- `app/ui/config`：fnOS Web 入口配置。

## 平台架构策略

FPK 必须与设备 CPU 架构匹配。

| 设备 | CPU 架构 | Rust target | `manifest platform` | 输出包 |
| --- | --- | --- | --- | --- |
| x86 飞牛 | x86_64 | `x86_64-unknown-linux-gnu` | `x86` | `motrix.fnos_0.1.0_x86.fpk` |
| OES / A311D 等 ARM 飞牛 | aarch64 / arm64 | `aarch64-unknown-linux-gnu` | `arm` | `motrix.fnos_0.1.0_arm.fpk` |

`pnpm run build:fpk` 默认同时生成 x86 与 ARM 包。ARM 设备安装 x86 包失败是预期结果，不代表 FPK-first 架构方向错误。

双架构构建命令：

```bash
rtk pnpm run build:fpk
```

仅构建 ARM 包：

```bash
rtk pnpm run build:fpk:arm64
```

## Rust server 进程模型

server 由 `cmd/start` 启动：

1. 准备数据目录、运行目录和日志目录。
2. 注入 `MOTRIX_FNOS_APP_DATA_DIR`、`MOTRIX_FNOS_HTTP_ADDR`、`MOTRIX_FNOS_ARIA2_PATH`。
3. 校验 `motrix-fnos-server` 和 `aria2-next` 可执行。
4. 后台启动 server，并写入 PID 文件。
5. server 内部初始化 SQLite、日志队列、任务状态和 Aria2 运行态。

server 由 `cmd/stop` 停止：

1. 读取 PID 并发送 `SIGINT`。
2. server 执行统一退出流程：广播退出事件、同步任务、暂停未完成任务、保存 Aria2 session、停止受管 Aria2 进程。
3. 脚本等待进程退出并清理 PID。

`cmd/status` 只负责读取 PID 和进程存活状态，不直接访问业务 API。

## Web UI 部署方式

Web UI 构建为静态资源，输出到：

```text
packaging/fnos/ui/dist/
```

前端通过同源相对路径访问后端：

- JSON API：`/api/*`
- SSE：`/api/events`

fnOS Web 入口配置在：

```text
packaging/fnos/app/ui/config
```

当前入口键名：`motrix.fnos.main`。

## Aria2、SQLite、日志和运行态文件

运行态文件必须位于应用数据目录下。`cmd/common.sh` 优先使用 fnOS 注入的 `TRIM_PKGVAR`，本地调试时回退到：

```text
packaging/fnos/app/data/
```

主要文件类别：

- SQLite 数据库
- Aria2 session
- Aria2 log
- server log
- PID 文件
- 其他运行态记录

打包脚本会在最终组装前清空 `app/data/` 的本地运行态残留，避免把 SQLite WAL、PID 或日志误打进 FPK。

## 与 legacy Tauri 资产的边界

`src-tauri/` 仍保留为 legacy 迁移来源和回归参照，但不再作为目标交付主线：

- 不再把 Tauri window / tray / Dock / notification 作为长期运行模型。
- 不再把 `tauri build` 作为 FPK 交付入口。
- 不新增 Tauri-only 主线能力。
- server、Web UI 和 FPK 脚本才是当前交付闭环。

## 与其他文档关系

- 总体边界以 `docs/architecture.md` 为准。
- API 细节见 `docs/api-contract.md`。
- 打包细节见 `docs/fpk-packaging.md`。
- 实机测试见 `docs/fnos-manual-test-checklist.md`。
