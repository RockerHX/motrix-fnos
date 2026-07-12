# Motrix fnOS UI 重设计与 Stitch/Figma 流程

> 当前阶段：设计输入已确认，Stitch MCP 已验证，下一步执行桌面暗色单页 PoC。
> 技术边界：继续使用 Vue 3、TypeScript、Naive UI 和 Pinia；不得以视觉稿改变 `docs/architecture.md` 规定的业务边界。

## 1. 文档职责

- `docs/design/ui-product-requirements.md`：功能、页面、状态和验收要求的唯一来源。
- `docs/design/DESIGN.md`：视觉 token 和设计规则的唯一来源。
- `docs/design/stitch-prompts.md`：可提交给 Stitch 的页面输入与迭代记录。
- 本文档：执行顺序、自动化门禁和 Stitch 产物 ID。

不得在本文档重复产品需求、视觉 token 或完整页面提示词。

## 2. 当前结论与门禁

Stitch 仍为 Beta。当前采用“一个 Design System、一个桌面母版、从母版派生页面”的流程。

1. 不执行旧的 P1-P10 独立自由生成流程。
2. 首轮只生成一个 `DESKTOP`、暗色、Downloading 正常运行页面；整体方向失败时最多生成第二个候选。
3. PoC 必须验证品牌绿、字体层级、侧栏、任务边界、按钮、圆角，以及二次编辑的一致性。
4. PoC 通过前不得生成移动端、浅色主题、弹窗或其他页面。
5. Figma frame 获得用户明确批准前，不得修改 UI 代码或安装运行时依赖。

## 3. Stitch MCP 单页 PoC

Codex 按顺序执行：

1. 查找私有项目 `Motrix fnOS UI Redesign`。不存在时创建；存在时先确认属于本流程。禁止改动其他项目。
2. 上传 `docs/design/DESIGN.md`，创建项目级 Design System，并确认项目能列出对应 asset。
3. 使用 `docs/design/stitch-prompts.md` 第 1 节生成一张 `DESKTOP` 页面；生成调用必须显式传入 Design System asset。
4. 通过 MCP 读取截图与 HTML，检查产品需求、视觉 token、可访问性和 Naive UI 可实现性。
5. 局部问题编辑同一 screen；整体方向失败时才生成第二个候选。不得继续其他页面。
6. 将 project ID、Design System asset ID、screen ID、模型和结论写入第 7 节。
7. 用户批准母版后，才按 `stitch-prompts.md` 第 2 节派生后续页面。

评审结论只能是：接受、需要调整、拒绝。需要调整时必须说明违反了哪一份事实源。

## 4. 来源与确定性

- Stitch 能力以 [Stitch Docs](https://stitch.withgoogle.com/docs) 和 MCP 实际返回为准。
- `google-labs-code/stitch-skills` 仅作参考实现；该仓库明确声明不是 Google 正式支持产品。
- 页面字段和操作来自 `ui-product-requirements.md`，视觉值来自 `DESIGN.md`。
- 未经官方正文、MCP 返回或当前界面验证的能力，不得写成确定事实。

## 5. 凭据与人工操作

Stitch MCP 使用 `STITCH_ACCESS_TOKEN` 作为 Bearer token，并发送 `X-Goog-User-Project: stitch-502207`。仓库、文档、截图和对话不得记录 token。

当 MCP 返回 `Auth required` 或 `Unauthenticated` 时，用户执行：

```zsh
NEW_TOKEN="$(gcloud auth application-default print-access-token)"
launchctl setenv STITCH_ACCESS_TOKEN "$NEW_TOKEN"
unset NEW_TOKEN
```

随后完全退出并重新打开 Codex。除刷新凭据和批准设计外，Stitch 项目操作由 Codex 通过 MCP 完成。

## 6. Figma 与代码门禁

母版获批后，用户将选定页面导入同一个 Figma 文件，并按 Desktop、Mobile、Components、Tokens 分页。重复元素应建立组件和 variant，关键 frame 标明尺寸，并补齐 hover、focus、disabled、loading、error、empty 和 selected 状态。

用户向 Codex 提供可查看链接和节点名称；无法授权时提供 2x PNG 与必要标注。只有用户明确批准的 frame 可以进入代码实现。批准后，Codex 先生成分阶段实现、测试和提交计划，再等待实施指令。

## 7. Stitch 执行记录

只在 MCP 调用成功后填写 ID，不得预填或猜测。

| 日期 | Project | Design System | Screen | Device / Theme / Model | 结论 | 下一步 |
| --- | --- | --- | --- | --- | --- | --- |
| - | 待创建 | 待创建 | 待生成 | `DESKTOP` / dark / 待记录 | 待评审 | 执行第 3 节 |

## 8. 新对话启动提示词

```text
请开始 Motrix fnOS UI 重设计的 Stitch MCP 单页 PoC。

开始前阅读 AGENTS.md、docs/architecture.md、docs/ui-redesign-stitch-figma-workflow.md，以及 docs/design/ 下的三份文档。它们已经用户确认，不得重新调查、重新生成或创建平行事实源。

我授权本次任务创建私有项目 `Motrix fnOS UI Redesign`、上传 docs/design/DESIGN.md、创建项目级 Design System，并生成一张 DESKTOP 暗色 Downloading PoC。先检查同名项目，禁止改动或删除其他项目。

严格执行 workflow 第 3 节。首轮只生成一个候选，通过 MCP 读取截图和 HTML并评审；整体方向失败时最多生成第二个候选。不得继续移动端、浅色主题、弹窗、其他页面、Figma 或 UI 代码。将实际 ID、模型和结论写入 workflow 第 7 节。

遇到认证失败时停止并提示我刷新 token；不得索取、输出或记录 token。
```
