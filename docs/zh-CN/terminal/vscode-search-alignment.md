# VS Code 终端搜索对齐

## 产品契约

Orchbee 终端搜索遵循 VS Code 集成终端的查找模型：

| 行为        | Orchbee 契约                                                                                                                                   |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| 增量输入    | `Terminal.updateSearch()` 采用 200 ms trailing-edge debounce。更新的查询会取消等待中的查询。                                                   |
| 即时导航    | `findNext()` 向前搜索，`findPrevious()` 向后搜索；二者都会循环，并取消等待中的增量输入。                                                       |
| 搜索模式    | 支持字面量、区分大小写、全词和 JavaScript 正则表达式模式。无效正则表达式返回空结果和 `regexError`，而不是异步异常。                            |
| 匹配枚举    | 匹配不重叠，并按完整匹配前进，与 VS Code 的 xterm 搜索 decoration 遍历一致。                                                                   |
| 结果状态    | `onSearchResults` 报告有界集合的活动索引和数量、查询、正则验证，以及是否达到结果上限。超出上限的活动匹配索引为 `-1`。                          |
| 高亮上限    | 默认最多收集 20,000 个匹配的 decoration 和结果计数，与 VS Code 集成终端高亮上限一致。方向导航仍可到达之后的匹配。`maxResults` 只降低收集上限。 |
| Buffer 变更 | 终端输出和大小调整会立即清除过期 decoration，并在相同 200 ms debounce 后重新开始最新查询，不滚动。                                             |
| 空输入      | 空增量查询会立即清除 decoration 和终端搜索选择。                                                                                               |

应用查找 widget 拥有 `Ctrl+F` 焦点、从单行选择填充输入框、大小写/全词/正则开关、结果计数展示、Enter/Shift+Enter 导航、按 Escape 关闭、搜索历史和无障碍标签。终端包拥有有界匹配、选择导航、decoration 状态、结果事件和 debounce 保证。

`highlightSearch()` 和 `findAll()` 仍然是面向编程使用方的即时底层 API。交互式搜索输入使用 `updateSearch()`，使调用方无法意外在每次按键时重新扫描 scrollback。
