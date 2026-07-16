# AGENTS.md

## 项目执行规则

- 用户输出使用中文；工具交互使用英文。
- 只做用户明确要求范围内的改动，禁止影响无关功能。
- 修改前先读取相关文件，优先编辑已有文件。
- 非 trivial 任务必须先建立可追踪任务；跨文件或多阶段任务优先计划模式。
- 宣称完成前必须执行最贴近改动的验证；未验证必须明确说明。

## 进度记录规则

每完成一个可交付任务切片，必须更新：

`docs/superpowers/PROGRESS.md`

记录格式：

```markdown
## YYYY-MM-DD HH:mm - <任务标题>

- 完成内容：<本次完成了什么功能或文档变更>
- 修改文件：<路径列表>
- 验证结果：<执行过的验证命令和结果；未执行则写明原因>
- 后续事项：<仍需继续的任务；没有则写“无”>
```

后续会话开始执行任务前，必须先读取本文件和 `docs/superpowers/PROGRESS.md`，确认历史进度与执行规则。
<!-- TRELLIS:START -->
# Trellis Instructions

These instructions are for AI assistants working in this project.

This project is managed by Trellis. The working knowledge you need lives under `.trellis/`:

- `.trellis/workflow.md` — development phases, when to create tasks, skill routing
- `.trellis/spec/` — package- and layer-scoped coding guidelines (read before writing code in a given layer)
- `.trellis/workspace/` — per-developer journals and session traces
- `.trellis/tasks/` — active and archived tasks (PRDs, research, jsonl context)

If a Trellis command is available on your platform (e.g. `/trellis:finish-work`, `/trellis:continue`), prefer it over manual steps. Not every platform exposes every command.

If you're using Codex or another agent-capable tool, additional project-scoped helpers may live in:
- `.agents/skills/` — reusable Trellis skills
- `.codex/agents/` — optional custom subagents

Managed by Trellis. Edits outside this block are preserved; edits inside may be overwritten by a future `trellis update`.

<!-- TRELLIS:END -->
