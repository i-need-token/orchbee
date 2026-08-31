# Ghostty 投影表面渲染器一致性

## 适用范围

依据固定版本的 Ghostty 渲染器、字体与整形器源码审计投影客户端渲染器。审计涵盖像素几何、字形约束、整形边界、光标呈现、投影 Kitty 图像合成，以及其他存在直接对应关系的浏览器渲染行为。

它明确排除 VT 解析、PTY 字节、终端模式、历史记录和选择权限、语义输入编码、搜索所有权、窗口、选项卡、拆分、更新和其他应用程序 UI。这些排除是架构边界：Ghostty VT 权限仅存在于 `apps/terminal-host` 中，而此包消耗打包的主机投影。

| 基线         | 值                                         |
| ------------ | ------------------------------------------ |
| 已验证       | 2026-08-25                                 |
| Ghostty 提交 | `9c3ec931d64561a8407dde7ac984ce156ae91539` |

`pnpm audit:ghostty-panel` 报告已检查的 Ghostty 源的权威声明和处置计数。审核接受 `test {}`、`test name {}` 和 `test "name" {}` 并拒绝丢失、重复、过时或超出范围的清单映射。

## 处置

| 处置       | 覆盖方式                                              |
| ---------- | ----------------------------------------------------- |
| 已覆盖     | 适用的声明，并有持续维护的投影渲染器证据              |
| 缺口       | 适用且可观察的渲染能力缺失或不完整                    |
| 浏览器产品 | 精确的本机算法替换为 `FontFaceSet`、画布文本或 WebGPU |
| 上游内部   | 分配器注入、本机缓存/容器或没有类似内容的页面布局细节 |
| 应用层     | 投影表面之外的本机配置或产品行为                      |

声明计数是来源追溯指标，而不是面向用户的功能完成百分比。

## Orchbee 证据

| 代码         | 证据                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------ |
| `E-RENDER`   | `src/rendering/__tests__/renderer.unit.test.ts` 和投影终端集成测试                                           |
| `E-FIT`      | `src/__tests__/surface-size.unit.test.ts`                                                                    |
| `E-SHAPE`    | `src/__tests__/buildTextShapingPlan.unit.test.ts` 和投影渲染器测试                                           |
| `E-LINK`     | `src/__tests__/terminal-row-links.unit.test.ts`、投影终端测试和投影渲染器测试                                |
| `E-IME`      | `src/input/__tests__/preedit.unit.test.ts` 和投影终端测试                                                    |
| `E-SELECT`   | `src/projected/__tests__/projected-terminal.unit.test.ts` 和投影渲染器测试                                   |
| `E-FONT`     | 字体度量、字形约束、完整的 Ghostty sprite 路由、生成的光栅/约束数据、整形及投影渲染器测试                    |
| `E-GRAPHICS` | 严格 OBKG/OBRW store、`src/rendering/__tests__/renderer.unit.test.ts` 与 Terminal Host 投影/动画差分测试     |
| `E-ENV`      | 精确的有界面 `terminal-parity/CF-03-visual-environment` 系统见证，以及每个具名视觉见证所断言的控制条件与限制 |

下方每项声明恰有一种处置。`<unnamed@line>` 标识固定提交中未命名的上游测试。

## 受控视觉系统环境

稳定的有界面 `terminal-parity/CF-03-visual-environment` 见证负责共同的合成 Canvas 证据基线。它通过生产应用加载器注册仓库中的产品 JetBrains Mono 与 Symbols Nerd Font 资产，在渲染器能力边界强制使用 Canvas 2D，并断言精确的 `900x500` Chromium 视口、DPR `1`、`en-US` 文档与浏览器区域设置、UTC 时区、浅色配色方案和减少动态效果偏好。见证会等待字体集就绪，并要求所选产品文本字体已加载，才接受渲染表面。

若证据有刻意保留的领域差异，其具名见证会明确说明并断言该差异：DPR 跟踪只改变设备指标；可见性跟踪自行控制动画时间；即时滚轮跟踪断言规范 Canvas transform；真实 Electron/PTY 跟踪使用已挂载的原生窗口；WebGPU 测试使用确定性的所有者本地适配器。断言会精确比较语义状态、几何、顺序、颜色或字节，不使用可能抹除语义差异的截图阈值或标准化。该基线不声称操作系统抗锯齿、原生显示器传输、WebGPU 驱动行为、其他浏览器引擎或原生 OS 效果完全一致。

## 已配置的产品字体

`@orchbee/terminal` 将 `monospace` 保留为其源中立默认值。共享 Web 应用程序在构造任何投影表面之前会注册这些未修改的 Ghostty 字体文件和随附的许可文本：

| Ghostty 依赖项                | 产品字体                             | 应用资产                                                                                    |
| ----------------------------- | ------------------------------------ | ------------------------------------------------------------------------------------------- |
| JetBrains Mono 2.304 variable | `Orchbee JetBrains Mono`             | `JetBrainsMono-Variable.ttf`、`JetBrainsMono-VariableItalic.ttf` 和 `JetBrainsMono-OFL.txt` |
| Nerd Fonts Symbols Only 3.4.0 | `Orchbee Symbols Nerd Font` 后备字体 | `SymbolsNerdFont-Regular.ttf` 和 `SymbolsNerdFont-LICENSE.txt`                              |

这些文件位于 `apps/web/src/workbench-client/projected-terminal-renderer/fonts/`。`FontFaceSet` 准备过程包含 U+F120，避免首次 Canvas 与 WebGPU 绘制和符号后备字体产生竞态。只有预置字体栈不支持的 CJK 与 emoji 才交由浏览器系统字体负责。现有 `pnpm audit:ghostty-panel` 命令会校验 Ghostty 依赖版本，以及每项字体和许可证资产的 SHA-256。

共享 Web 产品将其初始浏览器大小固定为 13 CSS 像素，以匹配 Ghostty 在校准的 macOS 比例上的默认 13 点设置。每个 Canvas 字体上下文请求 `geometricPrecision`；即使选择的 JetBrains Mono 轮廓和粗细相同，Chromium 的默认 `auto` 易读性提示也会加厚小尺寸光栅。 Ghostty 的固定 CoreText 路径启用抗锯齿和子像素定位，但在 `font-thicken = false` 时禁用字体平滑。

在固定 Ghostty 提交上，以 ZeroGH 为参照进行的原生 macOS 2x 校准将轮廓选择与目标尺寸光栅化区分开。Chromium 中使用 130 CSS 像素、ZeroGH 中使用 130 点，对常规字重 `A` 进行真实 10 倍渲染时，二者均得到 130×190 像素的墨迹边界，Alpha 覆盖面积分别为 9,462.10 和 9,469.57 像素。在实际的字号 13 设置下，常规字重 `1.jpg` 在 Chromium `auto` 下测得 479.88 像素，使用 `geometricPrecision` 后为 369.71 像素，在 ZeroGH 中为 369.76 像素。产品仍以字号 13 的光栅结果作为发布标准；10 倍渲染仅作为字体、轮廓与字重一致的诊断证据。

渲染器以 1,000 CSS 像素采样所选浏览器字体，把未经舍入的比率缩放回配置尺寸，并按当前物理设备像素比对基线取整。`rounds sampled font metrics at the physical pixel scale like Ghostty` 回归覆盖了先按 CSS 尺度取整、再进行高 DPI 光栅缩放所造成的一个物理像素基线误差。DPR 转换期间，单元格宽高仍保持稳定的 CSS 几何。

浏览器文本段规划器会把占多个单元格的 Host 字素排除在自然字宽整形段之外。每个宽字素锚定在其投影头单元格，间隔尾单元格不绘制，随后较窄的复杂文本段则正常开始。`anchors wide fallback graphemes to their Host cells and resumes shaping after tails` 回归同时覆盖累积后备漂移和 emoji 之后组合文本段丢失的问题。窄单元格文本段会按活动光栅尺度，将字体字宽与网格字宽之间的小数差作为 Canvas 字间距加入。选择与搜索覆盖层保留该文本段，仅对裁剪后的单元格对齐区域重新着色，不单独重新整形；`keeps grid-fitted shaping intact across selection boundaries` 与 `keeps a shaped run at the same horizontal origin when selection recolors it` 回归分别覆盖 Canvas 与 WebGPU 路径。WebGPU 图集还会让纹理四边形保持设备像素对齐，并把横向裁剪余数保留在光栅中；`keeps WebGPU run baselines on the cell anchor across cursor splits` 覆盖了光标切分后新首字形具有不同带符号墨迹边界的情况。

## 投影 Kitty 图形

固定 Ghostty 仍是解码图像、帧选择、virtual 与 relative 放置、源相交及 z 顺序的状态所有者。Terminal Host 把这些状态解析成有界 OBKG `0.0.2` 自有像素与绝对放置。路径、文件描述符、共享内存名称或映射、压缩流、编码容器及未解析 pin 都不会到达本包。

打包 store 会原子提交图像资源、放置、资源修订与单调 animation tick。Canvas2D 和 WebGPU 对每项图像 generation 缓存一份转换后资源，使用相交的源矩形与目标几何，并在三个对应接缝绘制：`z < -1073741824` 位于单元格背景下方，`-1073741824 <= z < 0` 位于背景与文本之间，`z >= 0` 位于文本上方。替换与失败路径不能保留过时 Canvas 或 GPU 资源。Render-detail OBRW `0.0.3` 携带一份重定位到所选历史窗口的独立完整图形快照；text detail 不携带图形。

上游 `src/renderer/image.zig` 声明共同映射到 Host 投影与客户端 renderer：源相交及 relative/virtual 放置在线协议前解析；pending/replaced 图像由原子 store 替换与资源清理覆盖；当前动画帧由 Host generation/tick 证据与按 generation 建键的 renderer cache 共同覆盖。包 renderer 不会成为第二个 Kitty 解析器或动画状态机。

## 宿主衍生的投影语料库

`@orchbee/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin`、`ghostty-alternate-surface-v0.0.3.bin` 和 `ghostty-render-row-window-v0.0.3.bin` 共同组成一套由 Host 拥有的原生表面证据。`apps/terminal-host` 从一个固定版本的原生 Ghostty VT 和一个 Host Projection 生成 OBTF 夹具对：馈送主流、捕获快照、在同一终端进入备用屏幕，再捕获第二个快照。另一个原生终端会重放主流，追加 DECSCA 受保护哨兵和恢复未受保护后的哨兵，并通过生产 `row_encoding.encode` 到 `projection_buffer.encodeRenderSnapshot` 路径生成 render-detail OBRW。三个夹具均不由 TypeScript 编码器构造。

Terminal Host 验证会重新生成并逐字节比较全部三个夹具。投影渲染器测试会解码主 OBTF 字节作为 Canvas 和 WebGPU 证据，再通过打包存储提交两个 OBTF 快照以获得转换证据。OBRW 消费者独立固定完整的六行乘八单元判定：行索引、标志、提示类、字素与超链接字符串及其原始字节范围、全部三个颜色列、每个样式标志和下划线类型、宽度拓扑、语义内容、受保护标志和提交元数据。它要求严格解码与可信结构视图匹配同一矩阵，通过 `replaceProjectionBufferSnapshot` 提交可信帧，并按相同语义列检查每一存储行。线协议有效的内存变异覆盖前景/背景交换、受支持样式变化、受支持语义变化、清除受保护单元、规范移除超链接、协调行换行和有效宽度拓扑替换。

该语料库启用 Ghostty 字素簇模式 `2027`，然后在一个 OSC 8 链接内包含窄文本、CJK、组合序列和 modifier-plus-ZWJ 表情符号。它将最终链接的宽字素放在右边缘，以便 Ghostty 发出包裹的间隔头，然后是下一行的宽头和间隔尾。然后，在出现明确的硬换行和最后一个未链接的单元格之前，链接将关闭。另外两个硬行设置非黑色动态默认值并使用显式的前景色/背景色、反色、微弱、不可见、闪烁、上划线、删除线、所有五种下划线类型、独立的下划线颜色、粗体和斜体。因此，共享断言涵盖了这些边界：

- Ghostty 拥有字素分割和末端宽度。
- 主投影将每个完整的字素保留为一根字符串，并保留窄的、宽的、间隔尾部和包裹的间隔头拓扑。
- TypeScript 解码器和打包存储保留准确的字节和宽度代码。
- Canvas 对每个非间隔字素绘制一次；它从不将间隔单元绘制为独立的字形。
- 搜索会连接软包装行，但不会跨越后面的硬边界，除非明确要求跨越行。
- 链路检测在软包上保留一个 OSC 8 范围，并在硬线之前停止它。
- 可访问性文本和单元格偏移映射保留每个物理行，同时相同的行标志继续区分软边界和硬边界。
- Ghostty 拥有 SGR 解析和样式组合；主机、解码器和打包存储保留生成的颜色编码、标志和下划线类型，而不重建 SGR 状态。
- Canvas 和 WebGPU 使用相同的样式行，包括显式 RGB 零、默认颜色、独立下划线颜色、隐藏文本、闪烁阶段和装饰几何形状。

三个夹具构成确定性一致性证据，而不是第二个 VT 实现或产品样本。Terminal Host 负责原生生成，`@orchbee/terminal-protocol` 负责已跟踪字节和导出，本包负责消费者语义。经审核变更语料、Ghostty 固定版本、Host 编码器、OBTF/OBRW/OBPB 格式、解码器、打包存储或绘制契约后，运行 `pnpm --filter @orchbee/terminal-host surface-fixtures:generate`，并审核生产者与消费者断言。该证据集证明这条有界生产者到消费者映射；它不证明 PTY 交付、历史锚点行为、生命周期策略、其他平台运行时或更广泛的可信传输。

## 打包式投影

一个包装单元呈现 Ghostty 样式元组 `(foreground, background, underline color, style flags, underline kind)`。 RGB 零是有效的显式颜色； `default` 和 `none` 仍然是不同的标记编码，并且绝不能从 RGB 值推断出来。客户端不保留 SGR 状态机。

Canvas 和 WebGPU 拥有等效的呈现规则：

- 仅`TerminalColorEncoding.default`通过客户端有效的默认前台或后台进行解析；显式 RGB（包括黑色）、主机投影调色板 RGB 和 `none` 保留其主机投影行为；
- 在选择或搜索覆盖之前反向交换已解析的前景和背景；
- 不可见和隐藏的闪烁阶段抑制文本和装饰，同时保留单元格背景；
- fade 将半 alpha 应用于文本和装饰，而不是背景；
- 单下划线、双下划线、卷曲下划线、点划线和虚线下划线保留其独特的几何形状，并且明确的下划线颜色仅覆盖该下划线的有效前景；
- 删除线和上划线使用有效前景和相同的微弱 Alpha；
- 粗体和斜体影响两个后端的字形光栅化；和
- 字形遥测计数提交或绘制的字形，而不是隐藏的字素。

改变表面透明度是一种成分策略的改变。即使打包存储没有脏行，它也会请求渲染并强制一次完整的 Canvas 重新绘制，因此在删除默认背景填充时会保留显式背景。 `src/__tests__/ghostty-surface-parity.unit.test.ts` 拥有这些规则的本机到两个后端的回归。

## 打包呈现颜色

启动配置文件种子 Ghostty 的默认前景、背景、光标颜色和 256 色调色板。 OSC 4、10、11 和 12 突变相同的 Ghostty 状态。主机投影将调色板单元格颜色解析为 RGB 并在每个帧中发布当前默认的前景、背景和可选的光标颜色。打包的客户端永远不会重建调色板或替换 `ITheme` 中的那些终端颜色。

可选客户端 `defaultColorOverride` 是较窄的显示投影，而不是第二个终端颜色权限：它仅影响打包的 `TerminalColorEncoding.default` 前景/背景令牌。它不会改变或替换主机元数据、调色板投影的 RGB、VT 状态、OSC 查询回复或主机光标颜色。渲染器在应用反向之前解析每个原始的前景/背景角色；默认下划线颜色使用有效前景，而显式下划线颜色保持显式。共享的 Orchbee Web/桌面组合不使用此兼容性投影：它保持 `defaultColorOverride` 为空，并通过完整的主机运行时主题事务应用浅色、深色或自定义颜色。

应用程序呈现仍然是一个单独的、更窄的层：

- 选择和活动/非活动搜索颜色来自客户端主题；
- 显式配置的光标重音控制块光标内绘制的文本，而主机背景是后备重音；
- 丢失的主机光标颜色回落到主机前景；
- 画布在其第一帧之前保持透明，因此所属容器提供加载表面，然后不透明调整大小并清除重用最新有效的默认背景；
- `allowTransparency` 仅控制渲染器清除。 `backgroundOpacity` 仍然是构成终端根的完整源中立配置值，其 alpha 既不影响字形也不影响显式单元格背景；和
- Workbench 终端根采用每个提交的有效默认背景，而 Electron 仅使用启动配置文件背景作为其预文档窗口表面。

本机语料库设置 OSC 4、10、11 和 12 值，并证明生成的元数据、解析的调色板单元、光标绘制、Canvas 输出和 WebGPU 上传。主机运行时还通过固定的 Ghostty 流发送匹配的 OSC 查询并验证确切的回复；其单独的 real-PTY 跟踪可验证终端回复路由和计费，而不依赖于固定长度的颜色回复读取。渲染器单元测试证明，传统的终端颜色主题字段不能成为第二个打包调色板，并且应用程序覆盖颜色仍然是可配置的。

每个 OBTF/OBPB 投影还包含通过其行和元数据捕获的主机主题修订。 `ProjectedTerminal` 在主机请求之前准备覆盖和光标资源，将传入帧暂存在私有打包存储中，并且仅当该存储命名确切的提交修订时才允许发布。应用程序屏障同步预检每个已安装的终端，提交其暂存存储和覆盖状态，然后在同一 JavaScript 任务中更改应用程序设计令牌主题。渲染器准备失败不会发送主机突变；被取代、隐藏、暂停或不匹配的阶段不能公开混合版本。投影终端单元跟踪涵盖精确提交、恢复最新和渲染器失败保存，而桌面 `terminal-parity/SU-09-theme` 观察在一次完整发布之前保存主机结果和修订标记帧的真实运行时演示装置。

## 打包链接和语义边界

Ghostty 拥有显式超链接和 shell 集成语义。 OSC 8 目标进入主机样式/字符串表，而 OSC 133 提供每个单元格的 `prompt`、`input` 或 `output` 语义内容和每行的提示类别。 OBTF 和 OBRW 直接保留这些字段；浏览器不解析任何转义序列。

本机语料库在软包装上携带一个 OSC 8 链接，然后在其最后一行中的调色板颜色单元格周围跨越 OSC 133 提示、输入和输出边界。 TypeScript 解码器和打包存储断言验证原始目标、行提示类和所有三个单元语义。单独的投影终端跟踪读取生成/修订固定的主机行窗口，解析确切指针单元格处的显式链接，在修改器辅助单击时重新验证该单元格，并发出一个键入的 URL 请求。然后，工作台组合跟踪通过其允许的外部链接操作和环境拥有的效果路由该请求。

显式 OSC 8 范围仍优先于检测到的 URL 或路径文本。来源中立的应用策略把激活限制在 `http`、`https` 与 `mailto`；Host metadata 从不授予文件系统或通用 scheme 权限。独立且默认关闭的 Desktop-local attachment capability 可以在同一 generation/revision-pinned 重校验后准入 typed file target。

头部 Canvas 2D 见证了 `terminal-parity/FS-05-file-links` 和 `terminal-parity/FS-06-url-policy` 通过生产投影表面、工作台操作和浏览器效果提供生成固定的合成主机行窗口。他们观察普通的白名单检测以及悬停、上下文和直接激活；自动FTP文字；不支持显式 OSC 8 元数据；显式文件元数据；和检测到的路径。文件和不受支持的目标不会产生外部影响。这些见证人控制浏览器组成和策略行为；它们不会调用操作系统 URL 处理程序、解析 OSC 字节、声明文件系统存在或添加文件打开权限。

有界面 Canvas 2D 见证 `terminal-parity/FS-19-local-file-links` 与 `terminal-parity/FS-20-image-preview` 会用 fake local opener 与合成且已验证的 PNG bytes 启用该可选 capability。它们观察 relative-path resolution、modifier activation、300 ms hover dwell、target 变化时的 cancellation、拒绝第一个 request 的迟到 completion，以及第二个 target 对应的生产锚定 pointer-transparent card。它们不会调用操作系统应用、读取真实文件、自动操作 Finder，也不会授予 Browser/Gateway 权限。

## 打包元数据转换

标题、当前目录、活动屏幕、光标状态、交互模式、生成、修订和行属于一个预计提交。主机捕获一次终端状态并将这些值写入同一条 OBTF 记录中；打包的客户端会暂存完整的帧并将其元数据和行替换在一起。它永远不会独立于它们描述的行来应用标题或模式回调。

主夹具设置 OSC 2 和 OSC 7 元数据、隐藏的稳定下划线光标、应用程序光标/键盘模式、括号粘贴、焦点报告、按钮运动以及 SGR 鼠标报告、modifyOtherKeys 2 级以及每个投影的 Kitty 键盘标志。配对流进入带有 DECSET 1049 的备用屏幕，更改两个元数据字符串，显示闪烁的条形光标，将鼠标报告更改为任何运动加上 SGR 像素，清除其他交互模式，选择两个 Kitty 键盘标志，并将网格替换为 `ALT`。

主机运行时回归通过其读取器提交接缝独立地提供相同的转换。它证明了 DECSET 1049 推进了一代，并使下一个投影成为包含新元数据的完整快照。 TypeScript 回归按顺序应用两个本机帧，并证明保留的主要元数据对象保持不变，而实时存储在一次提交中切换所有目标字段和行。

## 压缩光标投影

`PackedRendererMetadata.cursor` 保留规范的主机光标坐标。渲染器派生一个呈现范围而不更改该坐标，因此输入、可访问性和主机状态不会获取第二个光标所有者。

| 输入                                      | 演示专栏   | 演示宽度                       |
| ----------------------------------------- | ---------- | ------------------------------ |
| 光标单元格较窄且 `wideTail` 为 false      | 光标栏     | 一个细胞                       |
| 光标单元格为宽单元头，`wideTail` 为 false | 光标栏     | 两个细胞                       |
| `wideTail` 为真且光标列大于零             | 光标列减一 | 两个细胞                       |
| 零列处的 `wideTail` 无效                  | 零列       | 宽度源自光标单元格，绝不是负列 |

投影范围具有以下不变量：

- Canvas 2D 和 WebGPU 对块、空心块、条形和下划线光标使用相同的表示列和宽度。
- 一根柱子保持在前沿；块状、空心块状和下划线几何形状跨越整个宽字符。
- 文本整形将整个表示范围视为光标边界，因此连字不能穿过宽光标的任何一个单元格。
- 光标文本反转读取并剪切宽单元头字素；它从不尝试将间隔尾部呈现为独立字形。
- IME 锚定、输入编码、可访问性和主机确认的规范光标坐标保持不变。

参考是固定的 Ghostty 提交处的 `src/terminal/render.zig` 和 `src/renderer/generic.zig`。 `src/rendering/__tests__/renderer.unit.test.ts` 拥有确定性 Canvas 和 WebGPU 回归。这种纯演示投影没有长期状态、异步转换或 TLA+ 属性；仅当两个后端都执行真正的宽头/间隔尾排时，`PR-03` 才是完整的。

## 渲染器清单

- `src/renderer/State.zig` — 覆盖 (`E-RENDER`)：`preedit range covers exact cell width`； `preedit range shifts left at right edge`。
- `src/renderer/cell.zig` — 覆盖 (`E-RENDER`)：`Contents`； `Contents resize grows and shrinks`； `Contents clear retains other content`; `Contents clear last added content`; `Contents with zero-sized screen`。
- `src/renderer/cell.zig` — 覆盖 (`E-FONT`)：`Cell constraint widths`。
- `src/renderer/cursor.zig` — 覆盖 (`E-RENDER`)：`cursor: default uses configured style`； `cursor: blinking disabled`; `cursor: explicitly not visible`; `cursor: always block with preedit`。
- `src/renderer/link.zig` — 覆盖 (`E-LINK`)：`renderCellMap`； `renderCellMap hover links`; `renderCellMap mods no match`。
- `src/renderer/link.zig` — 浏览器产品：`renderCellMap inactive links don't allocate`；浏览器拥有的 link 索引与垃圾回收替代本机分配断言。
- `src/renderer/image.zig` — 覆盖 (`E-GRAPHICS`)：`kitty renderer ignores pending payloads and removes replaced placements`； `kitty renderer uses the intersected source rectangle`； `kitty renderer positions relative placements from the parent pin`； `kitty renderer relative placement with negative offsets`； `kitty renderer positions relative placements from virtual parent placeholders`； `kitty renderer uploads the current animation frame`；Host 解析规范关系，打包 store 与两个 renderer 后端保留其有界呈现。
- `src/renderer/metal/shaders.zig` — 浏览器产品：`<unnamed@278>`； WebGPU 着色器编译替换了 Metal 着色器编译。
- `src/renderer/shadertoy.zig` — 间隙：`spirv`； `spirv invalid`； `shadertoy to msl`; `shadertoy to glsl`。
- `src/renderer/size.zig` — 覆盖 (`E-FIT`)：`Size.balancePadding equal distributes whitespace equally`； `Size.balancePadding true shifts excess top to bottom`； `Padding balanced on zero`。
- `src/renderer/size.zig` — 覆盖（`E-FIT`、`E-SELECT`）：`GridSize update exact`； `GridSize update rounding`； `coordinate conversion`。

## 字体清单

- `src/font/Atlas.zig` — 覆盖 (`E-RENDER`)：`happy path`； `exact fit`; `doesn't fit`; `fit multiple`; `writing data`; `grow`; `writing BGR data`; `grow BGR`。
- `src/font/Atlas.zig` — 浏览器产品：`writing data from a larger source`；画布光栅化提供源作物。
- `src/font/Atlas.zig` — 上游内部：`grow OOM`； `init error`； `reserve error`; `grow error`; Zig 分配器失败注入没有浏览器等效项。
- `src/font/CodepointMap.zig` — 浏览器产品：`codepointmap`。
- `src/font/CodepointResolver.zig` — 浏览器产品：`getIndex`； `getIndex disabled font style`。
- `src/font/CodepointResolver.zig` — 覆盖 (`E-FONT`)：`getIndex box glyph`； Orchbee 在浏览器字体回退之前路由固定的 Ghostty 精灵面声明的每个 Unicode 代码点范围。
- `src/font/Collection.zig` — 上游内部：`<unnamed@930>`。
- `src/font/Collection.zig` — 浏览器产品：`init`； `add full`; `add deferred without loading options`; `getFace`； `getIndex`； `completeStyles`； `hasCodepoint`； `hasCodepoint emoji default graphical`; `face metrics`。
- `src/font/Collection.zig` — 覆盖 (`E-FONT`)：`setSize`； `metrics`。
- `src/font/Collection.zig` — 涵盖 (`E-FONT`)：`adjusted sizes`。
- `src/font/DeferredFace.zig` — 浏览器产品：`fontconfig`； `coretext`。
- `src/font/Glyph.zig` — 涵盖 (`E-FONT`)：`Constraints`。
- `src/font/Metrics.zig` — 应用层：`formatConfig percent`； `formatConfig absolute`; `Modifier: parse absolute`; `Modifier: parse percent`。
- `src/font/Metrics.zig` — 覆盖 (`E-FONT`)：`Metrics: apply modifiers`； `Metrics: adjust cell height smaller`; `Metrics: adjust cell height larger`; `Metrics: adjust icon height by percentage`; `Metrics: adjust icon height by absolute pixels`; `Modifier: percent`； `Modifier: absolute`。
- `src/font/SharedGrid.zig` — 覆盖 (`E-FONT`)：`getIndex`； `renderGlyph error after cache insert rolls back cache entry`。
- `src/font/SharedGrid.zig` — 上游内部：`init error`。
- `src/font/SharedGridSet.zig` — 覆盖 (`E-FONT`)：`Key`； `Key different font points`; `Key different font DPI`。
- `src/font/SharedGridSet.zig` — 上游内部：`SharedGridSet`。
- `src/font/discovery.zig` — 浏览器产品：`descriptor hash`； `descriptor hash family names`; `fontconfig`； `fontconfig codepoint`； `coretext`; `coretext codepoint`； `coretext sorting`； `windows`。
- `src/font/face.zig` — 上游内部：`<unnamed@96>`。
- `src/font/face.zig` — 浏览器产品：`Variation.Id: wght should be 2003265652`； `Variation.Id: slnt should be 1936486004`；数字常规/粗体和斜体样式使用 CSS 字体选择，因此浏览器拥有可变字体轴标识符。任意轴保留在完整的八字段表示事务之外。
- `src/font/face/coretext.zig` — 浏览器产品：`<unnamed@971>`； `name`； `emoji`; `in-memory`; `variable`； `variable set variation`; `svg font table`； `glyphIndex colored vs text`。
- `src/font/face/freetype.zig` — 浏览器产品：`face name`； `<unnamed@1112>`; `color emoji`； `mono to bgra`； `svg font table`； `bitmap glyph`； `bitmap glyph BDF`; `bitmap glyph PCF`； `bitmap glyph OTB`。
- `src/font/glyf_rasterize.zig` — 浏览器产品：`<unnamed@459>`； `glyf_rasterize: empty outline returns empty bitmap`； `glyf_rasterize: square fills bitmap center`； `glyf_rasterize: quadratic contour renders`； `glyf_rasterize: consecutive off-curve points render`; `glyf_rasterize: units per em controls baseline scale`; `glyf_rasterize: degenerate outline returns full empty bitmap`; `glyf_rasterize: contour can start off curve with final on curve point`； `glyf_rasterize: contour can start with implied midpoint`； `glyf_rasterize: multiple contours render independently`；浏览器字体光栅化取代了 Ghostty 的原生轮廓光栅化器。
- `src/font/glyf_rasterize.zig` — 覆盖 (`E-FONT`)：`glyf_rasterize: non-zero bearings preserve declared whitespace`； `glyf_rasterize: negative y coordinates descend below baseline`; `glyf_rasterize: two-cell bitmap and constraint render within width`； `glyf_rasterize: line height does not change unconstrained em scale`；带符号的 Canvas 墨水边界、终端基线和 Ghostty 派生的宽度约束保留相应的投影几何图形。
- `src/font/glyf_rasterize_png_test.zig` — 间隙：`glyf_rasterize: bubbletea glyph protocol examples match reference image`； Ghostty 字形图形不由主机投影格式表示，并且客户端渲染器仍然不受支持。
- `src/font/main.zig` — 上游内部：`<unnamed@75>`。
- `src/font/opentype.zig` — 上游内部：`<unnamed@17>`。
- `src/font/opentype/glyf.zig` — 浏览器产品：`glyf`； `glyf: decode triangle`； `glyf: decode multiple contours`； `glyf: decode repeat and short vector flags`； `glyf: decode off curve and same coordinate flags`; `glyf: decode one-point contour`; `glyf: decode contour ending at max point index`; `glyf: reject glyphs with instructions and composite glyphs`； `glyf: reject truncated`； `glyf: reject endpoints out of order`; `glyf: reject too many points`； `glyf: zero-contour glyph can be header-only`； `glyf: zero-contour glyph can include instruction length`； `glyf: zero-contour glyph rejects instructions`。
- `src/font/opentype/head.zig` — 浏览器产品：`head`。
- `src/font/opentype/hhea.zig` — 浏览器产品：`hhea`。
- `src/font/opentype/os2.zig` — 浏览器产品：`OS/2`。
- `src/font/opentype/post.zig` — 浏览器产品：`post`。
- `src/font/opentype/sfnt.zig` — 浏览器产品：`FixedPoint`； `parse font`； `get table`。
- `src/font/opentype/svg.zig` — 浏览器产品：`SVG`； `SVG document list offset past end`；浏览器字体加载与验证替代 Ghostty 的本机 SVG table parser。
- `src/font/shape.zig` — 上游内部：`<unnamed@96>`。
- `src/font/shaper/Cache.zig` — 覆盖 (`E-FONT`)：`Cache`。
- `src/font/shaper/coretext.zig` — 覆盖（`E-SHAPE`、`E-FONT`）：`shape inconsolata ligs`； `shape monaspace ligs`； `shape emoji width`； `shape emoji width long`; `shape Combining characters`； `shape selection boundary`; `shape cursor boundary`; `shape cursor boundary and colored emoji`; `shape cell attribute change`; `shape nerd fonts`。
- `src/font/shaper/coretext.zig` — 浏览器产品：`shape`； `shape U+3C9 with JB Mono`; `shape variation selector VS15`; `shape variation selector VS16`。
- `src/font/shaper/coretext.zig` — 覆盖 (`E-SHAPE`)：`run iterator`； `run iterator: empty cells with background set`; `shape left-replaced lig in last run`; `shape left-replaced lig in early run`; `shape with empty cells in between`;完整的行边界样式运行保留空单元格，并在选择、光标、预编辑、隐藏、约束符号、彩色字形和精灵边界处停止。
- `src/font/shaper/coretext.zig` — 浏览器产品：`shape Devanagari string`； `shape Tai Tham vowels (position differs from advance)`; `shape Tai Tham letters (position.y differs from advance)`; `shape Javanese ligatures`; `shape Chakma vowel sign with ligature (vowel sign renders first)`; `shape Bengali ligatures with out of order vowels`;画布塑造完整的 forced-LTR 运行，而不是复制 CoreText 字形位置。
- `src/font/shaper/coretext.zig` — 覆盖 (`E-FONT`)：`shape box glyphs`； `shape high plane sprite font codepoint`;完整固定的 Unicode sprite-face 解析器绕过浏览器字体回退。
- `src/font/shaper/feature.zig` — 应用层：`Feature.fromString`； `FeatureList.fromString`;接受的演示事务公开一种字体连字策略，而不是任意 OpenType 特征字符串。
- `src/font/shaper/harfbuzz.zig` — 覆盖 (`E-SHAPE`)：`shape inconsolata ligs`； `shape monaspace ligs`; `shape emoji width`; `shape emoji width long`； `shape Combining characters`； `shape selection boundary`; `shape cursor boundary`； `shape cursor boundary and colored emoji`； `shape cell attribute change`。
- `src/font/shaper/harfbuzz.zig` — 浏览器产品：`shape`； `shape variation selector VS15`; `shape variation selector VS16`。
- `src/font/shaper/harfbuzz.zig` — 覆盖 (`E-SHAPE`)：`run iterator`； `run iterator: empty cells with background set`; `shape arabic forced LTR`; `shape with empty cells in between`;完整的行边界样式运行是从左到右显式光栅化的。
- `src/font/shaper/harfbuzz.zig` — 覆盖（`E-SHAPE`、`E-FONT`）：`shaper selects font for entire grapheme`；每个 Host 投影字素跨浏览器字体回退保持为一个 shaping 单元。
- `src/font/shaper/harfbuzz.zig` — 浏览器产品：`shape Devanagari string`； `shape Tai Tham vowels (position differs from advance)`; `shape Tibetan characters`; `shape Tai Tham letters (run_offset.y differs from zero)`; `shape Javanese ligatures`; `shape Chakma vowel sign with ligature (vowel sign renders first)`; `shape Bengali ligatures with out of order vowels`; Canvas 取代了有界运行内的 HarfBuzz 字形放置。
- `src/font/shaper/harfbuzz.zig` — 覆盖 (`E-FONT`)：`shape box glyphs`；完整固定的 Unicode sprite-face 解析器绕过浏览器字体回退。
- `src/font/shaper/noop.zig` — 上游内部：`<unnamed@141>`。
- `src/font/sprite.zig` — 上游内部：`<unnamed@37>`； `<unnamed@43>`。
- `src/font/sprite/Face.zig` — 覆盖 (`E-FONT`)：`sprite face render all sprites`；确定性生成的数据携带每个声明的 Unicode 精灵和所有四个完整的填充奇数/偶数单元大小和厚度参考栅格，而选定的框、块、三角形和电力线形状在参考几何图形之间保留其程序渲染器。
- `src/font/sprite/Face.zig` — 覆盖 (`E-RENDER`)：`full height cursor sprites respect cursor height metric`；光标几何形状由渲染器拥有，并使用调整后的光标高度度量而不是 Unicode 精灵数据。
- `src/font/sprite/Face.zig` — 上游内部：`<unnamed@666>`；编译时声明遍历没有浏览器类似的功能。

## 维持限制

- Shadertoy 和自定义着色器翻译未公开。
- 任意字体变化和 OpenType 功能配置均未公开。
- 主机投影格式中不存在字形图形。
- 一般复杂脚本字形放置委托给浏览器文本光栅化； Orchbee 拥有完整的行边界样式运行、终端边界、空单元保留和 forced-LTR 方向。
- 固定的 Ghostty 范围的 Unicode 精灵覆盖范围已完成。为四个上游度量组合生成精确的填充参考像素和悬垂偏移；其他物理几何形状选择最近的参考并对其重新采样，而选定的程序形状继续直接缩放。

Kitty 协议解析、VT 行为与终端查询响应不在此 renderer 审核范围内；上述有界 Host-to-client 图形投影与合成在范围内。未映射的本机行为不能作为 Terminal Host 行为的证据。

## 升级验证

1. 将目标 Ghostty 提交与 Terminal Host 引脚进行比较。
2. 在 `../ghostty` 中准备一个位于 Terminal Host 精确 pin 且工作树干净的 Ghostty checkout，然后运行 `pnpm audit:ghostty-panel`；也可将 `GHOSTTY_SOURCE` 指向该 checkout 根目录。该命令会拒绝其他 HEAD，以及任何已暂存、未暂存或未跟踪的源码变更。
3. 当上游 Nerd 字体表、精灵范围或精灵参考栅格更改时运行 `pnpm generate:ghostty-panel-data`。
4. 检查每一项更改的上游声明并准确更新一个清单配置。
5. 在标记适用的声明涵盖之前添加或修改投影渲染器证据。
6. 运行终端包检查与仓库合并门。
