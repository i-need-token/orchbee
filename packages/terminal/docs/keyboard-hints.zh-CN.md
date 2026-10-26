# 键盘提示

## 范围

一个语义 Action 通过键盘标签与前缀筛选选择可见 URL、路径、文件位置和 hash，支持取消、打开 URL 及精确文本插入。不包含可配置匹配器、自定义处理器、多选、鼠标选择、剪贴板模式、编辑器启动或任意程序。

## 领域契约

`ProjectedTerminal` 在已挂载的可见视口上拥有一个 Hint session。只有终端可接收输入且具有
实时渲染界面时，session 才可用。它通过现有有界行 API 读取一个固定 generation/revision 的
可见行窗口，排除被任一视口边缘截断的软换行逻辑行，并按屏幕顺序最多检测 512 个目标：

- 允许的 `http`、`https` 与 `mailto` URL，包括已准入的显式 hyperlink；
- 检测到或显式提供的路径与文件名；
- 检测到的 `path:line` 与 `path:line:column` 位置；以及
- 长度为 7 到 64 个字符的小写十六进制 hash。

URL 与文件匹配使用共享终端链接检测器，因此显式 metadata 与更长的文件位置保留优先级，形似
hash 的子串不会重复另一个已准入目标。标签按屏幕顺序生成等宽、无前缀歧义的 base-36 字符串。
检测不会读取当前视口以外内容、同步检查完整缓冲区、测试文件系统是否存在，也不会向终端包授予
外部效果 capability。

## Session 与效果模型

session 从 `loading` 转换到 `active`、`empty`、`stale`、`unavailable` 或 `cancelled` 之一。
活动期间，它拥有 key-down 与 key-up 投递，因此标签输入绝不会到达 PTY。字符会缩小可见前缀，
Backspace 删除一个前缀字符，Escape 取消，Enter 或 Space 选择唯一剩余目标。IME composition
会取消且不消费该 composition event。

Frame 或行修订、视口转换、resize、Presentation 几何变化、焦点丢失、界面释放、处置、替换调用
或调用方取消都会退役 session。只有选中结果会越过应用拥有的 Action 边界：

| 目标类型              | 效果                                          |
| --------------------- | --------------------------------------------- |
| 允许的 URL            | 再次规范化，并请求应用拥有的外部 URL opener   |
| 路径、文件位置与 hash | 通过普通终端 paste API 插入可见匹配的精确文本 |

插入绝不附加 Enter，也不会用路径检测器解析出的绝对目标替换可见写法。已准入效果一旦报告成功，就会
越过 Action commit point；之后的替换不能把这个已完成效果重新标记为取消。

## Action、绑定与持久化

Workbench 拥有 `terminal.hints.open`；Web 注册投影终端实现，并把它投影到终端上下文菜单、
Action 搜索、快捷键 Settings 与本地化结果反馈。Apple 和标准平台默认均为 `Mod+Shift+O`。
标准平台 `Ctrl+Shift+E` 已由向下拆分拥有；当前快捷键目录为每个 Action 支持一个有效绑定。

增加该 Action 会把 Web 的严格设备本地快捷键记录从版本 8 扩展到版本 9。版本 9 reader 会迁移
精确的 v8 目录与默认值，拒绝与新默认值的冲突，只写入规范 v9 记录，并原样保留 v8 作为回滚快照。

## 验证

| 声明                                     | 证据                                                           | 限制                                           |
| ---------------------------------------- | -------------------------------------------------------------- | ---------------------------------------------- |
| 检测、重叠优先级、裁剪、边界与前缀标签   | `src/__tests__/terminal-hints.unit.test.ts`                    | 合成渲染行                                     |
| 固定可见读取、overlay 所有权、选择与失效 | `src/projected/__tests__/projected-terminal.unit.test.ts`      | 确定性投影终端与模拟 Host 行窗口               |
| 效果、取消、提交、绑定、标签与 v8 迁移   | Web Action、快捷键、Settings、i18n、controller 与 storage 测试 | 进程内效果、键盘事件与设备本地存储             |
| 四种目标通过生产 Canvas 界面工作         | `terminal-parity/FS-07-keyboard-hints`                         | 使用合成固定行与捕获 opener 的 headed Chromium |

<!-- documentation-pairing: english-sha256=60b01805058fd1e9e1a2af0a2dd00dd2872d665eb64a5dd124379a944f8c1965 zh-cn-sha256=6120b48b4c4fd6bcc311c1310cf1532be6e75d154d2cb6a0b83177b7d0adff45 -->
