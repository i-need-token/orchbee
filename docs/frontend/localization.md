# Frontend Localization

## Authority and supported locales

This document owns locale selection, product-message authoring, translation catalogs, formatting,
fallback, and localization verification for frontend product interfaces. Visual adaptation to
translated content follows [`visual-foundations.md`](visual-foundations.md), and package placement
follows [`package-boundaries.md`](package-boundaries.md).

Orchbee ships these product locales:

| Locale  | Language                   | Role                         |
| ------- | -------------------------- | ---------------------------- |
| `en-US` | English (United States)    | Source locale and fallback   |
| `zh-CN` | Simplified Chinese (China) | Required shipped translation |

Use canonical BCP 47 locale identifiers exactly as listed. A change that adds or modifies a
product-facing message updates both locales in the same change. Runtime fallback is resilience,
not permission to merge an incomplete catalog.

English remains canonical for source identifiers, comments, and engineering documentation.
Localized message values are product assets rather than canonical documentation and contain the
language of their locale.

## Locale ownership and resolution

Application workspaces own the user's locale preference, platform detection, persistence, and
effective-locale selection. `@orchbee/i18n` owns the supported-locale contract, locale
normalization, text direction, formatting runtime, product message descriptors, and every shipped
translation catalog. Applications pass a validated locale to the package provider at their
composition boundary; feature packages consume that provider and never detect the host locale.

Resolve the effective locale in this order:

1. An explicit user preference, when the application provides one.
2. Ordered operating-system or browser language preferences.
3. `en-US`.

Normalize English language variants to `en-US`. Normalize Simplified Chinese variants to
`zh-CN`; do not silently map a Traditional Chinese locale to Simplified Chinese. Unsupported or
invalid locale identifiers fall back to `en-US`.

Locale changes update the rendered interface without a page reload. The application synchronizes
the document `lang` attribute with the effective locale and derives `dir` from the locale instead
of assuming that every future language is left-to-right. Existing externally produced transcripts and other
historical external content are not rewritten when the locale changes.

`@orchbee/i18n` does not read browser or operating-system globals, store a preference, mutate the
document, or choose a locale on behalf of an application. The application synchronizes document
metadata after resolving the locale through the package's pure locale contract.

## Package boundary and catalog ownership

All Orchbee product workspaces use `@orchbee/i18n`; no other workspace imports `react-intl` or owns
a product translation catalog. Applications provide the effective locale at their composition
boundary, and feature packages consume the shared formatting capability without detecting the host
locale or creating another message system.

Install exactly one localization provider above the product-semantic interface in each application.
Do not install a feature-local provider, pass translated strings through unrelated component
layers, or let a reusable feature package select its own locale. Non-React product adapters format
against the same validated locale and catalogs.

The owning package `README.md` is authoritative for exported symbol names and consumer examples.
This standard owns catalog placement, authoring boundaries, fallback, and verification across
workspaces.

Keep every localized value physically inside
`packages/i18n/src/locales/<canonical-locale>/<product-domain>.ts`. Locale directory names use the
canonical BCP 47 identifier, including its canonical casing, and mirror the same product-domain
filenames. English and translated product copy must never be authored together in one file:
`locales/en-US/` contains only English values, and `locales/zh-CN/` contains only Simplified Chinese
values. A message addition, removal, or move updates the matching domain file in every shipped
locale in the same change.

Group exported descriptors under `packages/i18n/src/descriptors/` by the same stable product
domains and export each group as `<domain>Messages`. A descriptor's English `defaultMessage`
references the corresponding value from the `en-US` domain catalog instead of duplicating the
copy. Stable IDs and English translator descriptions are source metadata, not localized values.
Compose one complete runtime catalog per shipped locale; adding a domain does not create another
provider or locale contract.

## Product-message boundary

Localize every message authored by Orchbee for a product interface, including:

- Visible headings, descriptions, labels, buttons, menus, tooltips, placeholders, and empty
  states.
- Loading, validation, confirmation, error, status, and notification messages.
- Accessibility-only names and descriptions, including `aria-label`, `title`, `alt`, and
  visually hidden text.
- Product-authored onboarding or guidance rendered inside another content surface.

Do not translate user-authored values, item names, file paths, commands, source code, protocol
identifiers, logs, or output produced by an external process or tool. Product names and
approved technical terms remain unchanged according to the product terminology glossary when one
exists.

Internal exceptions and boundary errors carry stable codes and structured values, not localized
sentences. The product interface maps an expected error code to a localized message. An
unexpected error presents a localized generic message; raw exception text may be recorded through
an owned diagnostic channel but is not shown as product copy.

A generic UI primitive package does not select a product locale or own product-semantic messages. A
primitive that needs consumer-visible or accessibility-only text exposes that text for the consumer
to provide. Before a registry primitive with an upstream fixed-language default enters a product
surface, adapt it so the product-semantic consumer can supply localized text.

## Message identifiers and syntax

Use stable semantic identifiers that describe the owning domain and purpose:

```text
navigation.actions.open
navigation.emptyState.title
```

Do not use an English sentence as an identifier, encode the locale in an identifier, or reuse a
generic message merely because its current English text matches. A message descriptor includes
translator context when placement, tone, or variable meaning is not self-evident.

Write a complete translatable thought as one message. Do not concatenate translated fragments,
manually select singular and plural strings, or assume that another language preserves English
word order. Use named variables such as `{itemTitle}` and ICU MessageFormat `plural`, `select`,
number, date, and time arguments when grammar or formatting varies by locale.

Translations do not contain arbitrary HTML. Rich text uses named, application-controlled
placeholders whose React elements are supplied by the caller. Never render translated markup with
`dangerouslySetInnerHTML`.

## Runtime implementation

Message formatting uses ICU MessageFormat. English source messages live in the `locales/en-US/`
catalog, Simplified Chinese messages live in `locales/zh-CN/`, and every locale
provides a complete catalog keyed by the same stable identifiers. Typed descriptors reference the
English catalog values as defaults, so fallback and source-locale formatting share one authored
English value. Feature code formats exported descriptors instead of rendering product-facing
string literals directly.

Runtime fallback follows one path: locale negotiation falls back to `en-US`, a missing translated
message falls back to its English descriptor default, and an invalid or missing descriptor is a
development error. Fallback behavior is resilience only; catalog parity remains a merge
requirement.

Use locale-aware formatters for numbers, lists, dates, times, relative times, and display names.
Store and transport locale-neutral values such as timestamps and numeric quantities; format them
only at the presentation boundary with the effective locale and an explicit time zone when the
meaning depends on one.

## Layout and interaction

Product layouts support every shipped locale without shrinking typography. Controls and rows use
intrinsic sizing or `min-height` so translated labels and descriptions can wrap. Do not use a
fixed width derived from one English label, place required copy only in a placeholder, or encode
text in an image.

Truncation is acceptable for user data when the full value remains accessible. Do not truncate a
required action label, error, or instruction merely to preserve a fixed composition. Review
keyboard access, focus order, line breaking, and accessible names in both shipped locales.

## Verification

Localization changes satisfy all of these checks:

1. The source descriptors and every shipped translation catalog contain the same message IDs.
2. ICU syntax parses in every locale and corresponding messages use compatible variables.
3. Locale resolution covers exact matches, supported language variants, invalid input,
   unsupported languages, and Traditional Chinese fallback.
4. Representative product interfaces render under both `en-US` and `zh-CN`.
5. Product review searches changed interface code for visible and accessibility-only raw strings.
6. Storybook and visual review exercise translated expansion, wrapping, zoom, and narrow layouts
   for affected components.
7. The repository `pnpm verify` gate passes.

Prefer assertions on behavior and message identity over repeating incidental English copy in
unrelated tests. Assert exact translated output when locale selection, interpolation, or the copy
itself is the behavior under test.

## External references

Verified on 2026-07-18:

- [React Intl](https://formatjs.github.io/docs/react-intl/)
- [FormatJS message descriptors](https://formatjs.github.io/docs/react-intl/api/#message-descriptor)
- [ICU message syntax](https://formatjs.github.io/docs/core-concepts/icu-syntax/)
