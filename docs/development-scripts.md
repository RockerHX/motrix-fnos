# 开发脚本说明

## 作用与维护规则

本文档说明 `package.json` 中公开的 `pnpm run` 命令，包括用途、前置条件、文件副作用和使用注意事项。FPK 的目录结构、端口约束、产物检查和实机流程仍以 [FPK 打包说明](fpk-packaging.md) 为准。

固定规则：

- 在仓库根目录执行命令，先运行 `pnpm install` 安装依赖。
- Node.js 版本以仓库根目录 `.node-version` 为准，pnpm 版本以 `package.json` 的 `packageManager` 为准；CI 使用锁定版本和 `pnpm install --frozen-lockfile`。
- Rust 版本以 `rust-toolchain.toml` 为准；FPK 交叉构建 target 固定为 `x86_64-unknown-linux-gnu` 和 `aarch64-unknown-linux-gnu`。
- GitHub Actions 的第三方动作使用不可变 commit SHA，并在行尾保留版本注释；升级动作时必须同时更新 SHA、注释和静态检查。
- `package.json` 的 `scripts` 是命令清单的唯一事实来源；新增、删除或改变命令行为时同步更新本文档。
- 生成物、stage、FPK、交叉编译二进制和本地缓存不应提交。
- 执行会写文件或删除文件的命令前先检查工作区；版本、发布和清理命令尤其如此。
- 代码提交前使用 `pnpm run verify:pre-commit` 做快速静态检查，分支 `git push` 前由 Git hook 执行一次完整 `pnpm run verify`；GitHub `Verify` 只保留手动触发入口，依赖审计由独立的每周 workflow 执行。

## 命令速查

| 命令 | 主要用途 | 是否写入或删除文件 |
| --- | --- | --- |
| `dev` | 启动 Web UI 开发服务器 | Vite 可能写本地缓存 |
| `build` | 类型检查并构建 Web UI | 写入 `dist/` |
| `preview` | 本地预览已构建 Web UI | 否 |
| `test:scripts` | 测试构建、版本与发布脚本 | 测试只使用临时目录 |
| `test:unit` | 运行前端单元测试 | 可能写测试缓存 |
| `typecheck` | 只运行前端 TypeScript 检查 | 否 |
| `clean` | 删除前端和 FPK 生成物 | 是，删除生成物 |
| `clean:dry-run` | 预览 `clean` 的删除范围 | 否 |
| `clean:rust` | 删除整个 Rust `target` | 是，删除编译缓存与产物 |
| `version:set` | 显式同步项目版本 | 是，修改四个版本源 |
| `version:check` | 检查四个版本源是否一致 | 否 |
| `release:prepare` | 本地准备正式版本、日志、commit 和 tag | 是，属于高影响命令 |
| `release:notes` | 从 `CHANGELOG.md` 提取某版本发布正文 | 否 |
| `verify` | 执行发布前完整验证 | 写 Rust 与前端构建缓存 |
| `verify:pre-commit` | 执行提交前快速静态检查 | 否 |
| `verify:fpk` | 解包验收已生成的双架构 FPK | 只写临时解包目录 |
| `prepare` | 安装依赖后尝试配置 Git hooks | 修改本仓库 Git 配置 |
| `hooks:install` | 显式配置 Git hooks | 修改本仓库 Git 配置 |
| `build:server:linux:x64` | 交叉编译 x86 Linux server | 写入 `server/target/` |
| `build:web:fpk` | 构建并同步 FPK Web UI | 写入 `dist/` 和 FPK UI 目录 |
| `assets:aria2:fetch` | 下载或校验固定版本 Aria2 Next | 可能修改 `assets/aria2/` |
| `stage:aria2:x64` | 放置 x86 Aria2 sidecar | 覆盖共享 sidecar 文件 |
| `stage:aria2:arm64` | 放置 ARM Aria2 sidecar | 覆盖共享 sidecar 文件 |
| `build:fpk:x64` | 构建一个 x86 FPK | 写 stage、编译产物和 FPK |
| `build:fpk:arm64` | 构建一个 ARM FPK | 写 stage、编译产物和 FPK |
| `build:fpk:prepare` | 双架构预组装与预检，不调用 fnpack | 写双架构 stage 和编译产物 |
| `build:fpk:artifacts` | 只构建 x86 与 ARM 两个 FPK，供 Release 调用 | 重建 FPK 输出目录 |
| `build:fpk` | 完整验证源码并构建、验收双架构 FPK | 写构建缓存并重建 FPK 输出目录 |

## Web UI 开发

### `pnpm run dev`

启动 Vite 开发服务器，用于前端交互开发和热更新。

注意：

- 该命令只启动 Web UI，不会启动 Rust server、SQLite 或 Aria2。
- 依赖管理 API 的页面需要另行启动后端或配置开发代理。
- 不用于验证最终 FPK 的静态资源路径和 fnOS WebView 行为。

### `pnpm run typecheck`

运行 `vue-tsc --noEmit`，只检查 Vue 与 TypeScript 类型，不生成 Web 产物。适合修改过程中快速发现类型错误。

### `pnpm run build`

先运行 `vue-tsc --noEmit`，通过后执行 Vite 生产构建，输出到根目录 `dist/`。两个子阶段通过同一进度通道更新当前任务，仍各自只执行一次。成功路径不打印模块转换和 chunk 明细；构建 warning 与 error 仍正常输出。

注意：该产物是通用 Web 构建结果；进入 FPK 前还要由 `build:web:fpk` 同步到 `packaging/fnos/app/ui/dist/`。

### `pnpm run preview`

使用 Vite 本地预览根目录 `dist/`。应先执行 `pnpm run build`；它不代表 fnOS 入口、权限、listener 或 FPK 生命周期已经验证。

## 测试与验证

### `pnpm run test:unit`

使用 Vitest 运行全部前端 `*.spec.ts` 单元测试。自定义 reporter 继承 minimal reporter，不打印成功文件和 queued 状态；交互终端的第二行进度实时显示刚开始执行的测试文件和用例。失败详情、warning、error 与最终统计仍正常输出。适合前端 service、store、组件和启动编排变更。

### `pnpm run test:scripts`

使用 Node.js test runner 运行 `scripts/tests/*.test.mjs`，覆盖版本同步、发布日志、FPK 预检和其他构建脚本的纯逻辑或临时仓库测试。测试不得直接改动真实项目版本或发布状态。成功项不逐条打印，多行统计合并为单行摘要；失败和跳过项保留完整信息。耗时小于 1000ms 时以两位小数的毫秒显示，达到 1000ms 后转换为两位小数的秒。

### `pnpm run verify:pre-commit`

提交前快速验证，依次执行：

1. 项目版本一致性检查；
2. Rust 格式检查。

Git hook 还会对暂存区执行空白检查。该阶段不执行前端类型检查、脚本测试、Shell 测试、Rust 测试、前端单元测试或任何生产构建，目标是在约 1 秒内发现基础问题。

### `pnpm run verify`

推送前完整验证。它执行版本和格式检查、构建与发布脚本测试、FPK Shell 测试、Rust 测试与编译、前端单元测试，并通过一次 `pnpm run build` 完成唯一一次前端类型检查和生产构建。Rust 成功测试不逐项打印 `ok`，失败详情仍完整输出。交互终端中的长步骤使用两行动态区域：第一行显示阶段名称和累计耗时，第二行显示当前 Node/Vitest 测试、最近完成的 Rust 测试、Cargo crate 或构建子阶段；非交互日志每 30 秒输出一次包含当前子任务的心跳。步骤结束后再输出精简摘要，失败时完整回放 stdout 和 stderr。该命令仍不代替 FPK 解包检查或 fnOS 实机验证。

### `pnpm run verify:fpk`

要求 `packaging/fnos/dist/` 中存在版本匹配的 x86 与 ARM 两个 FPK，逐一解包检查 manifest、端口配置、生命周期脚本、Web UI、双架构 server/sidecar 和空运行数据目录。缺少产物时直接失败，不得静默跳过。

### `pnpm run audit:deps`

使用锁定的 `server/Cargo.lock` 和 `pnpm-lock.yaml` 检查 Rust 与前端生产依赖。运行前需要安装固定版本的 `cargo-audit 0.22.2`；高危和严重漏洞返回失败，中低危打印报告但不阻断，审计工具缺失或无法解析结果时返回失败。该命令不会自动升级依赖，由 `Dependency Audit` workflow 每周一北京时间 03:23 自动执行，也可手动触发。

快速验证不写 Rust 构建产物；完整验证会保留 Rust 编译缓存，避免每次推送前重新编译已验证的依赖和测试目标。磁盘空间不足时，再显式执行 `pnpm run clean:rust` 回收整个 Rust 构建目录。

## 清理命令

### `pnpm run clean:dry-run`

只列出 `clean` 将删除的路径，不执行删除。清理前优先运行该命令。

### `pnpm run clean`

删除以下生成内容：

- 根目录 `dist/`；
- FPK 中临时放置的 server、Aria2 sidecar、入口图标和 Web UI；
- `packaging/fnos/.stage/`；
- `packaging/fnos/dist/` 与旧的固定名称 FPK；
- 仓库内除忽略目录外的 `.DS_Store`。

它不会删除 `server/target/`，也不会删除源代码、SQLite 实机数据或仓库内置的 `assets/aria2/` 源资产。

### `pnpm run clean:rust`

删除整个 `server/target/`。下一次 Rust 测试、编译或 FPK 构建需要完整重建，耗时会明显增加。

`server/target/` 是 Rust 的本地编译缓存，不会提交到 Git。`debug/deps` 会保存调试测试产物，双架构 FPK 构建还会生成 x86 和 ARM 的 release 缓存，因此目录可能达到数 GB。验证不会自动删除这些缓存；磁盘紧张时再手动执行 `pnpm run clean:rust`。

## 版本命令

项目版本由以下四处共同表示：

- `package.json`；
- `server/Cargo.toml`；
- `packaging/fnos/manifest.template`；
- `packaging/fnos/app/ui/config` 中的缓存查询参数。

允许的项目版本格式是正式版 `x.y.z` 和 fnOS 可升级的 beta 测试版 `x.y.z-beta`。GitHub 正式发布仍只接受 `x.y.z`。

### `pnpm run version:check`

读取并比较四个版本源。任一来源缺失、格式错误或与 `package.json` 不一致都会失败。该检查不修改文件。

### `pnpm run version:set <version>`

把四个版本源显式同步为指定版本，例如：

```bash
pnpm run version:set 1.7.4
pnpm run version:set 1.7.5-beta
```

该命令不构建 FPK、不更新 CHANGELOG、不创建 commit/tag，也不自动恢复旧版本。Cargo 后续构建可能同步更新 `server/Cargo.lock`，提交前应检查全部版本改动。

### 本地 FPK 测试版本

fnOS 的实机升级规则要求测试版使用 beta 版本链：正式版先升级到同一核心版本的 `-beta`，再升级到对应的正式版；同一 beta 版本不能重复安装。项目不再提供自动递增测试版本命令，测试版本由 `version:set` 显式指定。

推荐测试流程：

1. 确认工作区中的版本文件没有未预期改动；
2. 执行 `pnpm run version:set 1.7.5-beta`；
3. 执行 `pnpm run version:check`；
4. 构建并安装对应架构 FPK；
5. 测试完成后执行 `pnpm run version:set 1.7.5` 验证 beta 到正式版升级；
6. 测试结束后按实际目标继续迭代，或将版本文件恢复为正式开发版本。

beta 测试版本只用于本地安装验证，不应创建 GitHub Release 或正式 tag。

## 正式发布命令

### `pnpm run release:notes <x.y.z> [--body]`

从 `CHANGELOG.md` 提取指定版本，并在输出前执行严格结构校验。默认输出版本标题和正文；追加 `--body` 时只输出正文。该命令不修改 CHANGELOG 或 Git 状态。

### `pnpm run release:prepare <x.y.z>`

本地正式发布准备命令，默认会：

1. 校验目标是高于当前版本的正式 `x.y.z`；
2. 复用已有目标版本 CHANGELOG，或按两个版本之间的 commit subject/body 生成确定性的分类发布日志；
3. 同步版本文件并更新 CHANGELOG；
4. 暂存固定的发布文件并创建中文 release commit；
5. 创建 `v<x.y.z>` tag。

该命令不再自行运行完整验证。准备完成后使用它输出的单次原子推送命令，由 `pre-push` 对包含版本提交的分支执行唯一一次完整验证，并同时推送分支和 tag。

常用的只读预演：

```bash
pnpm run release:prepare 1.7.4 --dry-run
```

可用参数还有 `--from <tag>`、`--no-commit` 和 `--no-tag`。后两个参数主要供受控自动化或故障排查使用，日常正式发布不应随意跳过提交或 tag。

高影响注意事项：

- 命令会拒绝接管无关的脏工作区；执行前先提交、暂存到安全位置或恢复无关改动。
- 发布日志不依赖外部模型服务：已有合法目标版本条目时直接复用；否则按 commit subject/body 的 Conventional Commit 分类生成确定性草稿。生成后的版本条目仍须通过分类结构校验。
- 该命令只用于正式版本，不接受 `-beta`。
- GitHub Actions 的 Release workflow 仍是远程正式发版入口；本地命令不能替代 Actions 权限、产物上传和双架构发布检查。

## Git hooks

### `pnpm run prepare`

通常由 `pnpm install` 自动调用，尝试执行：

```bash
git config core.hooksPath .githooks
```

为兼容非 Git 安装环境，该命令配置失败时不会让依赖安装失败。

### `pnpm run hooks:install`

显式安装本仓库 Git hooks，配置失败会返回非零退出码。

当前 hook 行为：

- 所有提交都先对暂存区执行空白检查；Markdown 允许用两个行尾空格表示强制换行。
- 暂存区只有 `docs/`、Markdown/文本文档或常见图片、字体、音视频资源时，完成空白检查后跳过版本和 Rust 格式检查。
- 暂存区包含代码、配置、脚本，或同时包含代码与文档/资源时，执行完整 `verify:pre-commit`。
- 删除 Markdown、图片等非代码资源时，也按同样规则跳过测试，不会因为删除动作被误判为空暂存区。
- 没有暂存文件时采用保守策略，仍执行 `verify:pre-commit`。
- `pre-push` 只在推送分支源码时执行完整 `pnpm run verify`；只推送 tag 或删除远端引用时跳过源码验证。

因此只提交文档或图片通常会很快完成；代码提交只做快速静态检查，推送前再集中执行一次完整测试和构建。GitHub `Verify` 不随 `main` push 自动运行，需要远端复核时手动触发。

## FPK 构建与资产命令

这些命令的完整约束和产物检查见 [FPK 打包说明](fpk-packaging.md)。交叉编译需要 Rust/rustup、对应 Rust targets、`cargo-zigbuild` 和 Zig；完整打包还需要 `fnpack`，脚本在未找到时会下载已验证的 `fnpack 1.2.1`。

### `pnpm run build:server:linux:x64`

使用 `cargo zigbuild` 为 `x86_64-unknown-linux-gnu` 构建 release server，默认 glibc baseline 为 `2.36`。产物位于：

```text
server/target/x86_64-unknown-linux-gnu/release/motrix-fnos-server
```

双架构 FPK 构建会直接调用底层脚本并传入对应 target；该公开命令只构建 x86。

### `pnpm run build:web:fpk`

先执行 `pnpm run build`，再清空并重建 `packaging/fnos/app/ui/dist/`。不要在该目标目录保存手工文件。

### `pnpm run assets:aria2:fetch`

从 Aria2 Next 对应 GitHub Release 下载 checksum 和缺失的 x86/ARM 二进制，校验 SHA-256 并设置可执行权限。它是维护固定 sidecar 版本的联网命令，不是每次 FPK 构建都要执行的安装步骤。

如果更新了脚本中的 Aria2 Next 版本，应同时审查 checksum、双架构二进制、许可证和实机兼容性；不得盲目提交下载结果。

### `pnpm run stage:aria2:x64` 与 `pnpm run stage:aria2:arm64`

把指定架构的仓库内置 sidecar 复制为：

```text
packaging/fnos/app/bin/aria2-next
```

两个命令写入同一个目标，后执行的架构会覆盖先前文件。它们主要供 FPK 编排脚本调用；不要把共享目标目录中当前恰好存在的架构当作最终双架构产物依据，应检查各自 `.stage/<platform>/`。

### `pnpm run build:fpk:x64` 与 `pnpm run build:fpk:arm64`

完成指定架构的 server 交叉编译、Web UI 构建、sidecar 放置、独立 stage、manifest 渲染、端口隔离预检和 `fnpack build`。

单架构命令默认清空 `packaging/fnos/dist/` 中已有输出，所以依次手动运行 x86 和 ARM 命令不会保留两个包。需要双架构候选包时使用 `pnpm run build:fpk`。

### `pnpm run build:fpk:prepare`

为 x86 和 ARM 分别完成预组装和全部静态预检，但跳过 `fnpack build`，因此不会生成新的 `.fpk`。它仍会执行双架构 server 编译、Web UI 构建和 sidecar staging，不是轻量级 lint 命令。

Web UI 在双架构循环开始前只构建一次，两个 stage 复用同一份静态资源。

主要检查目录：

```text
packaging/fnos/.stage/x86/
packaging/fnos/.stage/arm/
```

FPK 图标由 `scripts/build-fpk.mjs` 从包根资源同步到 `app/ui/images/`，并在预检阶段统一校验为 256×256。当前项目保留 `icon_64.png` 这个官方入口文件名，但文件内容仍使用 256×256，以保证高清显示；具体文件清单、官方 64/256 规范和浏览器缓存排查见 [FPK 打包说明](fpk-packaging.md) 的“图标尺寸与高清显示”。

### `pnpm run build:fpk:artifacts`

先清空 `packaging/fnos/dist/`，再完整构建 x86 与 ARM，最终保留两个 FPK：

```text
packaging/fnos/dist/motrix_<version>_x86.fpk
packaging/fnos/dist/motrix_<version>_arm.fpk
```

构建会清空源码 staging 区的 `packaging/fnos/app/data/`，以防 SQLite、日志或运行残留进入安装包。该目录只能存放占位内容，不得用于保存本地测试数据。

该命令只负责发布产物，不运行源码测试，供 Release workflow 使用。Web UI 只构建一次并同时进入两个架构的 FPK。

### `pnpm run build:fpk`

本地完整打包入口，固定执行以下链路：

1. 运行一次完整 `pnpm run verify`；
2. 复用刚生成且已通过类型检查的根目录 `dist/`；
3. 构建双架构 server 与 FPK，不重复构建 Web UI；
4. 运行 `pnpm run verify:fpk` 解包验收新产物。

Release 不调用该命令，避免在远端重复源码测试。

## 推荐工作流

### 普通代码提交

```bash
pnpm run verify:pre-commit
git status --short
```

### 本地升级测试包

```bash
pnpm run version:set 1.7.5-beta
pnpm run version:check
pnpm run build:fpk
```

安装前确认包名中的版本和架构正确。真实设备上的数据备份、升级和回滚不由这些本地脚本自动完成。

### 正式发布前检查

```bash
pnpm run build:fpk
```

随后按发布流程检查 CHANGELOG、双架构 SHA-256、解包内容和实机结果。不要把“本地构建通过”写成“fnOS 实机验证通过”。
