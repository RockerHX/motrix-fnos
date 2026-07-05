# FPK 打包说明

## 作用

这份文档只说明 **如何构建和定位 FPK 产物**。

它本身不参与运行时逻辑，也不会影响前后端代码行为；但其中记录的命令、路径、产物命名和 manifest 约定必须与仓库脚本保持一致。

## 当前产物

默认命令会同时生成 x86 与 ARM 两个 FPK：

- x86：`packaging/fnos/dist/motrix.fnos_1.1.0_x86.fpk`
- ARM：`packaging/fnos/dist/motrix.fnos_1.1.0_arm.fpk`

对应 server 二进制：

- x86：`server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server`
- ARM：`server/target/aarch64-unknown-linux-gnu/release/motrix-fnos-server`

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

## 打包目录

当前 FPK 主目录：

```text
packaging/fnos/
```

关键内容：

- `manifest`：FPK 元数据
- `cmd/`：启动、停止、状态脚本
- `config/`：资源与权限声明
- `app/bin/`：server 与 `aria2-next`
- `app/ui/dist/`：Web UI 静态资源
- `app/data/`：运行时数据目录
- `dist/`：最终输出的 `.fpk`

## 本地调试

先执行：

```bash
rtk pnpm run build:fpk:prepare
```

再调试脚本：

```bash
rtk packaging/fnos/cmd/start
rtk packaging/fnos/cmd/status
rtk packaging/fnos/cmd/stop
```

常看两个位置：

- 日志：`packaging/fnos/app/data/logs/server.log`
- PID：`packaging/fnos/app/data/run/motrix-fnos-server.pid`

## 最小排障

- 安装失败：先检查包架构是否与设备一致
- 启动失败：先看 `server.log`
- Web UI 打不开：先看 `cmd/status` 和 `service_port`
- 下载失败：先看保存目录权限、Aria2 sidecar 和诊断日志
- 卸载后重装仍有旧任务：检查 `cmd/uninstall_callback` 是否清理了 `TRIM_PKGVAR` 应用私有数据目录

## 相关文档

- 总体架构：`docs/architecture.md`
- 实机验证：`docs/fnos-manual-test-checklist.md`

## GitHub Release 在线打包

仓库提供 `Release FPK` workflow：

- 推送 `v*` tag 时自动构建 x86 与 ARM FPK，并创建 / 更新 GitHub Release。
- 也可以在 GitHub Actions 页面手动运行 workflow，默认使用 `package.json` / `server/Cargo.toml` / `packaging/fnos/manifest` 中一致的版本号生成 tag。
- workflow 会上传：
  - `motrix.fnos_<version>_x86.fpk`
  - `motrix.fnos_<version>_arm.fpk`
  - `SHA256SUMS.txt`

发布前必须确认 `package.json`、`server/Cargo.toml`、`packaging/fnos/manifest` 三处版本一致；workflow 会在版本不一致时失败。
