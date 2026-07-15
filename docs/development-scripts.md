# 开发脚本说明

## 作用与维护规则

本文档说明 `package.json` 中公开的 `pnpm run` 命令，包括用途、前置条件、文件副作用和使用注意事项。FPK 的目录结构、端口约束、产物检查和实机流程仍以 [FPK 打包说明](fpk-packaging.md) 为准。

固定规则：

- 在仓库根目录执行命令，先运行 `pnpm install` 安装依赖。
- `package.json` 的 `scripts` 是命令清单的唯一事实来源；新增、删除或改变命令行为时同步更新本文档。
- 生成物、stage、FPK、交叉编译二进制和本地缓存不应提交。
- 执行会写文件或删除文件的命令前先检查工作区；版本、发布和清理命令尤其如此。
- 日常提交前使用 `pnpm run verify:pre-commit`，准备正式发布前使用 `pnpm run verify`。

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
| `clean:rust:incremental` | 只删除 Rust incremental 缓存 | 是，保留主要编译缓存 |
| `version:set` | 显式同步项目版本 | 是，修改四个版本源 |
| `version:test` | 迭代本地 FPK 测试版本 | 是，修改四个版本源 |
| `version:check` | 检查四个版本源是否一致 | 否 |
| `release:prepare` | 本地准备正式版本、日志、commit 和 tag | 是，属于高影响命令 |
| `release:notes` | 从 `CHANGELOG.md` 提取某版本发布正文 | 否 |
| `verify` | 执行发布前完整验证 | 写构建缓存并清理 incremental 缓存 |
| `verify:pre-commit` | 执行提交前快速验证 | 写构建缓存并清理 incremental 缓存 |
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
| `build:fpk` | 构建 x86 与 ARM 两个 FPK | 重建 FPK 输出目录 |

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

先运行 `vue-tsc --noEmit`，通过后执行 Vite 生产构建，输出到根目录 `dist/`。

注意：该产物是通用 Web 构建结果；进入 FPK 前还要由 `build:web:fpk` 同步到 `packaging/fnos/app/ui/dist/`。

### `pnpm run preview`

使用 Vite 本地预览根目录 `dist/`。应先执行 `pnpm run build`；它不代表 fnOS 入口、权限、listener 或 FPK 生命周期已经验证。

## 测试与验证

### `pnpm run test:unit`

使用 Vitest 运行全部前端 `*.spec.ts` 测试。适合前端 service、store、组件和启动编排变更。

### `pnpm run test:scripts`

使用 Node.js test runner 运行 `scripts/tests/*.test.mjs`，覆盖版本同步、发布日志、FPK 预检和其他构建脚本的纯逻辑或临时仓库测试。测试不得直接改动真实项目版本或发布状态。

### `pnpm run verify:pre-commit`

提交前快速验证，依次执行：

1. 项目版本一致性检查；
2. 构建与发布脚本测试；
3. FPK 进程身份校验 shell 测试；
4. Rust 全部测试，并将 warning 视为错误；
5. 前端类型检查；
6. 前端单元测试。

它不执行 Rust release build、Web 生产构建或双架构 FPK 构建。

### `pnpm run verify`

发布前完整验证。在快速验证基础上增加 Rust 编译和 Web UI 生产构建。该命令仍不代替 `build:fpk`、解包检查或 fnOS 实机验证。

两个验证命令结束时默认删除 `server/target/` 下的 incremental 缓存，以控制磁盘占用。临时需要保留时使用：

```bash
pnpm run verify:pre-commit --keep-rust-incremental
pnpm run verify --keep-rust-incremental
```

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

### `pnpm run clean:rust:incremental`

递归删除 `server/target/` 下名为 `incremental` 的目录，保留依赖和主要构建产物。适合只回收增量编译缓存空间。

### `pnpm run clean:rust`

删除整个 `server/target/`。下一次 Rust 测试、编译或 FPK 构建需要完整重建，耗时会明显增加。

## 版本命令

项目版本由以下四处共同表示：

- `package.json`；
- `server/Cargo.toml`；
- `packaging/fnos/manifest.template`；
- `packaging/fnos/app/ui/config` 中的缓存查询参数。

允许的项目版本格式是正式版 `x.y.z` 和本地测试版 `x.y.z-test.N`。GitHub 正式发布仍只接受 `x.y.z`。

### `pnpm run version:check`

读取并比较四个版本源。任一来源缺失、格式错误或与 `package.json` 不一致都会失败。该检查不修改文件。

### `pnpm run version:set <version>`

把四个版本源显式同步为指定版本，例如：

```bash
pnpm run version:set 1.7.4
pnpm run version:set 1.7.4-test.3
```

该命令不构建 FPK、不更新 CHANGELOG、不创建 commit/tag，也不自动恢复旧版本。Cargo 后续构建可能同步更新 `server/Cargo.lock`，提交前应检查全部版本改动。

### `pnpm run version:test`

自动生成下一个本地测试版本并同步四个版本源：

```text
1.7.3        -> 1.7.4-test.1
1.7.4-test.1 -> 1.7.4-test.2
1.7.4-test.2 -> 1.7.4-test.3
```

正式版必须先递增 patch 再进入 `test.1`，因为 SemVer 中 `1.7.3-test.1` 低于 `1.7.3`，不能可靠地作为已安装正式版的升级包。相同核心版本中，`1.7.4-test.2` 高于 `1.7.4-test.1`，但低于最终的 `1.7.4`。

推荐测试流程：

1. 确认工作区中的版本文件没有未预期改动；
2. 执行 `pnpm run version:test`；
3. 执行 `pnpm run version:check`；
4. 构建并安装对应架构 FPK；
5. 测试结束后按实际目标继续迭代，或将版本文件恢复为正式开发版本。

测试版本通常只用于本地安装验证，不应创建 GitHub Release 或正式 tag。

## 正式发布命令

### `pnpm run release:notes <x.y.z> [--body]`

从 `CHANGELOG.md` 提取指定版本，并在输出前执行严格结构校验。默认输出版本标题和正文；追加 `--body` 时只输出正文。该命令不修改 CHANGELOG 或 Git 状态。

### `pnpm run release:prepare <x.y.z>`

本地正式发布准备命令，默认会：

1. 校验目标是高于当前版本的正式 `x.y.z`；
2. 复用已有目标版本 CHANGELOG，或分块分析 Git 历史与最终净 Diff 后生成发布日志；
3. 同步版本文件并更新 CHANGELOG；
4. 运行完整 `pnpm run verify`；
5. 暂存固定的发布文件并创建中文 release commit；
6. 创建 `v<x.y.z>` tag。

常用的只读预演：

```bash
pnpm run release:prepare 1.7.4 --dry-run
```

可用参数还有 `--from <tag>`、`--no-verify`、`--no-commit` 和 `--no-tag`。后三个参数主要供受控自动化或故障排查使用，日常正式发布不应随意跳过验证、提交或 tag。

高影响注意事项：

- 命令会拒绝接管无关的脏工作区；执行前先提交、暂存到安全位置或恢复无关改动。
- 本地未配置 GitHub Models provider 时会根据 commit log 生成明确的确定性草稿；自动发布配置模型后会由 GPT-4.1 mini 按领域和 token 预算提取结构化事实，再由 GPT-4.1 编辑并独立审稿。本地校验会拒绝重复事实、纯测试、空泛描述和无法追溯的条目；任一模型调用失败、分块超限或日志格式非法都会阻止发布。
- 该命令只用于正式版本，不接受 `-test.N`。
- GitHub Actions 的 Release workflow 仍是远程正式发版入口；本地命令不能替代 Actions 权限、产物上传和双架构发布检查。

## Git hooks

### `pnpm run prepare`

通常由 `pnpm install` 自动调用，尝试执行：

```bash
git config core.hooksPath .githooks
```

为兼容非 Git 安装环境，该命令配置失败时不会让依赖安装失败。

### `pnpm run hooks:install`

显式安装本仓库 Git hooks，配置失败会返回非零退出码。当前 pre-commit hook 会运行 `verify:pre-commit`，因此普通提交可能需要一段时间。

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

主要检查目录：

```text
packaging/fnos/.stage/x86/
packaging/fnos/.stage/arm/
```

### `pnpm run build:fpk`

先清空 `packaging/fnos/dist/`，再完整构建 x86 与 ARM，最终保留两个 FPK：

```text
packaging/fnos/dist/motrix.fnos_<version>_x86.fpk
packaging/fnos/dist/motrix.fnos_<version>_arm.fpk
```

构建会清空源码 staging 区的 `packaging/fnos/app/data/`，以防 SQLite、日志或运行残留进入安装包。该目录只能存放占位内容，不得用于保存本地测试数据。

## 推荐工作流

### 普通代码提交

```bash
pnpm run verify:pre-commit
git status --short
```

### 本地升级测试包

```bash
pnpm run version:test
pnpm run version:check
pnpm run build:fpk
```

安装前确认包名中的版本和架构正确。真实设备上的数据备份、升级和回滚不由这些本地脚本自动完成。

### 正式发布前检查

```bash
pnpm run version:check
pnpm run verify
pnpm run build:fpk
```

随后按发布流程检查 CHANGELOG、双架构 SHA-256、解包内容和实机结果。不要把“本地构建通过”写成“fnOS 实机验证通过”。
