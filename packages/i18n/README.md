# @orchbee/i18n

Locale contracts, formatting runtime, and shipped product translations for Orchbee interfaces.

## React composition

Applications resolve their ordered platform preferences and install one provider above all product
presentation:

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

Feature presentation imports its domain descriptors and formatter from this package:

```tsx
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type * as React from 'react';

export function TerminalActionsLabel(): React.ReactNode {
	const intl = useOrchbeeIntl();
	return intl.formatMessage(workbenchMessages.terminalActionsLabel);
}
```

## Non-React formatting

Adapters that produce product-authored text use the same locale and catalogs:

```ts
import { createOrchbeeIntl, type OrchbeeLocale, webMessages } from '@orchbee/i18n';

declare const locale: OrchbeeLocale;

const intl = createOrchbeeIntl(locale);
const title = intl.formatMessage(webMessages.newSessionTitle, { sessionNumber: 3 });
```

Applications own locale preference, detection, persistence, and document synchronization. This
package owns locale normalization, formatting, message descriptors, and translations; it does not
read platform globals or mutate document state.

## Catalog structure

Product copy is physically separated by locale and mirrored by product domain:

```text
src/locales/
├── en-US/
│   ├── web.ts
│   └── workbench.ts
└── zh-CN/
    ├── web.ts
    └── workbench.ts
```

English and Simplified Chinese values never share an authored file. Directory names use the
canonical BCP 47 identifier and casing. English descriptors reference the corresponding `en-US`
catalog values as their `defaultMessage`, while catalog tests enforce
message-ID and ICU-variable parity across every shipped locale.

## Development

```sh
pnpm --filter @orchbee/i18n build
pnpm --filter @orchbee/i18n typecheck
pnpm --filter @orchbee/i18n test
pnpm --filter @orchbee/i18n test:exports
```

The authoring, fallback, and verification contract is defined in
[`docs/frontend/localization.md`](../../docs/frontend/localization.md).
