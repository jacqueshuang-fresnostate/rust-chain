# PC 构建供应链事件响应 Runbook

## 1. 适用范围与已知 IOC

当 PC 的 PostCSS、Vite、Tailwind 或其他可执行构建配置出现未知顶层副作用、混淆长行、动态求值、网络访问或子进程能力时，立即按本 Runbook 处置。

当前已知异常文件 IOC：

```text
路径：pc/postcss.config.js
SHA-256：556812c8ec8177751aa22b8fa641a92e782f9e2564866887061c6626186bd5f0
```

处置期间只进行静态读取与哈希比对，不加载、导入、解码或执行可疑配置。代码仓库中的修复与门禁通过不代表主机、凭据或历史制品已经恢复可信。

## 2. 发现后 0–1 小时：冻结与保全

1. 立即冻结 PC 开发、预览、打包、发布以及所有会加载前端构建配置的任务。
2. 暂停受影响的自托管 Runner；阻止其领取新任务。GitHub 托管 Runner 的相关工作流也先禁用或保持发布环境审批锁定。
3. 隔离疑似执行过该配置的开发机和 Runner，但不要先清理进程、缓存、日志或工作目录。
4. 记录发现时间、仓库提交、分支/标签、工作流运行 ID、Runner ID、操作者、文件大小及 SHA-256。
5. 保存只读证据：工作目录快照、进程树、网络连接、DNS/代理/防火墙日志、Actions 日志、系统审计日志、包管理器日志、缓存和制品元数据。所有导出物再次计算 SHA-256，并记录保管人和时间。
6. 撤销尚未完成的部署审批，暂停消费事发时间窗内产生的 PC 制品、容器镜像、Actions artifact 和缓存。

## 3. 凭据轮换

先撤销旧凭据，再签发替代凭据；轮换动作必须记录负责人、完成时间、旧凭据失效证据和新凭据生效验证。按可疑构建进程可能读取到的权限边界处理，而不是只轮换源码中直接出现的值。

1. GitHub：撤销 PAT、部署密钥、SSH/GPG 签名密钥、GitHub App/Runner 注册凭据和长期令牌；检查异常 OAuth/App 授权及仓库、环境和组织级 Secret 访问记录。
2. 制品与依赖：轮换 GHCR、npm/私有 registry、对象存储、签名和发布凭据，撤销事发窗口内签发的临时令牌。
3. 基础设施：轮换云平台、DNS/CDN、数据库、MongoDB、Redis、RabbitMQ、邮件、监控和告警凭据。
4. 应用密钥：轮换 `JWT_SECRET` 并使既有会话失效；轮换其他可能暴露的 API Secret。`CREDENTIAL_ENCRYPTION_KEY` 涉及既有密文时，必须先备份并执行受审查的解密—重加密迁移，不得直接替换后造成数据永久不可读。
5. 开发者身份：对在受影响主机上使用过的 SSH Agent、云 CLI、浏览器会话和密码管理器会话执行撤销或重认证。

## 4. Runner、缓存与制品重建

1. 自托管 Runner 不做原地“清理后继续使用”；从可信只读镜像重新置备，更新系统和工具链，重新注册并使用新的最小权限凭据。保留旧磁盘快照用于取证。
2. 删除事发时间窗覆盖的 GitHub Actions cache、artifact、Docker BuildKit cache、依赖代理缓存和预览部署；记录被删除对象的 ID、键、摘要和时间范围。
3. 撤回或隔离该时间窗内产生的 GHCR 镜像、PC 安装包、静态站点和签名制品。不要通过重新打标签把旧层带入恢复版本。
4. 开发机恢复时从可信镜像重装；删除并重新生成 `node_modules`、包管理器缓存、`pc/dist`、Vite/PostCSS 缓存和原生打包中间产物。
5. 恢复构建必须使用新的 clean clone、受信提交和锁文件安装；禁止复用事件前后的工作目录或缓存层。

## 5. Git 历史与 IOC 审计

### 5.1 源码历史

在隔离分析环境中执行静态命令，定位首次引入、传播和删除节点：

```bash
git log --all --date=iso-strict --format='%H %ad %an %ae %G?' -- pc/postcss.config.js
git log --all -p -- pc/postcss.config.js
git branch --all --contains <INTRODUCING_COMMIT>
git tag --contains <INTRODUCING_COMMIT>
```

逐提交读取该路径并计算哈希，不 checkout、不加载配置：

```bash
git rev-list --all -- pc/postcss.config.js | while read -r commit; do
  digest="$(git show "${commit}:pc/postcss.config.js" 2>/dev/null | shasum -a 256 | awk '{print $1}')"
  printf '%s %s\n' "$commit" "$digest"
done
```

检查引入提交的父提交、PR、review、签名状态、作者身份、强制推送记录及所有包含该提交的分支和标签。审计其他可执行构建配置是否出现同类长行、网络/子进程能力或动态求值组合；不要复制或解码可疑载荷。

### 5.2 主机与网络

1. 以首次可能执行时间至隔离时间为窗口，关联 `node`、`npm`、`vite`、`postcss` 及其子进程树。
2. 查询 DNS、代理、EDR、主机防火墙和云审计日志中的未知目的地址、下载、持久化、凭据读取和横向访问。
3. 将工作流运行、Runner、提交、cache key、artifact ID、镜像 digest 和部署记录建立时间线。
4. 检查仓库 Secret、环境审批、分支保护、Actions workflow、依赖锁文件和发布标签是否有未授权变更。
5. 将确认的域名、IP、文件哈希、进程命令行和账号行为加入 IOC 清单；IOC 清单只能扩充，不用未经证实的推测覆盖原始证据。

## 6. 静态门禁范围与例外策略

`scripts/source_integrity_gate.py` 只用 Python 标准库，以 UTF-8 文本静态检查以下构建信任边界：

- `*.config.js|cjs|mjs|ts|cts|mts`；
- `.babelrc.*`、`.eslintrc.*`、`.postcssrc.*`、`.prettierrc.*`；
- `gulpfile.*`、`gruntfile.*`；
- 强制存在的 `pc/postcss.config.js`。

门禁跳过 `.git`、`node_modules`、`dist`、`target`、`vendor` 等版本控制元数据、依赖树和生成目录，避免把第三方或构建输出误当作仓库源码。这里是路径边界，不是内容白名单。已知哈希、长行、网络、子进程、动态求值和编码加载器规则均没有文件级或内容级豁免。

若合法构建需求确实需要网络或子进程，应把动作迁移到独立、显式、最小权限且可审计的 CI 步骤，并在依赖安装后运行；不得通过给可执行构建配置添加例外来绕过门禁。

## 7. 可信恢复流程

在新置备且凭据已轮换的环境中，从受信提交建立 clean clone。任何依赖安装或构建前先执行：

```bash
python3 scripts/source_integrity_gate.py
python3 -B -m unittest tests.test_source_integrity_gate
```

两项均成功后，才允许安装并验证 PC：

```bash
npm --prefix pc ci
npm --prefix pc run type-check
npm --prefix pc run test:margin
npm --prefix pc run build
```

保存命令输出、锁文件哈希、Runner 镜像版本、环境审批、产物 SHA-256 和发布签名。恢复制品必须使用全新版本号或不可变 digest，并与事发时间窗内的全部制品区分。

## 8. 恢复准入标准

只有全部满足后，安全负责人和发布负责人才能共同解除冻结：

- `pc/postcss.config.js` 为仅含 Tailwind/Autoprefixer 的声明式配置，已知 IOC 哈希在当前源码中不存在。
- clean clone 在任何 setup、install 或 build 前通过扫描器及其恶意 fixture 回归测试。
- 已确定引入提交、影响分支/标签、可能执行的主机/Runner、缓存、制品和部署范围；历史与 IOC 时间线无未解释缺口。
- 所有可能暴露的凭据已撤销并轮换，旧会话已失效，轮换证据已归档。
- Runner 已重建，受影响缓存、artifact、镜像和预览部署已失效；恢复构建没有复用旧层。
- PC 类型检查、测试和生产构建在 clean 环境通过，新制品摘要、签名和来源证明已留存。
- 已完成最少一个发布观察窗口，未发现异常网络、子进程、身份或制品行为，并已演练回滚入口。

任一标准不满足时继续保持发布冻结，不得用“当前构建成功”替代主机取证、凭据轮换或历史影响确认。

## 9. 回滚

恢复版本出现异常时，立即重新冻结发布、撤回新制品和会话、隔离新 Runner，并恢复到事件前已独立验证的不可变制品 digest。回滚不得恢复可疑源码、Runner 磁盘、cache 或构建层；保留失败恢复环境的证据并重新进入第 2 节流程。
