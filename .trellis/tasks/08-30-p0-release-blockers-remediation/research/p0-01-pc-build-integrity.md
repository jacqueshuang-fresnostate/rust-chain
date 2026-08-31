# P0-01 PC 构建完整性研究

## 现状与证据

- `pc/postcss.config.js` 当前共 7,583 字节、8 行，文件 SHA-256 为
  `556812c8ec8177751aa22b8fa641a92e782f9e2564866887061c6626186bd5f0`。
- 前 7 行是常规 PostCSS 配置；第 8 行是一条约 7.4 KB 的混淆 IIFE。它位于构建配置文件中，
  只要 Vite/PostCSS 加载该配置，就处于构建进程权限边界内。
- 根 Docker workflow 会先安装依赖，随后才执行 `scripts/p0-release-gate.sh`；当前 release gate
  又从 Rust 格式检查开始，缺少在依赖安装、构建配置加载前执行的静态源码完整性门禁。

## 修复边界

1. 将 `pc/postcss.config.js` 收敛为纯声明配置，只保留 `tailwindcss` 与 `autoprefixer`。
2. 新增只依赖 Python 标准库的静态扫描器；扫描器只能读取文本，禁止 import/require/执行目标配置。
3. 扫描器至少阻断：
   - 已知恶意/异常样本哈希；
   - 构建配置中的超长单行可执行载荷；
   - 构建配置中动态求值与网络、子进程能力的组合；
   - 构建配置直接引入网络或子进程模块；
   - 所有 `package.json` 的自动 install lifecycle、受保护 release 事件的 pre/post hook、网络下载、内联动态求值与编码加载器。
4. GitHub workflow 必须在 checkout 后、Rust/Node setup 与任何依赖安装前运行扫描器。
5. `scripts/p0-release-gate.sh` 首步也必须运行扫描器，保证本地与 CI 使用同一入口。
6. 为扫描器补正例、已知哈希、超长行、动态求值/网络、子进程等回归测试。
7. 增加事故处置说明，覆盖凭据轮换、Runner/缓存重建、历史调查和恢复验收。
8. 固定 `pc/package.json` 的真实生产构建入口为 `vite build`，并在本地 P0 gate 的 PC type-check/test 后实际执行该 build。

## 验证计划

- `python3 scripts/source_integrity_gate.py`
- `python3 -m unittest tests.test_source_integrity_gate`
- `npm --prefix pc run type-check`
- `npm --prefix pc test -- --run`
- `npm --prefix pc run build`
- 检查 workflow 中门禁顺序早于任何 setup/install/build 步骤。

## 所有权建议

- 实现代理独占：`pc/postcss.config.js`、源码完整性脚本/测试、Docker workflow、release gate、
  `docs/security/` 下事故说明。
- 不修改业务后端、迁移、公共进度文件与 Trellis 任务文件。
