# 原生主题

## 选择与归属

Windows 配置所有者接纳 `[theme] name = "terminal-native"`、`"one-light"` 或
`"one-dark"`。默认值为 `terminal-native`。名称要求精确匹配；非法名称或类型产生警告，
仅该字段回默认。其他主题字段，包括 `custom` 和自动明暗选择，仍未支持。
离线默认配置包含默认主题名称。

主题作用于 Orchbee 界面、侧栏、菜单、提示和选区覆盖层，不修改程序的终端单元格、
调色板、焦点、尺寸或控制所有者。主题模块映射已有语义角色；渲染器消费角色。
两套 RGB 主题声明其明暗方向并绘制界面背景。

## 色彩能力

Shell 在绘制前采样一次色彩环境输入。纯解析器依次采用：非空 `NO_COLOR`、
`FORCE_COLOR`、`COLORTERM`、`TERM`。`NO_COLOR` 和 `FORCE_COLOR=0` 关闭界面颜色。
Force 值 `1`、`2`、`3` 分别选择 16 色、256 色和真彩色。非法 force 值为未知，
采用 16 色。`COLORTERM=truecolor` 或 `24bit` 选择真彩色；包含 `256color` 的 TERM
选择 256 色，`TERM=dumb` 关闭颜色。
当 `COLORTERM` 和 `TERM` 缺失或为空时，符合普通 GUID 格式的 `WT_SESSION`
标识 Windows Terminal 并选择真彩色。存在 `TMUX`、`ZELLIJ` 或 `STY` 时，
不采用这个外层宿主回退。显式描述符（包括未知值）保留原有决定；格式错误的会话
标识仍视为未知并采用 16 色。不记录变量值。这是色彩能力决定，不推断终端明暗。

Windows Terminal 在拉起连接时设置[普通会话 GUID](https://github.com/microsoft/terminal/blob/main/src/cascadia/TerminalConnection/ConptyConnection.cpp)。
没有该标识的宿主仍采用保守回退；不增加终端查询或逐帧环境读取。

接纳主题时在渲染之外完成主题角色量化，使用现有规范 xterm 调色板和感知距离。
16 色下，文字角色排除亮黑，并通过修饰保留次要／禁用层级。无色模式保留文字、
字形和修饰。Host 调色板可能不同；这不构成物理对比度测量或终端明暗协商。
低色级下，宿主可能把 DIM 黑色画得与普通黑色一样。即使禁用修饰已保留，
仍必须显示不可用原因。

## 发布与重载

Server 在已有完整配置快照中发布主题名称。只改主题也会推进发布 revision。
Client 先完整验证快照，再应用任何字段，仅接纳更新的 revision，包括初始权威基线。
致命重载错误、非法快照或过期发布保留已安装主题。启动沿已有流程，可在权威基线
抵达前绘制本地临时配置。

`orchbee site reload-config`、已有重载快捷键和全局菜单重载同一所有者。
下一完整帧的全部界面使用同一缓存主题；渲染不读取配置文件或环境。
后台任务继续运行，不重放用户输入。配置触发的菜单取消沿用已有行为。

封闭 wire 快照要求主题名称，Client 和 Server 必须来自同一原生 cohort。
新 CLI 输出版本保留全部旧的封闭回执版本。回退整个 cohort 并选择 `terminal-native`；
不迁移或删除检查点、终端历史或配置文件。

## 正确性与证据

| 声明                             | 证据                                                | 边界                                                    |
| -------------------------------- | --------------------------------------------------- | ------------------------------------------------------- |
| 精确名称与字段局部回退           | 配置决策向量、CLI 警告与 schema 检查                | Windows TOML 所有者及已有致命来源边界                   |
| 完整角色映射与能力优先级         | 纯调色板／解析器向量、量化 buffer、全部消费界面渲染 | 三套主题、规范调色板、采样环境；排除程序颜色            |
| 原子重载及过期／致命输入保留主题 | 配置发布模型、Server／Client 轨迹及共享合约语料     | 已有有限 payload／revision 模型；不新增公平性或活性声明 |
| 启动及共享重载抵达真实 Client    | 隔离 Windows ConPTY 与根验证                        | Windows 原生进程／字节证据；物理模拟器外观另验          |

运行 `node scripts/tla/run-models.mjs --models windows-configuration-publication`
及必需的根 `pnpm verify`。自定义颜色、自动明暗报告及其余 UI 布局选项属于独立后片。
