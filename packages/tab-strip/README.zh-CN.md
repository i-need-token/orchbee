# @orchbee/tab-strip

完整的可重复使用 React 标签条功能。该包将公共组件、其域模型、布局规则、可访问性投影、拖动交互以及在一个包条目后面递归拥有的私有组件树组合在一起。它是一个 Rslib 库，而不是一个独立的站点。

## 适用范围

该包拥有选项卡和组转换、选择不变式、固定和分组排序、水平和垂直布局、拖动几何图形、可访问性投影、悬停卡、菜单和确定性 React 事件集成。消费者拥有应用程序组合、终端会话、导航、持久性、产品本地化、主题令牌值以及平台或跨窗口主机实现。

## 切入点

```ts
import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { TabModel, TabStrip, TabStripOrientation, type TabStripMessages } from '@orchbee/tab-strip';
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
```

- `@orchbee/tab-strip` 公开组件以及创建和托管它所需的选定合约。
- `@orchbee/ui/styles.css` 提供上下文菜单使用的共享原始样式和设计令牌注册表。
- `@orchbee/tab-strip/styles.css` 为内置消费者提供组件的 CSS 模块输出。

产品 Host 可以提供完整 `TabStripMessages` 契约，以本地化可见控件、菜单、无障碍名称与 live-region 公告。内置英文消息仍是封装测试和非产品夹具的参考后备；Orchbee 产品表面由 `@orchbee/i18n` 提供契约。

产品成分通过其安装的 `ActionRuntime` 和母体 `ActionScope`。选项卡条注册一个子范围，其操作拥有选择、创建、关闭、分组、固定、首选项和提交的移动语义。按钮、键盘交互和上下文菜单调用相同的操作。独立的装置可能会省略运行时；然后，该组件拥有并处置一个对应用程序命令界面不可见的隔离后备运行时。

产品组合可以为“新建 Tab”和上下文“关闭”提供 `shortcutLabels`。上下文菜单只会在共享的尾部快捷键槽位渲染非空、按平台格式化的当前绑定；Tab Strip 不会为没有绑定的命令虚构快捷键。

本地合成通过`TabModel`同步关闭已知Tab IDs。相反，终端拥有的组合提供 `onCloseTabs`。它还可以提供 `getCloseTabsConfirmation`，这使得所有者可以从受验证的选项卡集影响的完整资源中获得保护，同时选项卡条在同一 `tabs.tab.close` 操作内保持确认。 Action 委托其经过验证的 ID 集和执行上下文，而不改变 `TabModel`；只有在终端关闭编排成功后，所有者才会标记提交点并删除选项卡。因此，取消的确认、挂起的关闭或部分失败的关闭无法使选项卡条尽早删除拓扑。

URL 具有权威性的组合物可以提供`onSelectTab`。 `tabs.tab.select` Action 在任何本地 `TabModel` 写入之前委托其经过验证的 `TabSelectionInput` 和执行上下文；所有者提交导航，然后在观察到该路线时应用选择。省略回调会保留包拥有的本地选择转换。此端口不会将路由、历史记录或 URL 解析移动到选项卡条中。

指针移动和拖动几何体保持本地手势状态。拖动期间显示的模型更改是可逆预览；接受本地放置会回滚预览并调用 `tabs.layout.apply-arrangement` 以通过 Action 生命周期提交最终安排。

水平主机可以为必须共享选项卡条表面的控件提供 `horizontalLeadingActions`。选项卡轨道仅消耗剩余宽度，因此托管控件永远不会与第一个选项卡重叠或需要单独绘制的同级区域。横向布局为标签条、项目与拖动边界统一采用 `36px`；`28px` 的新建标签控件以及 `20px` 的分组芯片和分隔线均在该行内垂直居中。纵向几何保持独立。

支持两种方向的主机都会传递受控的 `orientation` 值和 `onOrientationChange`。右键单击空白条空间，然后显示打开或关闭垂直选项卡的本地化操作；选项卡、组和新选项卡控件保留自己的上下文菜单。

包 API 故意没有 React 或浏览器供应商子路径。桌面组合从包根导入跨窗口合约：`CrossWindowTabDragRuntime` 拥有模型切换、取消恢复、附加模型活性和目标布局排序； `DomTabDragRegistry` 适应主机提供的实时边界、z 顺序、活性和兼容性决策。 `TabStrip` 接受 `createCrossWindowDragRuntime` 工厂，而本机窗口创建、移动、捕获、移动循环和源窗口处理仍然是主机职责。浏览器应用程序不会综合这些操作系统功能。上游来源归属和许可义务记录在 [`NOTICE.reference`](NOTICE.reference) 中。

## 来源所有权

```text
src/
├── index.ts
├── styles.ts
└── tab-strip/
    ├── index.ts
    ├── TabStrip.tsx
    ├── components/
    │   └── tab-strip-view/
    │       ├── TabStripView.tsx
    │       └── components/
    │           ├── horizontal-contents/
    │           ├── vertical-contents/
    │           ├── tab-button/
    │           ├── group-header/
    │           ├── drag-ghost/
    │           ├── hover-card/
    │           └── tab-strip-context-menu/
    ├── model/
    ├── layout/
    ├── dragging/
    ├── accessibility/
    ├── contexts/
    ├── hooks/
    ├── constants/
    ├── types/
    └── utils/
```

`TabStrip.tsx` 是窄公共适配器。 `TabStripView` 组成准备好的水平、垂直、覆盖和菜单区域。私有组件递归地重复相同的所有权结构，并且没有 barrel。四个语义子系统是这个完整组件功能的内聚方面，而不是单独使用的源模块；选定的公共合约仅通过 `tab-strip/index.ts` 和包根重新导出。

## 正确性

一般的 `TabModel` 状态机具有三个互补层：

- 确定性回归保留每个已知的最小反例；
- 有界状态空间探索枚举了来自规范分组种子的深度三的每个建模选择、关闭、固定、组和键盘移动转换（15,497 个转换和 952 个不同状态）；
- 固定种子特性测试执行 200 个可重复的动作序列，最多 100 个转换。

独立的测试不变检查器会在每次转换后验证选项卡和组身份、固定前缀、连续组成员身份、活动和选定身份、选择锚点以及最近的组引用。生产模型断言相同的结构边界，因此无效的恢复状态和无效的中间突变在其源头失败。

水平组拖动还包含 TLA+ 模型、连续块移动的 Lean 证明、针对性的属性测试和确定性浏览器跟踪。请参阅 [`horizontal group-drag contract`](docs/horizontal-group-drag.md) 和 [`formal/README.md`](formal/README.md)。

```sh
pnpm --filter @orchbee/tab-strip typecheck
pnpm --filter @orchbee/tab-strip test
pnpm --filter @orchbee/tab-strip verify:correctness
pnpm --filter @orchbee/tab-strip test:exports
```
