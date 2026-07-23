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

## Non-React formatting

Non-React consumers use the same locale-aware formatter:

```ts
import { createOrchbeeIntl, type OrchbeeLocale } from '@orchbee/i18n';

declare const locale: OrchbeeLocale;

const intl = createOrchbeeIntl(locale);
const formattedNumber = intl.formatNumber(1234);
```

Applications own locale preference, detection, persistence, and document synchronization. This
package owns locale normalization, formatting, message descriptors, and translations; it does not
read platform globals or mutate document state.

## Catalog structure

Product copy is physically separated by locale and mirrored by product domain:

```text
src/locales/
├── en-US/
│   ├── index.ts
│   ├── messages.ts
│   └── workbench.ts
└── zh-CN/
	├── index.ts
	├── messages.ts
	└── workbench.ts
```

The Workbench domain currently owns the localized sidebar navigation chrome and visibility actions.
Add each future product domain beneath both locale directories and export the matching descriptors in the same
change. Runtime English and Simplified Chinese catalog values never share an authored file. Directory names use
the canonical BCP 47 identifier and casing, and catalog tests enforce message-ID and ICU-variable
parity across every shipped locale.

## Product language

[`docs/product-language.md`](docs/product-language.md) owns the approved `en-US` and `zh-CN`
terminology mappings and repeated interaction labels used during message authoring and review.
Catalogs remain authoritative for complete runtime messages. Update the reference and every
affected locale catalog together when an approved product-language decision changes.

## Development

```sh
pnpm --filter @orchbee/i18n build
pnpm --filter @orchbee/i18n typecheck
pnpm --filter @orchbee/i18n test
pnpm --filter @orchbee/i18n test:exports
```

The authoring, fallback, and verification contract is defined in
[`docs/frontend/localization.md`](../../docs/frontend/localization.md).
