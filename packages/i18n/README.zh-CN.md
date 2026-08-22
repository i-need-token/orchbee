# @orchbee/i18n

Orchbee 界面的 locale 契约、格式化运行时和已交付产品翻译。

## React 组合

应用解析有序的平台语言偏好，并在所有产品界面之上挂载一个 Provider：

```tsx
import { getOrchbeeTextDirection, OrchbeeI18nProvider, resolveOrchbeeLocale } from '@orchbee/i18n';
import type * as React from 'react';
import { createRoot } from 'react-dom/client';

declare const product: React.ReactNode;

const locale = resolveOrchbeeLocale(navigator.languages);
document.documentElement.lang = locale;
document.documentElement.dir = getOrchbeeTextDirection(locale);

createRoot(document.body).render(
	<OrchbeeI18nProvider locale={locale}>{product}</OrchbeeI18nProvider>,
);
```

## 非 React 格式化

非 React 使用方使用同一套 locale 格式化器：

```ts
import { createOrchbeeIntl, type OrchbeeLocale } from '@orchbee/i18n';

declare const locale: OrchbeeLocale;

const intl = createOrchbeeIntl(locale);
const formattedNumber = intl.formatNumber(1234);
```

应用负责 locale 偏好、检测、持久化和文档同步。该包负责 locale 规范化、格式化、消息描述符和翻译；它不会读取平台全局变量，也不会修改文档状态。

## 目录结构

产品文案按 locale 在物理目录中分隔，并按产品领域保持镜像：

```text
src/locales/
├── en-US/
│   ├── application-routing.ts
│   ├── desktop-application-lifetime.ts
│   ├── desktop-window.ts
│   ├── index.ts
│   ├── keyboard-shortcuts.ts
│   ├── messages.ts
│   ├── projected-terminal.ts
│   ├── terminal-settings-catalog.ts
│   ├── terminal-settings.ts
│   └── workbench.ts
└── zh-CN/
	├── application-routing.ts
	├── desktop-application-lifetime.ts
	├── desktop-window.ts
	├── index.ts
	├── keyboard-shortcuts.ts
	├── messages.ts
	├── projected-terminal.ts
	├── terminal-settings-catalog.ts
	├── terminal-settings.ts
	└── workbench.ts
```

应用路由领域拥有地址不可用和导航恢复文案。Workbench 领域拥有共享导航与布局文案。投影终端领域拥有与传输无关的投影终端、查找、选择、复制和链接体验文案。终端设置领域拥有覆盖行为、持久化、Ghostty 迁移报告和 Browser 访问入口文案；终端设置目录拥有稳定导航分类和明确不可交互的未来设置预览。键盘快捷键领域拥有快捷键录制、作用域、来源、冲突和本地持久化文案。Desktop 应用生命周期领域拥有 Electron 专属的退出和恢复文案；Desktop 窗口领域拥有原生窗口状态 Action 及其可用性说明。每个新增产品领域都要同时加入两个 locale 目录，并在同一变更中导出匹配的描述符。运行时英文和简体中文目录值绝不共享创作文件。目录名使用规范的 BCP 47 标识符与大小写；目录测试强制每个已交付 locale 的 message ID 和 ICU 变量保持一致。

大型类型目录通过其域组公开描述符，而不将翻译值移动到特征代码中：

```ts
import { terminalSettingsCatalogMessages, useOrchbeeIntl } from '@orchbee/i18n';

const intl = useOrchbeeIntl();
const label = intl.formatMessage(
	terminalSettingsCatalogMessages.get('terminal.settings.catalog.categories.general.title'),
);
```

## 产品语言

[`docs/product-language.md`](docs/product-language.md) 拥有已批准的 `en-US` 和 `zh-CN` 术语映射以及消息创作和审核期间使用的重复交互标签。目录对于完整的运行时消息仍然具有权威性。当批准的产品语言决策发生变化时，一起更新参考和每个受影响的区域设置目录。

## 开发

```sh
pnpm --filter @orchbee/i18n build
pnpm --filter @orchbee/i18n typecheck
pnpm --filter @orchbee/i18n test
pnpm --filter @orchbee/i18n test:exports
```

创作、回退和验证契约见 [`docs/frontend/localization.md`](../../docs/frontend/localization.md)。
