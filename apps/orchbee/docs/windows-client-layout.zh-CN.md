# Windows Client Panel 几何

本参考文档负责原生 Windows CLI 对活动 TUI Client 的 Panel 布局检查。TUI 持有视口、界面占用、焦点与缩放状态。Server 路由请求并保留有界观测；成员关系仍以规范 Workbench 拓扑为准。布局读取绝不选择上下文或 Tab、附着终端、改变缩放或调整终端尺寸。

## 观测

查询指定精确的 Client、TerminalContext、Tab、源 Panel 及最低 Workbench revision。现有已认证 Client 通道在把请求交给 UI 之前，进行一次可取消的规范 Snapshot 读取。Snapshot 不可用或目标缺失时明确失败。UI 使用该 Snapshot 投影目标，不把它安装为活动上下文；低于请求最低 revision 的 Snapshot 会被拒绝。对于选中 Tab，几何使用当前渲染成员缓存及其 revision；缓存尚未达到请求最低 revision 时明确失败，不通过刷新成员关系引发终端 resize 副作用。非活动目标使用取得的 Snapshot，明确作为未选中的投影。

结果包含目标与规范 revision、采样时的单元格视口与终端区域、目标 Tab 是否选中、本地缩放标志，以及必填但可为 null 的焦点 Panel。未访问及非活动上下文均可读取。非活动目标使用 Client 仍存在的记忆 Panel，缺失时回退到首个成员，遵循[选择记忆](native-context-selection.zh-CN.md#client-选择记忆)规则。此局部焦点不代表 Tab 被选中，`selected` 标记保留这一区别。非活动 Tab 的几何是它在采样视口中的投影，不表示正在绘制。

每个规范 Panel 都包含必填但可为 null 的未缩放外框、未缩放终端内容及显示终端内容矩形。null 表示渲染器没有在该层放置此 Panel。缩放保留未缩放几何，仅让焦点 Panel 显示完整内容区域。Minimal 视口反映渲染器的单 Panel 回退；过小视口不编造几何。分割记录稳定的 split/slot 身份、方向、渲染比例，以及必填但可为 null 的区域／分隔线矩形。保持规范遍历顺序。

全部矩形位置与尺寸均为整数单元格。界面占用、边框线、共享分隔线、最小尺寸折叠与奇数单元格取整，都来自绘制和终端尺寸同步使用的同一投影。终端像素尺寸不影响单元格邻接。

## 邻居与边缘

导航使用未缩放的外框叶矩形，不使用扣除标题后的内容矩形，也不使用归一化拓扑坐标。候选必须完全位于指定方向，并在垂直于该方向的轴上有正重叠。依次选择边缘距离最小、重叠最大、二倍中心距离最小、规范遍历顺序最前者。坐标相加前扩大整数类型。源几何为空或隐藏时明确标记不可用。

观测包含源 Panel 的四个可为 null 的邻居。方向边缘通过比较可见源的未缩放矩形与终端区域边界确定，与参考实现的物理边缘处理一致。无邻居与物理边缘是不同事实。隐藏源不作边缘判断。缩放不会抹掉有效的未缩放邻居。采用的导航排序固定参考 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/layout.rs::find_in_direction`；原生界面占用及降级仍遵循 Orchbee 契约。Orchbee 在整个允许坐标范围内扩大二倍中心计算类型；不采用参考实现对极大坐标的 u16 中心饱和行为。

## 路由、生命周期与边界

布局是现有 Client 控制通道上的读取意图，具有独立的 layout-read 完成结果及保留状态。它继承 [Client 视图控制](windows-client-control.zh-CN.md) 的精确 Client／调用者选择、操作身份、新鲜度、单次投递、排队过期、断连结算及调用者超时／取消行为。读取结果不能确认导航或缩放命令，普通 applied 成功也不能确认布局读取。

UI 在验证目标 Snapshot 后，于一个 tick 内捕获观测。结果描述该时刻；后续尺寸、输入或拓扑变化不会重写回执。不声称非活动 Tab 已成功绘制。观测前发生本地中断会拒绝查询。Snapshot I/O 在 UI 线程外执行，具有自有截止时间及可唤醒退出，不自动重试。

专用 OBWP 帧类型 27/28 承载布局完成／结果，在分配前限制为 1 MiB；普通 Client 帧类型 25/26 保留分配前 16 KiB／64 KiB 限制；现有普通请求及小结果仍按其 action 执行原有边界。Panel 与分割条目之和不得超过规范上下文的 4096 节点限制；身份唯一，所有焦点、源及邻居引用必须存在。矩形保持在采样视口与相应内容区域内。

Server 在查询入队前，从 16 MiB 编码布局预算中预留 1 MiB。完成后用有界编码观测的实际大小替换预留。拒绝、无结果断连及回执过期释放预留。现有总计 256 回执及完成后 60 秒保留期仍有效。容量拒绝发生在投递之前；绝不驱逐待处理或未过期结果来腾出空间。

## CLI 与证据

`panel layout`、`panel neighbor --direction left|right|up|down` 和 `panel edges` 接受位置参数 Panel，或互斥的 `--panel`／`--current`，几何查询使用现有 Client 选择规则。显式 `panel layout <panel>` 且省略 `--client` 时，
改为无头读取规范拓扑及比例（`orchbee.panel.structure` v1，`scope:shared`），不声明视口、焦点或缩放。
添加 `--client` 才请求几何。三个只读命令都拒绝 `--request-id` 和 `--detach`。连接或副作用前完成解析。机器输出保留完整布局观测，以及必填但可为 null 的 `projection`：邻居结果含方向、源可用性及可为 null 的邻居 ID；边缘结果含源可用性及四个可为 null 的物理边缘布尔值。在收到布局结果前，projection 为 null；源隐藏时边缘布尔值为 null。文本输出呈现所请求的视图，不隐瞒不可用几何。

证据覆盖精确的界面占用／取整／最小尺寸几何、嵌套与退化树、独立邻居排序向量、带缩减重放的生成层级不变量、非活动及未访问目标、无选择／附着／resize 副作用、双 Client 隔离、过期成员关系、Snapshot 失败、大小边界、容量、中断及退出。Rust、TypeScript 与 JSON Schema 共享一致性语料，但独立验证。受控 Windows TUI 轨迹把契约连接到真实显示。共享比例的提交／发布／绘制检查使用下文实现证据；不再保留独立的布局流程 TLA 模型。同作者检查不等于独立复核。

方向聚焦在导航前使用此投影，见 [Client 视图控制](windows-client-control.zh-CN.md#panel-方向聚焦)。比例修改遵循下述契约；交换及布局导入／应用仍是独立能力。扩展仅用于精确 cohort 准入；共享比例使用升级后的持久准入边界；视图／控制观测与回执仍为易失状态。

## 分割比例调整

`panel resize` 调整精确或当前源 Panel 周围最近的可用分割。必须提供四向方向；有限 amount 默认为 `0.05`，取绝对值并限制为至多 `0.5`。`layout set-split-ratio` 接受互斥的 `--tab`、`--panel` 或 `--current` 目标、二元分割路径（`root` 或点分隔的 `0`/`1`，至多 16 步），以及限制到 `0.1`–`0.9` 的有限比例。`panel resize` 使用现有精确 Client 路由、操作身份及截止时间。
显式 `layout set-split-ratio --tab/--panel` 且省略 `--client` 时直接使用 Server CAS，
返回共享范围的 `orchbee.layout.ratio` v1，保留请求及结果。无需 Client，并保留未知修改结果；
`--request-id` 可选，拒绝 `--detach`。隐式／当前目标或显式 `--client` 仍使用 Client 路由及绘制确认。连接或副作用前完成解析。路径缺失及 resize 源隐藏均明确失败。设置比例可以指向隐藏的分割，不编造焦点。

Server 在规范 Workbench 树中持有每个分割的标称比例。所有 Client 在各自视口中投影相同比例；焦点、zoom 和终端控制仍由各窗口管理。新分割默认为 `0.5`，保留的子树继续保留比例。原子修改及当前版本 1 checkpoint／catalog 准入见[共享比例契约](../../orchbee-server/docs/shared-split-ratios.zh-CN.md)。

绘制、终端尺寸、指针区域和布局读取消费同一加权树投影。每个分割用轴长减去共享分隔单元格，按名义比例计算一次，第一部分取最近整数单元格（半数向上），并限制到两个子区域都满足约定最小尺寸。无法同时满足时沿用活动／首叶折叠。即使最小尺寸或整数取整限制了单元格结果，名义比例仍可观测。边线标题、zoom 和界面占用保留原所有者。

Resize 使用可见的未缩放源单元格。可选分隔线轴向相同，在垂直轴与源有正重叠，并位于请求边缘的一单元格以内。距离最近者优先，同距按规范遍历顺序；没有则检查对侧边缘。右／下增加 amount，左／上减少，再限制到支持的比例范围。没有可用分割明确返回无变化。该策略采用 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/layout.rs::resize_focused`；原生界面占用、最小尺寸及扩宽算术仍遵循 Orchbee。原生使用有限 f64，参考实现使用 f32。参考对比覆盖原生几何中的边界选择，不声称参考渲染或持久化完全相同。

UI 根据已绘制缓存及准入的完整 Snapshot 一次性解析调整。显式路径和 resize 边缘必须对应相同 split、有序 slot 及原比例；可选 `expectedSplit` 还为创建流程钉住这三个身份。worker 仅提交一次带确切修订及原比例的规范 `layout.set-split-ratio` 命令。Server 拒绝会明确返回，Client 与 CLI 均不自动重试；不乐观写入本地比例。

`splitRevision` 现在报告 Client 投影缓存的完整宽度规范 Workbench 修订。它随共享发布推进，也包括其他元数据变化；它不是独立比例计数器，也不证明非活动目标已经绘制。保留回执保持其捕获时的观测。

| 条件                                     | 提交与确认                                                             |
| ---------------------------------------- | ---------------------------------------------------------------------- |
| 没有可用 resize 分割或比例未变化         | `layout-adjusted`，不写规范状态、不 resize 终端或请求绘制              |
| 改变非活动或未访问 Tab                   | 等待 Server 提交后确认，不选择、附着或绘制该 Tab                       |
| 改变已选中 Tab                           | 等待提交与共享发布，重新观察几何、失效指针区域，成功完成兼容绘制后确认 |
| 校验、容量、过期修订／原比例或身份被替换 | 拒绝，保留当前状态；需要新的显式操作                                   |
| 提交结果未知，或提交后中断、过期、失败   | Incomplete，不声称回滚或自动重放                                       |

既有 frame 25/26 保留有界 `layout-adjusted` 回执，携带确切目标、修订、selected 状态，以及 `no-split` 或有身份的 `split` 变化。只有 resize 可返回 `no-split`。setter 报告归一化请求比例，resize 报告钳制后的有符号增量。selected 成功要求原选择／zoom 保持，成功绘制的缓存修订不早于提交，且仍包含已提交比例；回执捕获实际绘制修订。非活动目标回执捕获提交修订，不推进无关的选中视图。成功绘制不声称 Host resize 已完成。

确定性及生成 Server 轨迹覆盖边界、嵌套层级、CAS、确切身份、重放、无变化及非目标字段保持，并支持删除缩减和序列化重放。原生控制器及 worker 轨迹覆盖单次提交、非活动纯净性、发布／绘制顺序、拒绝、未知结果及退出。独立 Rust、TypeScript、JSON Schema 校验及正常 Windows ConPTY 轨迹将共享比例连接到真实双 Client 几何和控制者尺寸。真人 Windows Terminal 和实际重启证据仍独立验收。

## 验证映射

| 声明                                               | 维护中的证据                                                                                                                                                                                                                                         |
| -------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 几何、焦点保留、嵌套拓扑与折叠                     | `src/tui_layout/tests.rs`、`src/tui_layout/tests/generated.rs`：八个固定种子，每种子 196 个动作，JSON 重放与删除缩减                                                                                                                                 |
| 选中／非活动／未访问目标的只读行为及 Snapshot 中断 | `src/terminal_session/ui/client_layout/tests.rs`、`src/terminal_session/client_control/tests.rs`、`src/terminal_session/client_control/lane/tests.rs`                                                                                                |
| 精确身份、边界、物理边缘及邻居排序                 | `crates/workbench-protocol/tests/client_layout.rs`、共享 Client 视图语料及独立 TypeScript 校验                                                                                                                                                       |
| 预留、完成、断连与过期                             | [Server 注册表测试](../../orchbee-server/src/client_view_registry/tests.rs)及[生成交错](../../orchbee-server/src/client_view_registry/tests/traces.rs)：96 个固定种子，每种子六个固定前缀动作及 128 个生成动作，重放及删除缩减；有限轨迹，不穷尽调度 |

以 `src/` 开头的路径相对于本应用，其余路径相对于仓库。运行 `cargo test --locked -p orchbee --bin orchbee`、`cargo test --locked -p orchbee-workbench-protocol --test integration client_layout::`、`cargo test --locked -p orchbee-workbench-protocol --test integration client_view::` 及 `cargo test --locked -p orchbee-server client_view_registry`。仓库门禁负责进程及绑定检查。参考捕获维护使用八个确定性种子，在边界至多为 1,200 单元格的非空矩形上，对比未改动的固定 Herdr 邻居函数与协议投影器，共 65,536 次方向查询；不覆盖参考实现的中心饱和域。维护中的协议向量独立于外部 checkout 保留决策边界。

终端尺寸所有者在未缩放时按精确附着实例放行每个可见且持有控制权的 pane。缩放时仅焦点 pane 调整尺寸；隐藏同级保留查看器和原物理尺寸。只读 pane 不会为布局变化获取控制权。输入仍限于焦点。已绘制布局是 Client 视图结果，Host 尺寸调整另行异步进行。

上报的根 `area` 即使在放大时也包含全部未放大的 Panel 和分隔矩形。放大仅改变选中 Panel 的显示矩形；标题和滚动条可能使显示矩形小于根范围。
