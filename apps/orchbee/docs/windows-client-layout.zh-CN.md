# Windows Client Panel 几何

本参考文档负责原生 Windows CLI 对活动 TUI Client 的 Panel 布局检查。TUI 持有视口、界面占用、焦点与缩放状态。Server 路由请求并保留有界观测；成员关系仍以规范 Workbench 拓扑为准。布局读取绝不选择上下文或 Tab、附着终端、改变缩放或调整终端尺寸。

## 观测

查询指定精确的 Client、TerminalContext、Tab、源 Panel 及最低 Workbench revision。现有已认证 Client 通道在把请求交给 UI 之前，进行一次可取消的规范 Snapshot 读取。Snapshot 不可用或目标缺失时明确失败。UI 使用该 Snapshot 投影目标，不把它安装为活动上下文；低于请求最低 revision 的 Snapshot 会被拒绝。对于选中 Tab，几何使用当前渲染成员缓存及其 revision；缓存尚未达到请求最低 revision 时明确失败，不通过刷新成员关系引发终端 resize 副作用。非活动目标使用取得的 Snapshot，明确作为未选中的投影。

结果包含目标与规范 revision、采样时的单元格视口与终端区域、目标 Tab 是否选中、本地缩放标志，以及必填但可为 null 的焦点 Panel。未访问及非活动上下文均可读取。Client 没有该目标的本地焦点 Panel 时，焦点为 null；投影使用渲染器的首叶回退，不声称该叶已选中。非活动 Tab 的几何是它在采样视口中的投影，不表示正在绘制。

每个规范 Panel 都包含必填但可为 null 的未缩放外框、未缩放终端内容及显示终端内容矩形。null 表示渲染器没有在该层放置此 Panel。缩放保留未缩放几何，仅让焦点 Panel 显示完整内容区域。Minimal 视口反映渲染器的单 Panel 回退；过小视口不编造几何。分割记录稳定的 split/slot 身份、方向、渲染比例，以及必填但可为 null 的区域／分隔线矩形。保持规范遍历顺序。

全部矩形位置与尺寸均为整数单元格。界面占用、标题行、共享分隔线、最小尺寸折叠与奇数单元格取整，都来自绘制和终端尺寸同步使用的同一投影。终端像素尺寸不影响单元格邻接。

## 邻居与边缘

导航使用未缩放的外框叶矩形，不使用扣除标题后的内容矩形，也不使用归一化拓扑坐标。候选必须完全位于指定方向，并在垂直于该方向的轴上有正重叠。依次选择边缘距离最小、重叠最大、二倍中心距离最小、规范遍历顺序最前者。坐标相加前扩大整数类型。源几何为空或隐藏时明确标记不可用。

观测包含源 Panel 的四个可为 null 的邻居。方向边缘通过比较可见源的未缩放矩形与终端区域边界确定，与参考实现的物理边缘处理一致。无邻居与物理边缘是不同事实。隐藏源不作边缘判断。缩放不会抹掉有效的未缩放邻居。采用的导航排序固定参考 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/layout.rs::find_in_direction`；原生界面占用及降级仍遵循 Orchbee 契约。Orchbee 在整个允许坐标范围内扩大二倍中心计算类型；不采用参考实现对极大坐标的 u16 中心饱和行为。

## 路由、生命周期与边界

布局是现有 Client 控制通道上的读取意图，具有独立的 layout-read 完成结果及保留状态。它继承 [Client 视图控制](windows-client-control.zh-CN.md) 的精确 Client／调用者选择、操作身份、新鲜度、单次投递、排队过期、断连结算及调用者超时／取消行为。读取结果不能确认导航或缩放命令，普通 applied 成功也不能确认布局读取。

UI 在验证目标 Snapshot 后，于一个 tick 内捕获观测。结果描述该时刻；后续尺寸、输入或拓扑变化不会重写回执。不声称非活动 Tab 已成功绘制。观测前发生本地中断会拒绝查询。Snapshot I/O 在 UI 线程外执行，具有自有截止时间及可唤醒退出，不自动重试。

专用 OBWP 帧类型 27/28 承载布局完成／结果，在分配前限制为 1 MiB；普通 Client 帧类型 25/26 保留分配前 16 KiB／64 KiB 限制；现有普通请求及小结果仍按其 action 执行原有边界。Panel 与分割条目之和不得超过规范上下文的 4096 节点限制；身份唯一，所有焦点、源及邻居引用必须存在。矩形保持在采样视口与相应内容区域内。

Server 在查询入队前，从 16 MiB 编码布局预算中预留 1 MiB。完成后用有界编码观测的实际大小替换预留。拒绝、无结果断连及回执过期释放预留。现有总计 256 回执及完成后 60 秒保留期仍有效。容量拒绝发生在投递之前；绝不驱逐待处理或未过期结果来腾出空间。

## CLI 与证据

`panel layout`、`panel neighbor --direction left|right|up|down` 和 `panel edges` 接受位置参数 Panel，或互斥的 `--panel`／`--current`，并使用现有 Client 选择规则。连接或副作用前完成解析。机器输出保留完整布局观测，以及必填但可为 null 的 `projection`：邻居结果含方向、源可用性及可为 null 的邻居 ID；边缘结果含源可用性及四个可为 null 的物理边缘布尔值。在收到布局结果前，projection 为 null；源隐藏时边缘布尔值为 null。文本输出呈现所请求的视图，不隐瞒不可用几何。

证据覆盖精确的界面占用／取整／最小尺寸几何、嵌套与退化树、独立邻居排序向量、带缩减重放的生成层级不变量、非活动及未访问目标、无选择／附着／resize 副作用、双 Client 隔离、过期成员关系、Snapshot 失败、大小边界、容量、中断及退出。Rust、TypeScript 与 JSON Schema 共享一致性语料，但独立验证。受控 Windows TUI 轨迹把契约连接到真实显示。TLA 模型资产描述扩展的回执生命周期；按用户要求暂不执行，也不声称已执行。同作者检查不等于独立复核。

比例修改、方向聚焦、交换及布局导入／应用分别交付。本读取契约不声称这些能力已具备。扩展仅用于精确 cohort 准入；回滚只丢弃易失的 Client 观测与回执，无需迁移规范状态。

## 验证映射

| 声明                                               | 维护中的证据                                                                                                                                          |
| -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 几何、焦点保留、嵌套拓扑与折叠                     | `src/tui_layout/tests.rs`、`src/tui_layout/tests/generated.rs`：八个固定种子，每种子 196 个动作，JSON 重放与删除缩减                                  |
| 选中／非活动／未访问目标的只读行为及 Snapshot 中断 | `src/terminal_session/ui/client_layout/tests.rs`、`src/terminal_session/client_control/tests.rs`、`src/terminal_session/client_control/lane/tests.rs` |
| 精确身份、边界、物理边缘及邻居排序                 | `crates/workbench-protocol/tests/client_layout.rs`、共享 Client 视图语料及独立 TypeScript 校验                                                        |
| 预留、完成、断连与过期                             | `apps/orchbee-server/src/client_view_registry/tests.rs`；`ClientViewControl` 模型抽象纯度与保留预算转移                                               |

以 `src/` 开头的路径相对于本应用，其余路径相对于仓库。运行 `cargo test --locked -p orchbee --bin orchbee`、`cargo test --locked -p orchbee-workbench-protocol --test client_layout --test client_view` 及 `cargo test --locked -p orchbee-server client_view_registry`。仓库门禁负责进程及绑定检查。参考捕获维护使用八个确定性种子，在边界至多为 1,200 单元格的非空矩形上，对比未改动的固定 Herdr 邻居函数与协议投影器，共 65,536 次方向查询；不覆盖参考实现的中心饱和域。维护中的协议向量独立于外部 checkout 保留决策边界。
