# 一致的原生 CLI 语法

## 用途与状态

已授权 Todo 批次中 ORC-136 的交付决策。本记录说明选择及协同切换方式；已发布行为由
[命令界面规范](../../docs/zh-CN/cli/command-surface.md)及
[原生 CLI 参考](../../apps/orchbee/docs/native-cli-query.zh-CN.md)拥有。
保留本记录供以后语法变更了解设计理由，不把它作为第二套运行时权威。

## 决策

| 范围       | 唯一公开写法及含义                                                                                                                                                                                   |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 四向方向   | Panel 几何及放置使用 `--direction left                                                                                                                                                               | right       | up               | down`，在边界映射至既有线协议方向。                                                                       |
| 有序放置   | `tab move --order previous                                                                                                                                                                           | next        | first            | last`，与 `--insert-index` 互斥。                                                                         |
| 搜索方向   | `terminal copy-search --search-direction forward                                                                                                                                                     | backward`。 |
| 删除动词   | `close` 结束活动 workspace/tab/panel 生命周期；`delete` 销毁存储记录；`remove` 移除托管文件系统／机器成员关系；`uninstall` 移除已安装插件；`dissolve` 删除分组但保留子项。将这些副作用类别补入规范。 |
| Panel 目标 | Panel 命令族使用位置 `[panel]`，可用 `--current` 指定上下文选择；移除同义 `--panel`。其他命令族保留类型化 `--panel` 选择器。                                                                         |
| Agent 目标 | 保留精确 Session／名称选择；在 `--panel`／`@panel:` 之外增加显式 `--terminal`／`@terminal:`。终端目标可指向子终端，Panel 选择主终端；不回退其他成员。                                                |
| 名称       | 创建／重命名保留 `--label`，托管 Agent 身份使用 `--name`。新标签不是额外目标操作数。                                                                                                                 |
| Provider   | 统一使用 `--provider`，采用含 `antigravity` 的规范交互 provider 名称。集成能力是其支持子集；私有适配器 ID `antigravity-cli` 留在适配器边界。                                                         |
| 事件名称   | CLI 事件输入／输出使用 `panel.*`；在 CLI 边界映射既有私有 `pane.*` 线事件。公开事件信封升版，并保留旧 Schema 版本。                                                                                  |
| 输出       | 使用 `--source visible                                                                                                                                                                               | recent      | recent-unwrapped | detection`及`--lines`；移除读取／等待的 `--history`、`--rows`、`--unwrap`。几何 `--rows` 仍表示终端尺寸。 |
| 等待预算   | `terminal wait-output` 默认 30000 ms，接受有界 `--timeout`，只有显式 `--forever` 才无限等待。                                                                                                        |
| 按键       | 修饰符使用 `ctrl+`、`alt+`、`shift+`、`super+`，命名按键使用规范小写名称。移除连字符、control 和操作系统名称别名，保留可打印单字符输入。                                                             |

## 切换及不变量

实验性命令界面在一个协调版本中直接改变，废弃写法在访问运行时前失败。不引入隐藏别名、
自动重试或目的地推断变化。存储的运行时身份、Server 修改顺序和私有适配器／线 DTO 保留
既有契约；公开命名差异由边界的纯转换承担。

语法清单、补全、本地化帮助、Agent 指南、公开 Schema 和成对包文档同步修改。
解析及补全用例覆盖接纳／拒绝写法，投影用例覆盖精确 Panel／Terminal 路由及事件名称转换。
按用户要求，本批次逐项实现并提交，最后统一运行验证门禁。

<!-- documentation-pairing: english-sha256=1a0030560ea84724e182265ba4b387667681ca0a7e619e4df56e6a17cd975377 zh-cn-sha256=5f2fcbec87149f1d8ead5b057f1ee286af49f75d2a7de723b4eb6d26c954a2fc -->
