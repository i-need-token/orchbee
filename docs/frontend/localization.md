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
the effective locale supplied to `@orchbee/workbench`. The Workbench owns the supported-locale
contract, locale normalization, product message descriptors, and translations for the interface
it renders.

Resolve the effective locale in this order:

1. An explicit user preference, when the application provides one.
2. Ordered operating-system or browser language preferences.
3. `en-US`.

Normalize English language variants to `en-US`. Normalize Simplified Chinese variants to
`zh-CN`; do not silently map a Traditional Chinese locale to Simplified Chinese. Unsupported or
invalid locale identifiers fall back to `en-US`.

Locale changes update the rendered interface without a page reload. The application synchronizes
the document `lang` attribute with the effective locale and derives `dir` from the locale instead
of assuming that every future language is left-to-right. Existing terminal transcripts and other
historical external content are not rewritten when the locale changes.

Do not create an `@orchbee/i18n`, `@orchbee/localization`, or similarly generic package while the
Workbench is the only concrete owner. Extract a shared package only when another independent
consumer needs the same non-product-specific localization capability.

## Product-message boundary

Localize every message authored by Orchbee for a product interface, including:

- Visible headings, descriptions, labels, buttons, menus, tooltips, placeholders, and empty
  states.
- Loading, validation, confirmation, error, status, and notification messages.
- Accessibility-only names and descriptions, including `aria-label`, `title`, `alt`, and
  visually hidden text.
- Product-authored onboarding or guidance rendered inside a terminal surface.

Do not translate user-authored values, session names, file paths, commands, source code, protocol
identifiers, logs, or terminal output produced by an external process or agent. Product names and
approved technical terms remain unchanged according to the product terminology glossary when one
exists.

Internal exceptions and boundary errors carry stable codes and structured values, not localized
sentences. The product interface maps an expected error code to a localized message. An
unexpected error presents a localized generic message; raw exception text may be recorded through
an owned diagnostic channel but is not shown as product copy.

`@orchbee/ui` does not select a product locale or own product-semantic messages. A generic
primitive that needs consumer-visible or accessibility-only text exposes that text for the
consumer to provide. Before a registry primitive with an upstream fixed-language default enters a
product surface, adapt it so the product-semantic consumer can supply localized text.

## Message identifiers and syntax

Use stable semantic identifiers that describe the owning domain and purpose:

```text
workbench.terminal.actions.splitRight
workbench.emptySessions.title
```

Do not use an English sentence as an identifier, encode the locale in an identifier, or reuse a
generic message merely because its current English text matches. A message descriptor includes
translator context when placement, tone, or variable meaning is not self-evident.

Write a complete translatable thought as one message. Do not concatenate translated fragments,
manually select singular and plural strings, or assume that another language preserves English
word order. Use named variables such as `{sessionTitle}` and ICU MessageFormat `plural`, `select`,
number, date, and time arguments when grammar or formatting varies by locale.

Translations do not contain arbitrary HTML. Rich text uses named, application-controlled
placeholders whose React elements are supplied by the caller. Never render translated markup with
`dangerouslySetInnerHTML`.

## Runtime implementation

`@orchbee/workbench` uses `react-intl` and ICU MessageFormat. English source messages and
translator descriptions live in typed message descriptors; `zh-CN` translations form a complete
catalog keyed by the same stable identifiers. Feature code formats descriptors instead of
rendering product-facing string literals directly.

The `Workbench` composition boundary accepts an effective locale from its application consumer
and installs the localization provider for all product-semantic descendants. Application-only
messages remain in the application that owns them rather than being added to the Workbench
catalog.

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
