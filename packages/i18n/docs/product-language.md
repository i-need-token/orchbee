# Product Language

## Authority and use

This reference owns approved product terminology and repeated interaction copy for every shipped locale. Runtime catalogs remain authoritative for complete messages; this reference constrains word choice and does not replace descriptors, translator context, or locale files.

Before adding or changing product copy:

1. Identify the product concept or user action, then use the approved term or action label below.
2. Search the affected catalog and neighboring interface for competing wording.
3. Add a term only when a concrete product concept recurs or could be translated inconsistently.
4. Add an interaction rule only when a repeated product scenario has multiple plausible labels.
5. Update this reference and every affected locale catalog in the same change when an approved decision changes.

Grammar may vary with a complete message, but the underlying product concept and action meaning remain stable. Matching visible text does not justify reusing an unrelated message ID.

## Product terminology

| Concept               | `en-US`               | `zh-CN`      | Use                                                                    | Avoid as synonyms                          |
| --------------------- | --------------------- | ------------ | ---------------------------------------------------------------------- | ------------------------------------------ |
| Orchbee               | Orchbee               | Orchbee      | Product name                                                           | Transliterated or translated product names |
| Browser access        | Browser access        | 浏览器访问   | Desktop surface for using a terminal from a browser                    | Web access、浏览器连接                     |
| Built-in colors       | Built-in colors       | 内置颜色     | Terminal colors option `host`; in the GUI it paints the theme's colors | Terminal default、Host、终端默认、主机     |
| Color theme name      | Gruvbox Dark          | Gruvbox Dark | Published name of a third-party color theme; never translated          | Translated names such as Gruvbox 深色      |
| Command output        | Command output        | 命令输出     | Shell-marked text produced by one command                              | Terminal output、log                       |
| Keyboard shortcut     | Keyboard shortcut     | 键盘快捷键   | Physical chord bound to one semantic product action                    | Hotkey、快捷方式                           |
| Personal Access Token | Personal Access Token | 个人访问令牌 | Installation-scoped secret used to sign in to Browser access           | Access token、API token、personal token    |
| Planned               | Planned               | 规划中       | Visible future setting that is explicitly unavailable and inert        | Coming soon、即将推出、未来                |
| Project               | Project               | 项目         | Optional named organization of independent Workspaces                  | 工程                                       |
| Workspace             | Workspace             | 工作区       | Independent terminal context, optionally grouped in a Project          | Session、会话                              |
| Settings              | Settings              | 设置         | Product configuration entry or surface                                 | 偏好设置、配置                             |
| Sidebar               | Sidebar               | 边栏         | Supporting navigation or tool surface at an edge                       | 侧边栏、侧栏                               |
| Tab                   | Tab                   | 标签页       | Selectable item and its associated content surface                     | 选项卡、页签                               |
| Tab group             | Tab group             | 标签页组     | Collection that owns one or more tabs                                  | 标签组、分组                               |
| Tab strip             | Tab strip             | 标签栏       | Container that presents and manages tabs                               | 标签页栏、选项卡栏                         |
| Terminal              | Terminal              | 终端         | Terminal session or terminal surface                                   | 控制台、命令行                             |
| Terminal session      | Terminal session      | 终端会话     | One running shell/process lifetime owned by a Terminal presentation    | 控制台会话、命令行会话                     |
| Window                | Window                | 窗口         | Native Desktop application window                                      | 页面、视图                                 |

Do not add general language merely to enlarge the table. Record only product terms whose stable translation prevents a real ambiguity.

## Interaction actions

Prefer a specific action verb over a generic confirmation label. A form inside a dialog does not make its primary action `Submit`; name the result of the action.

| Intent                        | `en-US` | `zh-CN` | Use                                                                | Do not substitute                                  |
| ----------------------------- | ------- | ------- | ------------------------------------------------------------------ | -------------------------------------------------- |
| Create a durable entity       | Create  | 创建    | A new named entity exists after success                            | Submit, OK, Confirm                                |
| Persist edits                 | Save    | 保存    | Current edits become durable                                       | Submit, Confirm                                    |
| Add to a collection           | Add     | 添加    | An item is attached or inserted without implying a new entity      | Create, Submit                                     |
| Send for processing           | Submit  | 提交    | A request, workflow, or approval enters downstream processing      | Save, OK; do not use for every dialog form         |
| Apply a choice                | Apply   | 应用    | A setting or selection takes effect                                | Submit                                             |
| Confirm a stated decision     | Confirm | 确认    | The interaction explicitly asks the user to confirm a prior choice | Generic form completion; use a specific verb first |
| Acknowledge information       | OK      | 确定    | The dialog has no domain action or data mutation                   | Submit, Save                                       |
| Abort an unfinished operation | Cancel  | 取消    | Dismiss without committing the pending operation                   | Close                                              |
| Dismiss a completed/read view | Close   | 关闭    | Leave an informational or completed surface                        | Cancel                                             |
| Destroy an owned entity       | Delete  | 删除    | The entity is destroyed                                            | Remove, Confirm                                    |
| Detach without destroying     | Remove  | 移除    | Membership or association ends while the entity remains            | Delete                                             |
| Repeat a failed operation     | Retry   | 重试    | Attempt the same failed operation again                            | Submit                                             |

- A destructive confirmation uses the specific destructive verb such as `Delete`, not `OK` or `Confirm`.
- Paired state actions use stable opposites, such as Show/Hide, Expand/Collapse, Pin/Unpin, and Add/Remove.
- Button labels are concise action phrases without terminal punctuation. Explanations, consequences, and recovery guidance belong in the surrounding message.
- Reuse a label consistently within one interaction, including its button, menu item, tooltip, notification, and accessibility text, while keeping separately owned message IDs when their semantics differ.

Use **Close Workspace** for closing one Workspace and its terminals, **Ungroup** for removing only a Project while retaining its Workspaces, and **Close All Workspaces** for the separate explicit group-wide close. **New Workspace** creates immediately without mandatory Project setup. Default names use an available localized base name with numeric suffixes; explicit user names stay unchanged.

## Names shared with the TUI

A person who uses Windows Terminal, iTerm2, Ghostty or the VS Code terminal every day must understand every name without knowing Orchbee's internals. These names are shared with the TUI; identifiers, action ids and wire names keep their own spelling.

| Concept                                           | `en-US`                        | `zh-CN`                  | Avoid                      |
| ------------------------------------------------- | ------------------------------ | ------------------------ | -------------------------- |
| Make this window the one that types and sizes     | Type here                      | 在这里输入               | Take control、接管         |
| This window shows the terminal but does not type  | View only                      | 仅查看                   | Read-only                  |
| Another window holds the terminal                 | Another window is typing here. | 另一个窗口正在这里输入。 | Client, controller、客户端 |
| A view smaller than the terminal                  | Only part is shown             | 只显示了一部分           | Live crop、实时裁切        |
| A Panel's first terminal                          | Main terminal                  | 主终端                   | Primary terminal           |
| A Panel's further terminals                       | Terminal N                     | 终端 N                   | Child terminal、子终端     |
| Fill the Tab with one Panel, and undo it          | Maximize / Restore             | 最大化 / 还原            | Toggle maximization, zoom  |
| Remove a Project, keep its Workspaces             | Ungroup                        | 取消分组                 | Dissolve、解散             |
| Sidebar heading over Workspaces without a Project | Workspaces                     | 工作区                   | Recents、最近              |
| An Agent whose state Orchbee cannot read          | Status unknown                 | 状态未知                 | Limited status、状态受限   |
| Split the Panel downward                          | Split down                     | 向下拆分                 | Split below、向下分割      |

"Client" never appears in product copy; say window (窗口). In notices, Worker, Terminal Host and Server are Orchbee.

A grey row says why only when the fix is not obvious, in one form each: **Terminal disconnected** (终端已断开), **Not in a Git repository** (不在 Git 仓库里), and **Select text first** (请先选中文本). Every other disabled row is simply grey, with no reason and no short form.

## Status sentences

A notice says one of three things, the same way in both clients:

- A failure is one plain sentence: **Couldn't {action} {object}.** (没能{动作}{对象}。), such as **Couldn't rename the Tab.** (没能重命名标签页。). A short reason or the place in the interface that fixes it may follow, never a reason code, an id, a revision or another internal noun.
- An unknown outcome is exactly **Not sure it finished. Check, then try again.** (不确定是否完成，请先看一下再重试。). A close that is already settled as not done keeps **Couldn't close {label}.** (没能关闭 {label}。).
- A success says nothing: its result shows on screen, or it has none to show.

No notice tells a person to run an `orchbee` command, except a configuration file syntax error, which keeps `orchbee config check`. Troubleshooting detail goes to the existing diagnostic log, never to a new surface.

## Review

For every affected product message:

- Verify its terms and action meaning against this reference.
- Check sibling controls and the same workflow for competing synonyms.
- Supply translator context when placement, object, tone, or consequence is not obvious.
- Review `en-US` and `zh-CN` together for semantic equivalence rather than literal word substitution.
- Update this reference only for a recurring decision; one complete context-specific sentence remains in its owning catalog.
