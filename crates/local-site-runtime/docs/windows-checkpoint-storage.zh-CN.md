# Windows checkpoint 存储

## 范围与归属

Windows 私有会话存储传输不透明的 Workbench 文档。Server 验证领域含义并拥有捕获、恢复与保存调度。
本契约落实已接受的保留和恢复策略：保留三份有效存档，回退到有效副本，保留无法读取的原文件，
并在全部副本无效时明确提醒空恢复。它不授权在未知或仍存活的 Site owner 旁启动新会话。
Windows 崩溃启动准入仍属于本地 Site runtime。

Workbench payload 保持既有的有界版本化 JSON 格式。文件封装仅属于此存储 owner；
不向 Workbench 文档或公开 wire 协议增加字段。旧的普通 `checkpoint.json` 仍可读取。
旧可执行 cohort 不得打开升级后的 Site；Workbench JSON 保持版本 4 不代表带封装的多槽位布局向后兼容。
catalog 准入必须保留全部字节并拒绝旧 cohort，不能自动摊平或删除升级状态。回滚需要另行授权的离线转换，
或使用不同 Site；本切片不提供自动降级或活 owner 接管。

持久会话 catalog 版本是此私有布局的准入边界；它与 Workbench 文档版本、frame 版本彼此独立：

| Catalog 记录 | 进程身份与恢复                           | 已停止 Site 的 checkpoint 证据                                  |
| ------------ | ---------------------------------------- | --------------------------------------------------------------- |
| v1           | 没有进程登记；绝不推断 Active 记录已死亡 | 仅旧式普通 `checkpoint.json` 主文件                             |
| v2           | 确切 PID 与创建 FILETIME 登记            | 仅旧式普通 `checkpoint.json` 主文件                             |
| v3           | 相同的确切进程登记及当前冷恢复 guard     | 任一固定槽位或精确的自有损坏 raw 归档；全部内容仍由 Server 验证 |

读取 v1 或 v2 不会迁移。有效的旧版 Stopped 记录只有在普通主文件存在时才能开始新激活。
activator 在提交该激活标记、拉起 Server 或打开多槽位存储之前，先原子发布 catalog v3 的新 Active
记录；全新激活也先发布 v3。因此，仅准入 v1、v2 的旧 cohort 会在修改 catalog、runtime 标记或
checkpoint 目录之前拒绝升级后的 Site。Stopped v3 可以使用 backup 或 raw 证据，因为此前全部损坏的
恢复可能有意不留下有效槽位；内容验证及回退／空恢复决策仍归当前 Server。

## 状态与发布

既有私有目录及独占 `state.lock` 仍是唯一写者权限。三个固定槽位为 `checkpoint.json`、
`checkpoint.previous-1.json` 和 `checkpoint.previous-2.json`。文件名不表示新旧。
封装文件包含五字节 magic（`OBCK` 后接字节 `1`）、八字节小端非零 generation、
32 字节 SHA-256（此前 magic／generation 字节后接 payload），以及原样 payload。
payload 仍限 2 MiB，封装恰好增加 45 字节。
旧的未封装文件使用 generation 零。
每个新 frame 仅从完整准入的有效槽位中取最大 generation 再递增。digest 无效或领域 payload
无效时读到的 generation 只用于诊断，不能推进发布；没有有效槽位时从一重新排序。完整有效槽位的
generation 已为 `u64::MAX` 时仍视为耗尽并拒绝发布。

读取逐个通过私有普通文件边界准入槽位，限制字节数，验证封装和 digest，再请 Server 验证完整 payload。
文件缺失与无效相互区分。权限、身份、reparse 和文件系统错误导致准入失败，不能转成空工作区。
pending 字节不是已提交槽位。

选择 generation 最新的有效副本。同 generation 的不同 payload 判为无效；完全相同的重复副本
不会生成新 generation。存在无效副本时，结果携带恢复提醒。没有有效副本时，已有无效或已保留的
原文件要求明确的空恢复提醒；所有数据都不存在才表示新工作区。

在持续持有 state lock 时，发布遵循以下转换：

| 转换         | 条件                                    | 效果                                                                                                                                |
| ------------ | --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| 准备         | payload 已验证、有界，generation 可递增 | 优先选择缺失槽位，其次无效槽位，最后最早的有效槽位。写入并 flush 一个私有 pending 文件。                                            |
| 保留无效原件 | 选中的已有槽位无效                      | 把完整原文件移动到新建私有 `checkpoint.corrupt-<32 lowercase hex>.raw` 路径（占位部分为 32 位小写十六进制）；不覆盖或截断原始字节。 |
| 提交         | pending 完整；被替换的无效原件已保留    | 原子替换选中槽位；此替换是唯一发布点。                                                                                              |
| 失败或中断   | 尚未提交                                | 保留原有有效槽位；已移动的无效原件继续保留。下次打开存储时丢弃未提交 pending 字节。                                                 |

成功替换后最多保留三份有效槽位。替换最早有效槽位不能破坏此前最新的有效存档。
无法保留无效字节时拒绝替换。generation 耗尽时拒绝发布并保留数据。最新封装 payload 未变化时
无需再次发布；定时采样不能用相同副本消耗历史。

损坏原件属于用户恢复数据，不计入三份有效副本预算。它们留在该 Site 的私有状态目录。对于已停止的
命名 Site，`orchbee session delete <name> --confirm` 会随其他自有 checkpoint 文件一起删除其自有
损坏 raw 归档。default Site 没有删除命令；它的 raw 归档会一直保留，直到另行授权的修复移除。它们
不是过程日志，不能静默回收。删除仅识别固定存储文件与精确的自有损坏文件命名语法；未知文件、链接项
和仍存活 owner 继续沿用既有拒绝边界。

## 恢复提醒

Server 在 managed Client 端点 ready 前发布有界私有恢复提醒。提醒绑定到确切的持久 activation
generation，仅包含有限 outcome，不含 payload 字节或本地化文案。正常恢复清除当前提醒。
Client activation 通过既有私有 runtime owner 读取匹配的提醒。
提醒发布失败时端点不能 ready。`WindowsCheckpointRestoreNotice` 将读取、发布／失败、ready、
Client 连接及旧 generation 读取建模为独立事件。

无界面的 `site start` 在 stderr 显示本地化警告，保持既有成功文档和退出码。首次 TUI 通过既有
notice 界面显示相同提醒。同一 activation 的重连可再次显示；旧 generation 不能污染新启动。
这提供可见的恢复反馈，不读取终端屏幕，也不增加外部通知目的地。

## 证据及边界

存储模型从 v1、v2 或 v3 catalog 开始；其中 `legacyPrimary` 输入仅表示一个已准入的普通主文件，
backup 和 raw 文件名都不能满足该输入。`PublishCatalogV3` 映射为 activator 成功原子发布持久记录；
在该发布之前禁用多槽位准备和读取。`RefuseOldCohort` 只改变轨迹观测，保留 catalog 与存储状态。
runtime 标记保留不在此存储状态向量中，须由旧 cohort 原生无效果见证验证。

存储模型将抽象的单调 `publicationId` 与私有 frame generation 分开。`publicationId` 仅在模型中区分
损坏及归档前后的每个已提交原件，不写入文件；适配层把该身份映射到槽位中被固定的准确字节或新的 raw
归档名。frame generation 只对当前已准入有效槽位排序，包括全部有效排序证据丢失后从一重建。
模型检查三个槽位、无效原件保留、原子提交及回退；Server 自动保存模型检查最终停止顺序。
适配映射把同目录原子替换成功视作提交，仅在该结果之后主张进程崩溃恢复。确定性存储测试覆盖旧字节、
digest／格式／领域拒绝、与文件名无关的排序、generation 冲突、
原件保留失败、pending 中断、耗尽与不变发布。原生 Windows 测试覆盖独占 owner、私有路径、
替换拒绝和原始字节保留。

适配层在替换前 flush pending 文件，但不声称目录项 write-through。这些检查不证明存储硬件能抵抗断电，
且尚未执行真实机器重启见证。原生进程崩溃／重启验收及可见提醒证据仍须单独执行。Server 的
[Workbench checkpoint 契约](../../../apps/orchbee-server/docs/workbench-checkpoint.zh-CN.md)
拥有 payload、调度与确切物理目录 fence。
