# @orchbee/i18n

Locale contracts, formatting runtime, and shipped product translations for Orchbee interfaces.

## React composition

Applications resolve their ordered platform preferences and install one provider above all product presentation:

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

## Non-React formatting

Non-React consumers use the same locale-aware formatter:

```ts
import { createOrchbeeIntl, type OrchbeeLocale } from '@orchbee/i18n';

declare const locale: OrchbeeLocale;

const intl = createOrchbeeIntl(locale);
const formattedNumber = intl.formatNumber(1234);
```

Applications own locale preference, detection, persistence, and document synchronization. This package owns locale normalization, formatting, message descriptors, and translations; it does not read platform globals or mutate document state.

The `websiteMessages` descriptors and mirrored `website.ts` locale catalogs own static product-site copy, including metadata and accessible control names.

## Catalog structure

Product copy is physically separated by locale and mirrored by product domain:

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

The application-routing domain owns unavailable-address and navigation-recovery copy. The Workbench domain owns shared navigation and layout copy. The projected-terminal domain owns the transport-independent projected terminal, find, selection, copy, and link experience. The terminal-settings domain owns Settings-overlay behavior, persistence, Ghostty migration-report, and Browser-access-entry copy; terminal-settings-catalog owns the stable navigation taxonomy and explicitly noninteractive future-settings preview. The keyboard-shortcuts domain owns shortcut recording, scope, source, conflict, and local-persistence copy. The Desktop application-lifetime domain owns Electron-only quit and recovery copy; the Desktop-window domain owns native window-state actions and their availability guidance. Add each future product domain beneath both locale directories and export the matching descriptors in the same change. Runtime English and Simplified Chinese catalog values never share an authored file. Directory names use the canonical BCP 47 identifier and casing, and catalog tests enforce message-ID and ICU-variable parity across every shipped locale.

Large typed catalogs expose descriptors through their domain group without moving translated values
into feature code:

```ts
import { terminalSettingsCatalogMessages, useOrchbeeIntl } from '@orchbee/i18n';

const intl = useOrchbeeIntl();
const label = intl.formatMessage(
	terminalSettingsCatalogMessages.get('terminal.settings.catalog.categories.general.title'),
);
```

## Product language

[`docs/product-language.md`](docs/product-language.md) owns the approved `en-US` and `zh-CN` terminology mappings and repeated interaction labels used during message authoring and review. Catalogs remain authoritative for complete runtime messages. Update the reference and every affected locale catalog together when an approved product-language decision changes.

## Development

```sh
pnpm --filter @orchbee/i18n build
pnpm --filter @orchbee/i18n typecheck
pnpm --filter @orchbee/i18n test
pnpm --filter @orchbee/i18n test:exports
```

The authoring, fallback, and verification contract is defined in [`docs/frontend/localization.md`](../../docs/frontend/localization.md).
