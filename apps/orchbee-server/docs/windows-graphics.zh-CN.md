# Windows Panel 图形

## 已接受的结果与所有权

既有 Windows CLI 替换授权包括真实的 Panel 图形 set、clear、information 和分帧 stream
（R078–R081）。参考行为固定为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的
`app/api/pane_graphics.rs`、`api/server/pane_graphics_stream.rs` 和 `app/pane_graphics.rs`。
仅有公开包装不能证明渲染器可用。本次不增加 Desktop、macOS 或 Linux 运行时。

Site 按规范 Panel 与 Presentation 身份拥有临时图层。既有导航管理权限准入视觉写入，读取
权限准入信息和有界资源读取。图形不写 PTY 输入，也不改变终端滚动历史。Client 拥有真实
渲染表面、裁剪、能力观察与绘制确认。调用者提供的 cell 尺寸、viewport 可见性或渲染成功
不是权威事实。

Primary selector 在准备前解析为一个当前 Presentation。该身份或终端 binding 被替换或删除
使准备失效；重命名和保留身份的移动不制造替换。提交前重复来源准入。Presentation 消失时
图层消失；它们不进入持久 checkpoint。

## 图像与上限

Wire 准入 PNG、RGB、RGBA 和 BGRA，带显式尺寸与规范补位 base64。一次性输入最多
512 KiB，stream frame 最多 16 MiB。每轴尺寸为 1–4096，解码 RGBA 最多 32 MiB。
分配前检查长度算术。PNG 必须完整解码，且与声明尺寸一致。通过关联的只读适配器复用
既有 Host 所有的 Wuffs PNG 解码，不添加 shell 辅助、PTY 转义注入、新解码依赖或持久
服务。既有临时 viewer 所有者保留不确定物理回收，解码或回收失败不能安装图层。

规范存储像素为 RGBA。替换先准备完整像素，再释放旧图层。畸形输入、来源变化、预算耗尽
及 stream 冲突保留旧图层。Site 最多保留 64 个图层 slot，每个 Presentation 最多 16 个，
总存储像素最多 64 MiB。单个准备图像及解码暂存独立有界。图形所有者串行解码，但不持有
规范拓扑锁。

Catalog 读取只含 metadata，不复制完整图像。每次像素读取最多 256 KiB，固定精确资源版本、
尺寸、偏移及总长度。Client 准备整个资源，仅在所有 chunk 一致且当前 catalog 仍标识该资源
后发布。未知、过期、截断或超大 chunk 不会产生局部更新像素。

## 图层与 producer 生命周期

Layer ID 含 1–80 个 ASCII 字母、数字、点、下划线或连字符。省略 CLI layer ID 时选择
`primary`。Placement 使用有符号 viewport 行／列、可选显式网格尺寸（零请求自然适配）及
有符号 z-index。裁剪与 alpha 合成在实际 Panel 表面内进行。负坐标或超大 placement 不能
绘制 chrome 或其他 Panel。

| 状态       | 输入                  | 条件                                    | 结果                           |
| ---------- | --------------------- | --------------------------------------- | ------------------------------ |
| 空闲／静态 | Set                   | 当前来源、有效完整图像、容量            | 安装静态图层                   |
| 空闲／静态 | Stream open           | 当前来源、producer slot 可用            | 一个连接拥有 stream            |
| Streaming  | Set／clear／其他 open | Producer 活动                           | 拒绝且不改变像素               |
| Streaming  | Frame                 | 精确所有者、递增非零 sequence、有效图像 | 原子替换 frame                 |
| Streaming  | 重复／旧 sequence     | Sequence 未推进                         | 拒绝且不替换                   |
| Streaming  | Close／EOF／取消      | 所有连接结束                            | 释放 stream 和图层             |
| 静态／空闲 | Clear                 | 没有活动 producer                       | 移除图层；不存在时成功且未变化 |
| 任意       | Presentation 删除     | 规范身份不再存在                        | 回收图层与 producer            |

Stream framing 遵循参考：有界 JSON header，后接声明长度的原始字节 body。每个接受的 frame
返回真实 sequence／revision 回执。Damage rectangle 是针对完整规范 frame 的有界提示，
不是让未提供像素未定义的 patch。错误 framing 结束 producer 并释放图层。私有文件 frame
仅准入 Site 所有的 producer 根下精确普通文件，固定尺寸并有界读取；消费后的 lease 与
producer 根在最后使用者结束后释放。外部路径不是上传捷径。丢失写入回执保持未知，
不能重放。

## Windows 生产者文件归属

托管 Windows Site 为每个已准入生产者按需分配一个私有目录。纯 open 准备先证明规范来源、
连接归属和容量；目录 I/O 在规范锁之外执行，提交时再次核对来源和生产者。幂等 open
返回同一目录。分配失败保留旧静态层。非托管 endpoint 模式不公布文件目录并拒绝文件帧。

Server 文件所有者最多保留 128 个根目录，包含失败和退役目录，存活生产者最多 64 个。
准备 token 防止尚未提交的根目录被回收。规范图层所有者只保留目录身份，不保留 Windows
句柄、不执行文件系统操作。帧准备在生产者／来源／序号准入后租用该精确根目录；完整复制
并归一化 RGBA/BGRA，再原子提交，无需 base64 往返或不完整补丁。帧头仍限定 16 MiB，
damage 只提示完整帧的变化。Info 仅向请求连接返回其既有生产者目录，读取者拿不到其他
生产者的根目录。

规范移除、关闭和 EOF 立即退役生产者。根目录存在时，既有 Close 维护所有者最多每
250 ms 核对保留目录；仅在最后一个准备／读取 token 结束后，于锁外回收退役目录。
删除失败保留精确归属记录并重试；退役目录不能准入新帧。关闭先拒绝写入，等待准备结束，
退役生产者并回收全部自有根目录，之后才能发布 Stopped。忙碌租约／目录或清理失败不能
证明干净关闭。崩溃恢复沿用既有 Active 记录拒绝规则，不接管未知图形根目录。

定向证据以真实 Windows 文件／句柄轨迹和生产 Workbench 会话映射归属：来源变化和外来
生产者不能读取，畸形文件保留像素和序号，完整 BGRA 正确归一化，幂等 open 稳定，
EOF／拓扑移除释放根目录，忙碌清理保留责任。最终组合／全量验收与物理像素传输仍是
独立义务。

## Client 渲染与真实信息

原生 Windows Client 必须消费安装后的图层 catalog 和完整像素。普通 cell 表面提供彩色
raster 回退；协商的像素传输可提高分辨率，且不改变图层所有权。仅有实际能力证据后才报告
像素传输、物理 cell 尺寸和像素鼠标；未知尺寸保留为未知。信息区分安装与对应 Client 绘制，
隐藏或过期 Client 不伪造可见性或确认。Clear、zoom、切换、resize 和退出 TUI 沿同一渲染器
所有者回收输出资源。

## 证据与集成

依赖顺序为严格图像／资源契约、Site 图层／stream 事务和上限、关联的既有 Host PNG 解码、
实际原生 Client 消费／渲染，然后是公开 CLI grammar／framing／回执。定向检查覆盖完整
旧图层保留、来源变化、跨连接 producer 冲突、EOF 回收、顺序／耗尽、chunk 不一致、
PNG 上限、alpha／z／负 placement 裁剪及真实 Windows CLI 管道。剩余主要能力全部实现后
再跑 Windows 组合渲染及全量回归。仅基础测试不能将这四个方法判为对齐。

## 当前实现范围

Windows Site 图层事务和 Host 只读 PNG 适配器已实现。规范投影持有共享图层存储，在提交时立即清理消失或重新绑定的来源，无需等待读取图形。准备阶段有独立串行锁；锁顺序为规范状态所有者、图层存储，PNG I/O 不持有这两把锁。静态 primary 选择器在解码后再次核对。

OBWP 57/58 承载封闭的请求和结果。OBNG 承载受限、由 viewer 定位的 PNG 请求；复用带关联标识的 OBNR 返回记录。Terminal Host 80/81 使用独立的 graphics-image 编码（5），上限为 32 MiB 加 56 字节结果头，普通输入上限保持原有约束。

原生 CLI 上传／流入口及 [原生 TUI 字符栅格消费者](../../orchbee/docs/native-panel-graphics.zh-CN.md) 已实现。独立读取通道暂存按版本定位的完整资源；已认证 Client 发布者记录成功且未遮挡的绘制观察。Info 采用新鲜且匹配的 Client 事实，未知物理像素尺寸保持 null。托管 Windows 生产者已拥有私有文件帧目录和完整有界复制，支持准备／读取 token、维护重试和干净关闭结算。非托管 endpoint 模式不公布目录。物理像素传输及最终组合／全量验收仍待完成；四项对比行仍需精确参照复查和 Windows 验收。最终集成时统一确定原生 cohort 版本并重新打包；本检查点不授权合入 main 或替换旧 cohort。

定向证据包括 Rust／TypeScript／JSON Schema 共用的 62 个案例、图片与载体边界、8 个图层所有者测试、7 个生产 Server 会话／观察者／文件案例及 Client 新鲜度／范围测试，以及 5 个 Host PNG／图形测试（含真实 Windows PTY 创建和 PNG 解码）。这是组件证据，尚非组合渲染或最终验收。

## 消费者实现顺序

在同一已授权 Windows 补齐目标下，基础完成后先接入 CLI 上传及保留流，再接入实际 TUI 渲染与能力事实；两项均完成之后才进行组合验收。CLI 语法与输入规范由 [Windows Panel 图形 CLI](../../orchbee/docs/windows-panel-graphics.zh-CN.md) 拥有。实现顺序调整不改变对齐完成条件。
