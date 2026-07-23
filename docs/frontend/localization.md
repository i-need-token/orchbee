# Frontend Localization

## Authority and locales

| Responsibility                                                                             | Owner                                                                                      |
| ------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| User preference, platform detection, persistence, effective locale, and document metadata  | The composition application                                                                |
| Supported locales, normalization, direction, formatting, descriptors, and shipped catalogs | `@orchbee/i18n`                                                                            |
| Product terminology and repeated interaction copy                                          | [`@orchbee/i18n` product language reference](../../packages/i18n/docs/product-language.md) |
| Visual adaptation to translated content                                                    | [`visual-foundations.md`](visual-foundations.md)                                           |
| Package placement                                                                          | [`package-boundaries.md`](package-boundaries.md)                                           |

| Locale  | Role                         |
| ------- | ---------------------------- |
| `en-US` | Source locale and fallback   |
| `zh-CN` | Required shipped translation |

Use these canonical BCP 47 identifiers exactly. Every product-message addition, change, move, or deletion updates both catalogs in the same change; fallback never permits an incomplete catalog.

English remains canonical for identifiers, comments, and engineering documentation. Localized message values are product assets.

## Locale resolution

Resolve the effective locale in order:

1. Explicit user preference when supported by the application.
2. Ordered operating-system or browser language preferences.
3. `en-US`.

- Normalize English variants to `en-US` and Simplified Chinese variants to `zh-CN`.
- Fall back to `en-US` for invalid, unsupported, and Traditional Chinese identifiers; never silently map Traditional Chinese to Simplified Chinese.
- Pass the validated locale from the application composition boundary to `@orchbee/i18n`; feature packages never inspect host locale.
- Update rendered messages and the document `lang` and derived `dir` without a page reload.
- Do not rewrite externally produced transcripts or historical external content after a locale change.
- `@orchbee/i18n` never reads host globals, stores preferences, mutates the document, or chooses a locale for an application.

## Catalog and provider contract

Install exactly one localization provider above each product-semantic application interface. Feature packages consume it; they do not create providers, choose locales, or pass translated strings through unrelated layers. Non-React adapters use the same validated locale and catalogs.

`@orchbee/i18n` is the only product localization system; no other workspace imports `react-intl` or owns a product catalog.

| Artifact            | Contract                                                                                                                   |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Locale catalog      | `packages/i18n/src/locales/<canonical-locale>/<product-domain>.ts`; locale directories mirror domain filenames             |
| English values      | Only under `locales/en-US/`                                                                                                |
| Simplified Chinese  | Only under `locales/zh-CN/`                                                                                                |
| Descriptor group    | `packages/i18n/src/descriptors/<product-domain>.ts` under `packages/i18n/src/descriptors/`, exported as `<domain>Messages` |
| Descriptor default  | `defaultMessage` references the matching `en-US` catalog value instead of duplicating English copy                         |
| Runtime catalog     | One complete catalog per locale with identical stable IDs                                                                  |
| Public API guidance | Exported names and consumer examples live in the owning package `README.md`                                                |

Stable IDs and English translator descriptions are source metadata. Adding a domain never creates another provider or locale contract.

## Product-message boundary

| Localize                                                                                 | Do not translate                                                                                       |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| Headings, descriptions, labels, buttons, menus, tooltips, placeholders, and empty states | User values, item names, file paths, commands, source code, protocol identifiers, logs, or tool output |
| Loading, validation, confirmation, error, status, and notification messages              | Product names and approved technical terms                                                             |
| `aria-label`, `title`, `alt`, and visually hidden product text                           | Raw internal exceptions or boundary diagnostics                                                        |
| Product-authored onboarding and guidance                                                 | Externally produced content                                                                            |

- Internal failures carry stable codes and structured values; the UI maps expected codes to localized messages and unexpected failures to a localized generic message.
- Raw exception text may enter an owned diagnostic channel but never product copy.
- Generic UI primitives receive consumer-visible and accessibility text from their product-semantic consumer.
- Adapt any registry primitive with a fixed-language default before using it in a product surface.

## Terminology and interaction copy

Use the [`@orchbee/i18n` product language reference](../../packages/i18n/docs/product-language.md) as the authority for approved product-term mappings and repeated interaction labels in every shipped locale.

- Consult the reference before adding or changing product copy. If a recurring product concept or ambiguous interaction has no entry, add a concrete decision when the affected product messages are implemented.
- Update the reference and every affected locale catalog in the same change when an approved term or interaction label changes.
- Prefer action-specific labels based on the result: creating, saving, submitting for processing, applying, confirming, acknowledging, canceling, closing, deleting, and removing are distinct actions.
- Do not force identical wording across different meanings or reuse an unrelated message ID merely to obtain consistent text.
- Runtime catalogs own complete context-specific messages. The reference owns stable word choice; it is not a second catalog.

## Message identifiers and syntax

Use stable semantic IDs that name the owning domain and purpose:

```text
navigation.actions.open
navigation.emptyState.title
```

- Never use an English sentence as an ID, encode a locale in an ID, or reuse an unrelated ID because the current English copy matches.
- Add English translator context when placement, tone, or variable meaning is not self-evident.
- Author one complete translatable thought; never concatenate fragments or assume English word order.
- Use named variables such as `{itemTitle}` and ICU MessageFormat `plural`, `select`, number, date, and time arguments.
- Rich text uses named application-controlled placeholders; translations contain no arbitrary HTML and never use `dangerouslySetInnerHTML`.

## Runtime and layout

- Feature code formats exported descriptors instead of rendering product-facing string literals.
- Fallback follows one path: negotiation to `en-US`, missing translation to the English descriptor default, and invalid or missing descriptor to a development error.
- Use locale-aware number, list, date, time, relative-time, and display-name formatters.
- Store and transport locale-neutral values; format them at presentation with the effective locale and an explicit time zone when required.
- Size layouts intrinsically or with `min-height` so translated content can wrap without smaller typography.
- Do not derive fixed widths from English, put required instructions only in placeholders, encode text in images, or truncate required actions, errors, and instructions.
- User data may truncate only when the full value remains accessible.
- Review wrapping, zoom, focus order, keyboard access, and accessible names in both locales.

## Verification

| Check             | Required result                                                                                                       |
| ----------------- | --------------------------------------------------------------------------------------------------------------------- |
| Catalog parity    | Source descriptors and every shipped catalog contain identical IDs                                                    |
| ICU compatibility | Every message parses and corresponding locale values use compatible variables                                         |
| Resolution        | Exact matches, supported variants, invalid input, unsupported languages, and Traditional Chinese fallback are covered |
| Rendering         | Representative interfaces render under `en-US` and `zh-CN`                                                            |
| Raw strings       | Changed product code contains no unowned visible or accessibility-only product strings                                |
| Language review   | Changed copy follows approved terminology and repeated interaction labels                                             |
| Layout            | Storybook and visual review cover expansion, wrapping, zoom, and narrow layouts                                       |
| Repository gate   | `pnpm verify` passes                                                                                                  |

Assert behavior and message identity by default; assert exact translated output only when locale selection, interpolation, or copy is the behavior under test.

Primary references: [React Intl](https://formatjs.github.io/docs/react-intl/), [message descriptors](https://formatjs.github.io/docs/react-intl/api/#message-descriptor), and [ICU syntax](https://formatjs.github.io/docs/core-concepts/icu-syntax/).
