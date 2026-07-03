# Motrix FNOS 仓库清理审计（FPK-first）

> 生成日期：2026-07-03  
> 目的：基于当前仓库真实状态，判断哪些内容已经不应继续保留在 FPK-first 主线中，哪些文件应立即清理，哪些代码应在完成少量迁移后删除，哪些文档应归档或改写。

---

## 1. 审计结论摘要

当前仓库的主线已经明显转向：

- 前端主线是 `src/` 下的 **Vue Web UI**
- 后端主线是 `server/` 下的 **Rust server + Axum**
- 交付主线是 `packaging/fnos/` 下的 **FPK 打包目录**

但仓库里仍残留三类明显的“旧路线包袱”：

1. **完整的 legacy Tauri 桌面壳**  
   `src-tauri/`、Tauri CLI 依赖、Tauri dev/build 脚本、Tauri CI 检查仍在。

2. **已经不该留在仓库主视野中的历史/临时内容**  
   如脚手架图标、`.DS_Store`、临时文件、旧的本地产物目录。

3. **仍挂在 `src-tauri/` 路径上的“其实属于 FPK 主线”的资产**  
   主要是 Aria2 sidecar 二进制与相关脚本路径；这部分不是继续保留 Tauri 的理由，而是应该先搬家。

一句话结论：

> 这个仓库现在最该做的，不是继续“双轨长期共存”，而是把 **`src-tauri` 从“legacy 参照”收缩为“待下线遗留目录”**，然后分阶段移除。

---

## 2. 本次审计覆盖范围

本次审计已实际检查以下内容：

- 根目录文件：`README.md`、`package.json`、`vite.config.ts`、`.gitignore`、`index.html`
- 文档目录：`docs/*.md`
- CI 与脚本：`.github/workflows/verify.yml`、`scripts/*.mjs`
- 前端源码：`src/`
- 后端源码：`server/src/`
- legacy Tauri 源码：`src-tauri/`
- FPK 打包目录：`packaging/fnos/`

同时确认了几个关键事实：

- `src/` 内已经没有 `@tauri-apps`、`invoke(`、`listen(` 或其他 Tauri 直连引用
- `server/src/` 内仅剩 **1 处** 与 `src-tauri` 的直接耦合：调试回退到 `src-tauri/binaries`
- `src-tauri/Cargo.toml` 现在是 **依赖 `../server` 的 legacy 外壳**，而不是主线核心
- `src-tauri/src/tasks/mod.rs`、`config/aria2.rs`、`debug_logs/mod.rs`、`database/tasks.rs` 等文件已经只是 `pub use motrix_fnos_server::*`

这说明：

> 当前真正承载业务的不是 `src-tauri/`，而是 `server/` + `src/` + `packaging/fnos/`。

---

## 3. 关键证据

### 3.1 前端主线已经去 Tauri 化

- `src/` 中搜索不到任何 `@tauri-apps`、`invoke(`、`listen(`、`tauri`
- `App.vue`、`runtimeEvents.ts`、各 feature service 已走 HTTP / SSE
- `vite.config.ts` 已按浏览器 + server 代理模式工作

结论：

- `src/` 当前是可继续保留并演进的主线
- 不应再为 Tauri 兼容去污染前端结构

### 3.2 `server/` 已经是真正的后端主线

- `server/src/main.rs` 直接启动 `motrix_fnos_server::app::run_server()`
- `server/src/lib.rs` 已完整承载 `app/api/aria2/config/database/runtime/settings/tasks`
- `server/src/` 中唯一残留的 legacy 路径耦合，是 `server/src/runtime/aria2_process.rs` 的：

```rust
repo_root.join("src-tauri").join("binaries")
```

结论：

- `server/` 可继续保留
- 删除 `src-tauri/` 前，只需要先处理 sidecar 资产来源路径

### 3.3 `src-tauri/` 现在本质上是 legacy 桌面壳

证据：

- `src-tauri/Cargo.toml` 依赖 `motrix_fnos_server = { path = "../server" }`
- `src-tauri/src/lib.rs` 仍绑定：
  - `tauri::Builder`
  - 菜单 / 托盘 / 窗口事件
  - `tauri_plugin_*`
  - `invoke_handler`
- `src-tauri/src/commands/*.rs` 仍是 Tauri command 适配层
- `src-tauri/src/tasks/mod.rs` 等多个模块已只是对 `server` 的 re-export

结论：

- `src-tauri/` 不再是核心资产，只是一个桌面壳遗留层
- 它现在的存在价值主要是：
  1. 保留历史回归入口
  2. 暂时承载 sidecar 二进制目录

---

## 4. 建议清理原则

后续清理建议按下面原则执行：

1. **先清“确定无价值的垃圾”**
2. **再搬走仍有价值但放错位置的资产**
3. **最后删除整个 Tauri legacy 路线**

不要反过来做。  
如果直接删 `src-tauri/`，当前会同时打断：

- Aria2 sidecar 来源路径
- `scripts/fetch-aria2-next.mjs`
- `scripts/stage-aria2-sidecar.mjs`
- `server` 调试回退路径
- CI / verify 脚本
- README 中的 legacy 命令说明

---

## 5. 应立即删除的内容

这部分属于“现在删掉只会让仓库更干净，不会影响 FPK 主线”。

### 5.1 应立即从仓库删除的已跟踪文件

| 路径 | 建议 | 原因 |
| --- | --- | --- |
| `public/tauri.svg` | 删除 | Vite/Tauri 脚手架残留，当前主线未使用 |
| `public/vite.svg` | 删除 | Vite 脚手架残留，当前主线未使用 |
| `src/assets/vue.svg` | 删除 | Vue 脚手架残留，当前主线未使用 |

说明：

- `index.html` 当前只引用 `/icon.png`
- 未发现上述 SVG 资源在现有主线代码中被引用

### 5.2 应立即清理的本地/忽略产物

这部分多数已被 `.gitignore` 忽略，但仍建议在本地目录中清掉，避免误判或污染排查。

| 路径 | 建议 | 原因 |
| --- | --- | --- |
| `dist/` | 清理 | 前端本地构建产物 |
| `packaging/fnos/app/ui/dist/` | 清理本地产物 | FPK 组装阶段生成 |
| `packaging/fnos/ui/dist/` | 清理 | 旧路径残留，本地 stale 输出 |
| `packaging/fnos/app/bin/` | 清理本地产物 | server/aria2 staged 二进制 |
| `packaging/fnos/dist/` | 清理本地产物 | FPK 输出目录 |
| `packaging/fnos/motrix.fnos.fpk` | 清理本地产物 | 打包中间产物 |
| `src-tauri/src/aria2/mod.rs.tmp` | 删除 | 临时文件 |
| 根目录、`docs/`、`packaging/`、`src-tauri/` 下的 `.DS_Store` | 删除 | macOS 垃圾文件 |
| `src/stores/.gitkeep` 与空目录 `src/stores/` | 删除 | 当前 store 已迁入 feature 目录，无实际用途 |

特别说明：

- `packaging/fnos/ui/dist/` 当前既未被脚本使用，也不在当前 FPK 目录约定里
- 它很像阶段性调整后遗留下来的旧输出目录，应作为 stale 产物处理

---

## 6. 应在“小迁移完成后”删除的内容

这部分不是“不能删”，而是“先搬依赖，再删更稳”。

### 6.1 `src-tauri/binaries/` 不应继续挂在 legacy 目录下

当前仍引用 `src-tauri/binaries` 的地方：

- `scripts/fetch-aria2-next.mjs`
- `scripts/stage-aria2-sidecar.mjs`
- `server/src/runtime/aria2_process.rs`

建议动作：

1. 新建新的 sidecar 资产目录，例如：
   - `assets/aria2/`
   - 或 `packaging/assets/aria2/`
2. 把以下文件迁移过去：
   - `src-tauri/binaries/aria2-next-x86_64-unknown-linux-gnu`
   - `src-tauri/binaries/aria2-next-aarch64-unknown-linux-gnu`
   - `src-tauri/binaries/aria2-next-2.4.9-checksums.sha256`
3. 同步改这三个引用点：
   - `scripts/fetch-aria2-next.mjs`
   - `scripts/stage-aria2-sidecar.mjs`
   - `server/src/runtime/aria2_process.rs`

完成后可删除：

| 路径 | 建议 | 原因 |
| --- | --- | --- |
| `src-tauri/binaries/` 整体 | 删除/迁移后清空 | 不应再依附 legacy Tauri 目录 |
| `src-tauri/binaries/aria2-next-aarch64-apple-darwin` | 删除 | 飞牛 FPK 主线完全不需要 macOS sidecar |

### 6.2 整个 `src-tauri/` 应作为 legacy 主线下线目标

在 sidecar 路径迁移完成后，`src-tauri/` 基本就可以进入删除阶段。

建议删除对象：

#### A. 整个 Tauri 壳和入口

| 路径 | 建议 |
| --- | --- |
| `src-tauri/src/lib.rs` | 删除 |
| `src-tauri/src/main.rs` | 删除 |
| `src-tauri/src/commands/` | 删除 |
| `src-tauri/src/runtime/mod.rs` | 删除 |
| `src-tauri/src/app/mod.rs` | 删除 |
| `src-tauri/src/aria2/mod.rs` | 删除 |

原因：

- 全部是桌面壳 / Tauri 运行时 / command 适配层
- 不属于 FPK-first 主线

#### B. `src-tauri` 中仅用于 re-export `server` 的薄包装模块

| 路径 | 建议 |
| --- | --- |
| `src-tauri/src/tasks/mod.rs` | 删除 |
| `src-tauri/src/config/mod.rs` | 删除 |
| `src-tauri/src/config/aria2.rs` | 删除 |
| `src-tauri/src/database/settings.rs` | 删除 |
| `src-tauri/src/database/tasks.rs` | 删除 |
| `src-tauri/src/debug_logs/mod.rs` | 删除 |

原因：

- 它们已经不是主实现，只是 `pub use motrix_fnos_server::*`

#### C. Tauri 工程元数据与桌面打包资产

| 路径 | 建议 |
| --- | --- |
| `src-tauri/Cargo.toml` | 删除 |
| `src-tauri/Cargo.lock` | 删除 |
| `src-tauri/build.rs` | 删除 |
| `src-tauri/tauri.conf.json` | 删除 |
| `src-tauri/capabilities/` | 删除 |
| `src-tauri/icons/` | 删除 |
| `src-tauri/.gitignore` | 删除 |

原因：

- 全部服务于 Tauri 桌面构建
- 与 fnOS FPK 交付无关

---

## 7. 应同步删除或改写的脚本、依赖、CI

如果 `src-tauri/` 下线，下面这些内容也必须一起清掉，否则仓库会一直误导开发方向。

### 7.1 `package.json`

应删除：

| 项目 | 建议 |
| --- | --- |
| `tauri` script | 删除 |
| `tauri:dev` script | 删除 |
| `tauri:build` script | 删除 |
| `tauri:dev:raw` script | 删除 |
| `@tauri-apps/cli` devDependency | 删除 |

完成后：

- 重新生成 `pnpm-lock.yaml`

### 7.2 `scripts/`

| 路径 | 建议 | 原因 |
| --- | --- | --- |
| `scripts/tauri-dev.mjs` | 删除 | 完全是 legacy 桌面开发入口 |
| `scripts/verify.mjs` | 改写 | 不应再验证 `src-tauri/Cargo.toml` |
| `scripts/fetch-aria2-next.mjs` | 改写 | sidecar 源目录应从 `src-tauri/binaries` 迁出 |
| `scripts/stage-aria2-sidecar.mjs` | 改写 | sidecar 源目录应迁出 |

### 7.3 GitHub Actions

`.github/workflows/verify.yml` 当前仍有明显 Tauri 时代残留：

- `Install Linux dependencies for Tauri`
- `workspaces: src-tauri -> target`
- `pnpm run verify` 内仍构建 / 测试 `src-tauri`

建议改为：

1. 删除 Tauri Linux 依赖安装步骤
2. Rust cache 改为 `server -> target`
3. CI 验证重点切到：
   - `cargo test --manifest-path server/Cargo.toml`
   - `pnpm run typecheck`
   - `pnpm run build`
   - `pnpm run build:fpk:prepare`

---

## 8. 文档层面的待清理项

### 8.1 应改写但不应删除的文档

| 文档 | 建议 | 原因 |
| --- | --- | --- |
| `README.md` | 改写 | 仍保留 `src-tauri`/`tauri:dev`/legacy 测试说明 |
| `docs/development-plan.md` | 改写 | 当前大量“双轨保留”描述，后续应收敛成单主线 |
| `docs/api-contract.md` | 改写 | 仍写着“阶段 2/3 过渡期”“暂不删除 src-tauri” |
| `docs/fnos-manual-test-checklist.md` | 改写 | 仍写 `ui/dist/index.html`，应改成 `app/ui/dist/index.html` |

### 8.2 建议归档而不是继续作为活跃文档的内容

| 文档 | 建议 | 原因 |
| --- | --- | --- |
| `docs/fnos-fpk-remediation-plan.md` | 归档到 `docs/archive/` 或移出主文档入口 | 这是历史整改计划，且已出现旧路径和阶段性表述 |
| `docs/ui-stitch-prompts.md` | 迁到设计归档区或单独 `docs/design/` | 它不是当前工程主线文档，继续放在 docs 根目录会降低工程文档信噪比 |

### 8.3 可保留但建议后续合并的文档

| 文档 | 建议 | 原因 |
| --- | --- | --- |
| `docs/fnos-fpk-architecture.md` | 保留，后续视情况与 `docs/architecture.md` 合并 | 当前内容有价值，但与总架构文档存在边界重叠 |
| `docs/fpk-packaging.md` | 保留 | 仍是当前主线必需文档 |
| `docs/api-contract.md` | 保留 | 仍是当前主线必需文档 |

---

## 9. 不建议删除的内容

以下内容虽然看起来“旧”，但当前仍应保留：

| 路径 | 结论 | 原因 |
| --- | --- | --- |
| `server/` | 保留 | 已是后端主线 |
| `src/` | 保留 | 已是 Web UI 主线 |
| `packaging/fnos/manifest`、`cmd/`、`config/`、`wizard/` | 保留 | 已是 FPK 主线资产 |
| `public/icon.png` | 保留 | 当前 `index.html` 的 favicon |
| `src/components/EngineStatusPanel.vue` | 保留 | 当前仍被 `DiagnosticsDialog.vue` 使用 |
| `src/views/MainWindow.vue` | 暂保留 | 名字有桌面历史，但当前只是页面入口编排，不是必须立刻改名 |
| `packaging/fnos/app/ui/images/*.png` | 保留 | FPK UI 配置资源，不是脚手架垃圾 |

---

## 10. 推荐的实际清理顺序

建议按下面顺序执行，而不是一次性猛删：

### 第 1 步：先清确定垃圾

- 删除：
  - `public/tauri.svg`
  - `public/vite.svg`
  - `src/assets/vue.svg`
  - `src-tauri/src/aria2/mod.rs.tmp`
  - 各类 `.DS_Store`
  - 空的 `src/stores/`
- 清本地产物：
  - `dist/`
  - `packaging/fnos/ui/dist/`
  - `packaging/fnos/app/ui/dist/`
  - `packaging/fnos/app/bin/`
  - `packaging/fnos/dist/`

### 第 2 步：把 sidecar 资产从 `src-tauri` 搬走

- 新建独立 sidecar 目录
- 改：
  - `scripts/fetch-aria2-next.mjs`
  - `scripts/stage-aria2-sidecar.mjs`
  - `server/src/runtime/aria2_process.rs`

### 第 3 步：下线整个 Tauri legacy 工程

- 删除 `src-tauri/`
- 删除 `@tauri-apps/cli`
- 删除 tauri scripts
- 删除 `scripts/tauri-dev.mjs`
- 改写 `scripts/verify.mjs`
- 改写 GitHub Actions

### 第 4 步：统一文档口径

- README 去掉 legacy 命令入口
- development plan 去掉“双轨保留”叙述
- api-contract 去掉过渡期措辞
- manual checklist 修正旧路径
- remediation plan 归档

---

## 11. 最终建议

如果从“项目目标一致性”角度判断，当前仓库最应该做的并不是继续保留 Tauri 作为长期回归链路，而是：

1. **把仍有价值的 Linux/Aria2 资产从 `src-tauri` 迁出来**
2. **删除整个 Tauri 桌面壳**
3. **让仓库目录和文档都只表达一个事实：这是一个 fnOS FPK 应用，而不是跨端桌面应用**

否则会长期出现以下问题：

- 新人会误以为 `src-tauri` 仍是正式主线
- CI 继续为桌面壳付出构建成本
- 文档持续混杂“当前主线”和“历史迁移”
- sidecar、脚本、路径会继续被 legacy 目录绑架

本次审计的最终判断是：

> **应保留的主线只有 `server/`、`src/`、`packaging/fnos/`。**  
> **应逐步退场的是 `src-tauri/`、Tauri CLI、Tauri CI、脚手架残留资源和历史整改噪音。**

