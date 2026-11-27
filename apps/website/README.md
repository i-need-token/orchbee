# Orchbee Website

## Commands

Run from the repository root:

```sh
pnpm website
pnpm website:build
pnpm website:preview
```

Development and static preview use `http://127.0.0.1:3100`. Development resolves shared workspace exports to source, so editing translations and tokens requires no library rebuild. The build command prepares the declared localization and token dependencies before building the application.

## Static delivery

The site is static files in `apps/website/dist/`. There is no request-time rendering, server process, account, analytics service, or reporting endpoint. React renders HTML during the build, and the build renderer is removed from the output. Production JavaScript only activates that markup in place and carries no React; every page is complete and readable before activation and without JavaScript. The published script has a budget of 128 kB: `scripts/generate-pages.mjs` fails the build when the JavaScript under `dist/static/js/` exceeds it, which almost always means an activation imported a build-time-only package or the renderer.

The site has four pages: home at `/`, the terminal interface at `/terminal/`, the desktop application at `/desktop/`, and the command line at `/cli/`. Simplified Chinese repeats them under `/zh-CN/`. Every page contains complete HTML, localized metadata, a canonical link and reciprocal language links, Open Graph and Twitter metadata with `/og.png`, and structured data, all naming `https://orchbee.com`. The build also emits `robots.txt`, a `sitemap.xml` that lists all eight pages, and an unindexed `404.html` in each language directory. `public/_headers` sets security and caching headers for Cloudflare and is not served itself.

The site also serves the signed update feed that `orchbee update` reads: `/update/<channel>/<platform>/feed.json` and `feed.json.sig`, with `Cache-Control: no-cache`, so a channel move or a withdrawal is seen at once. The tracked source is `public/update/<channel>/<platform>/feed.json.signed` and `feed.json.sig`: the signature covers the exact bytes, so the document keeps a name the formatter leaves alone, and `scripts/generate-pages.mjs` renames it to `feed.json` in `dist/`, byte for byte, and fails the build when a folder holds anything but the pair. A channel or platform without files answers 404. Nothing is tracked before the first update-capable release; the address and the format are frozen from then on, see [Release feed](../../proposals/platform/release-distribution.md#release-feed). A deploy switches both files together; a client that catches one old and one new file is refused and succeeds on its next try.

The site serves the install script `public/install.sh` at `/install.sh`, run as `curl -fsSL https://orchbee.com/install.sh | sh` on macOS and Linux; its header states what it does, its options, and its two test variables. It reads the `stable` feed of its platform from `/update/`, so a platform without a tracked pair gets one sentence and nothing installed. On a Mac older than macOS 13 it stops before downloading anything. Its messages follow `LC_ALL`, `LC_MESSAGES` or `LANG` in `en-US` and `zh-CN`, as the native launchers do, because a piped script cannot reach the localization package. `scripts/release-smoke-macos.mjs install-script` runs it in every macOS release smoke against the row's own files.

`wrangler.jsonc` publishes `dist/` as a Cloudflare Worker with static assets only. Deploying it takes these steps:

1. Build the site with `pnpm website:build` from the repository root.
2. Sign in to the Cloudflare account that holds the active `orchbee.com` zone with `pnpm --filter @orchbee/website exec wrangler login`.
3. Deploy with `pnpm --filter @orchbee/website run deploy`. On the first deploy, Wrangler attaches `orchbee.com` and `www.orchbee.com` as custom domains and creates their DNS records and certificates.
4. Check that `https://orchbee.com/zh-CN` and `https://orchbee.com/terminal` redirect to their addresses with a trailing slash, that each of the eight pages answers, and that an unknown address under `/zh-CN/` returns status 404 with the Chinese page.

Both host names serve the same files; canonical links send search engines to `https://orchbee.com`. The Worker has no `workers.dev` address and no preview addresses.

## Pages and components

`src/website/pages.ts` names the pages and their addresses; `scripts/generate-pages.mjs` renders
every page in both languages through `src/composition/prerender.tsx`, and the development server
routes an address through the same table.

A page component only lists its sections in order.

The shared column is the `wrap` class of `PageShell.module.css`; a home section composes `section` and a client page section composes `detailSection` from it (`composes: section from '../page-shell/PageShell.module.css'`), which also gives the section's spacing at each width. Content that rises into view carries `data-reveal`.

### Adding a section

1. Create the section's directory: under `home-page/components/` for home, or under the client page's own `components/` directory, with its `.tsx` and `.module.css`.
2. Add it to its page component in page order. A section touches no file another page owns.
3. Keep the heading outline whole: the page head is the only `h1`, a section title is an `h2`, and a title inside a section is an `h3`, whatever its size. A section whose only label is a kicker passes `heading` to `Kicker`. A message inside a figure is a paragraph, not a heading.
4. Put its copy in its page's message domain (below) and its tokens in the website block of the token file (below).
5. If it needs script, add an activation module beside it and list it in its page's `activations.ts`.

### Activation

`src/composition/entry.tsx` runs in the document head before the body renders. It marks the document with `data-js`, and with `data-motion` unless the visitor asked for reduced motion, so styles can show script-only states and start entrances hidden from the first paint. After the document loads it runs the list in `src/website/activations.ts`: the chrome's activations, then each page's own list from `components/<page>-page/activations.ts`.

An activation module is an `activate<Name>.ts` file beside its component that exports one function taking an `ActivationContext` (`src/website/activation.ts`). It finds its component by a data attribute and does nothing on a page without one. A control that only works with script renders `hidden` and is shown by its activation; one that also carries content the page needs, such as a step of a figure or a theme of a list, renders `disabled` and is enabled by it. Under reduced motion, or without JavaScript, every scene shows its final state. The package declares only its stylesheets as side effects, so the browser bundle keeps the activations and drops every component the entry does not run; no website module may do work when it is imported.

Every list runs on every page, so the activation of a shared component, which several pages render, goes in the global list in `src/website/activations.ts` instead of a page's list, as `activateGetOrchbee`, `activateDownloadDialog`, `activateSessionScene`, `activatePanelPlay` and `activateNumberedMarks` do; it then binds once on whichever page renders the component.

### Drawn product windows

The pages draw the product from its real interface as HTML, never as screenshots. A window is built from parts that each work alone, so a later figure composes the parts it needs:

- `window-fit/WindowFit` is the slot a window fills. Every window keeps the product's real pixel
  size and zooms to the width of its slot with `zoom: min(tan(atan2(100cqw, <width>)), 1.3)`, which
  needs no script. The slot is the `window-fit` container: at `@container window-fit (max-width:
48rem)` a window takes its mini layout and at `40rem` its phone layout, so a narrow slot shows a
  narrower window rather than a smaller picture of the wide one. Variants key on the window's
  `data-size`; `full` is the size that narrows. A new size is a new `data-size` value with its own
  pixel size and container rules. `mini` (TuiWindow and DesktopWindow, 660×400) starts at the mini
  layout, so two mini windows side by side keep their sidebars in slots narrower than `40rem`. It
  takes the phone layout (430×430, no sidebar) on a viewport of at most `40rem`, by media query: its
  slot cannot tell a narrow page from one column of a wide page.
- The desktop application's drawn parts copy the product's metrics and use the product's own tokens (`--ob-sys-color-*`,
  `--ob-comp-workbench-*`, the product type roles) rather than website tokens, and the window root
  sets `data-theme`, so the product's dark or light values apply inside it.
- Terminal contents are English in every language; the chrome around them takes its words from the
  product's own message descriptors (`workbenchMessages`, `agentObservationMessages`) or from
  `websiteProductMessages`.
- A drawn window takes another terminal theme in two shared pieces, so the product's terminal package stays out of the published script. At build time, `terminal-text/terminalThemeData(theme)` turns one of the product's `terminalColorThemes` (from `@orchbee/terminal`) into page data: the theme's `polarity` and `colors`, its whole palette as `name:#rrggbb` entries, including the roles the theme does not name (bold, dim, input, line, and row selection), which it mixes from the theme's foreground and background. The component writes them as `data-terminal-theme` and `data-terminal-colors` on the element the visitor presses, as the desktop page's appearance buttons do (the style contract allows no attribute spread). In the browser, `terminal-text/applyTerminalTheme(window, source)` reads that data from `source`, removes the `--ob-comp-website-terminal-*` properties a previous theme wrote on the window, writes the new ones through the CSSOM, and sets the window's `data-theme` to the theme's polarity so the product's own tokens follow. Orchbee Dark, the stylesheet's theme, carries no colors, so applying it leaves the stylesheet's values. Only a component imports `terminalThemeData`; an activation imports only `applyTerminalTheme`. Every palette entry has its token with Orchbee Dark's value, ANSI 0–15 and the selection's background and foreground among them, so a figure can draw any of them.
- The terminal palette is the public `--ob-comp-website-terminal-*` tokens, Orchbee Dark by default. A figure re-themes a drawn terminal by setting these properties on its window element through the CSSOM (`element.style.setProperty`), never with a `style` attribute. The console and host frame colors are system tokens and do not re-theme.
- Windows are pictures: the root has `role='img'` and a page-language `aria-label`, and nothing inside takes focus.

### Scenes

A scene is a script that plays in drawn windows. `src/website/scene/` holds what every scene shares:

- `scene.ts`: a `Scene` is an immutable initial state, time-stamped steps that each return a new state, a fade time, and a length. `sceneFinalState` folds every step; the static page renders that final state, which is also what the visitor sees without script and under reduced motion.
- `bindScene(root)` finds the bound elements once and returns a function that writes a frame, touching only values that changed; rows compare by identity, so a scene keeps unchanged rows as the same array. Script writes rows by cloning the row and cursor elements from the `<template>` a bound TerminalText renders, so every classed element comes from the page; it creates only unstyled `b` and `span[data-tone]` elements and never writes a `style` attribute or HTML.
- An activation never starts a player under reduced motion.
- Script reads page-language words from the page, never from a second catalog: the scene root renders a `<template data-scene-strings>` with one `<span data-key>` per word it may need.

`SessionScene` is the root of the scripted Session (`session-scene/`), the scene the pages share: wrap every element the Session drives, windows, status lines and lists
alike, and `activateSessionScene` (in the global activation list) plays it in each root on the page,
watching the WindowFit slots inside. To show the Session elsewhere, put a SessionScene around the
new figure and give its elements the Session's binding keys; to make another scene, write a scene
module and an activation in the same shape.

The agents section's list of recognized Agents (`recognized-agents.ts`) names each by the product's own provider
descriptor, in the order of the product's provider list; a provider the product adds is added there.
A module an activation imports, such as a scene or `agent-ring.ts`, imports no message catalog: the
production script would carry every catalog with it.

The terminal section's `terminal-themes.ts` reads `terminalColorThemes` at build time and groups them by polarity in the
product's order, under the names the product's settings show; a theme the product adds without a
name there stops the page build.

### Panels to try

A client page lets the visitor split, focus, resize, maximize and close Panels in a drawn window. The model, the drawing and the window are separate pieces, so each client plugs in its own window:

- `panel-tree/panel-tree.ts` is the model: an immutable layout (a tree of splits and leaves, the focused leaf, and whether it is maximized) and functions that each return an outcome: a new layout, a new divider ratio, a new focus, or a refusal with its reason (`full`: six Panels already; `small`: the Panel is too small to split that way; `last`: the last Panel stays; `one`: a single Panel cannot maximize; `none`: no divider in that direction). It knows no elements and no keys.
- A leaf component takes `PanelLeafProps`: its root carries `data-leaf` with the Panel's id,
  `data-focus` while focused, and `data-alone` while it is the only Panel; an element inside marked
  `data-panel-close` closes the Panel on click; its terminal text binds its rows as `panel`. A split
  is `[data-split][data-axis]` with two `[data-part]` children and a `[data-divider]` between them;
  style them by those attributes.
- `activatePanelPlay` runs every PanelPlay on the page. Without script the window is a picture and the buttons and hint stay hidden. With script the window's host becomes a focusable `role='application'` region with a page-language label and a description of its keys: while it has focus, keys act on its Panels (the terminal interface's keys work with or without Ctrl+b first), Escape takes focus out of it, and Tab leaves as usual. Nothing in it moves by itself, so it works the same under reduced motion.
- Another client adds its own window component in the same shape: a leaf component that follows the
  contract above, a `data-size` of its own if it needs another pixel size, and a focus ring under
  `[data-panel-host]:focus-visible`, as `TuiWindow.module.css` draws one.
- The desktop application closes a Panel with Ctrl+Shift+W, which browsers keep for closing their
  own window, so in the drawn window that action is a button only.

### Numbered marks

A figure can number the parts of a drawing and explain each in a note beside it; a part and its note light up together. `numbered-mark/` holds the piece, and the page supplies the drawing, the notes, and their styles:

- The figure marks its scope with `data-marks` on an element around the drawing and the notes; numbers match only inside one scope.
- Inside it, an element with `data-mark='<number>'` is a part of the drawing; one with `data-mark-note` as well is the note that explains that number. Notes are ordinary text, such as items of an `<ol>`.
- `NumberedMark` draws the numbered disc. It is hidden from assistive technology, since the notes carry the meaning. It takes no class name: to place a disc on a drawing, wrap it in an element of the figure's own styles.
- `activateNumberedMarks` (in the global activation list) gives every element of the same number `data-hot` while one of them is under the pointer or its note has keyboard focus; activation makes the notes focusable, so keyboard users get the same pairing. A disc fills while it, or an element around it, carries `data-hot`; style any other lit state, such as an underline, by `[data-hot]` in the figure's own module. Nothing changes by itself, so it is the same under reduced motion.

The command line page's grammar figure (`cli-page/components/cli-grammar/`) is the example.

### Messages

`@orchbee/i18n` owns the website copy in six domains with paired English and Chinese catalogs: `websiteShellMessages` for the chrome, the client names and summaries, shared call-to-action copy and the 404 page; `websiteHomeMessages`, `websiteTerminalMessages`, `websiteDesktopMessages` and `websiteCliMessages` for each page's metadata and content; and `websiteProductMessages` for the copy of the drawn product windows that more than one page shows (window labels, the Session status line). Words the product itself shows come from the product's own descriptors instead, as the desktop page's notification and notification-area menu take theirs from `desktopTrayMessages`. A section adds its messages to its page's domain, in `packages/i18n/src/locales/{en-US,zh-CN}/website-<page>.ts` and `packages/i18n/src/descriptors/website-<page>.ts`. Messages mark a command with `<code>` and emphasis with `<b>`; pass `richText` from `src/website/rich-text.tsx` as the values to render them. English and Chinese are phrased independently; commands stay in English in both languages. Locale addresses are explicit language choices, so the language links are ordinary links that work without JavaScript; with JavaScript they also keep the section the visitor is reading. New locales require matching static routes, catalogs, metadata, language links, 404 pages, and sitemap alternatives.

Copy that a shared component shows the same way on several pages goes in `websiteShellMessages`: the detail kickers (`detailHighlights`, `detailReference`) and the PanelPlay strings. A page's own content in those components (its titles, highlights, and key labels) stays in its page's domain. A message that shows `{` or `}` quotes it (`'{'`), since braces start a placeholder.

### Tokens

The website's colors, type, shadows, motion, layers, and lengths live in the one website block of `packages/design-tokens/src/tokens.css`, dark only. Component styles use only these tokens; `pnpm lint:styles` enforces the style contract. A section adds the tokens it needs to the block, keeping each group sorted, then runs `pnpm generate:design-token-inventory`. When two branches both add tokens, keep both insertions and regenerate the inventory. A tracked type role registers `--ob-sys-typography-website-<role>-tracking` right after its role, in `em`, and the component sets it on `letter-spacing` beside the `font` shorthand. Media queries use the shared breakpoints of `@orchbee/design-tokens` (`64rem`, `48rem`, `40rem`); container queries on a window's `window-fit` slot use the same values.

## Release state

`src/website/release.ts` is the only release switch. While `release` is `{ published: false }`, every call to action leads to the source build in the [GitHub repository](https://github.com/i-need-token/orchbee), and the install section shows the source commands. Once a release is published, its value names the version and, in `downloads`, the download of each platform the release published. The `PLATFORMS` catalog in the same file describes each platform: its tab and download group, its installers, its install commands and its copy keys. The download dialog's groups, the call-to-action buttons with their link line, and the install section's tabs and rows are all built from that catalog and show only the platforms the release published, in catalog order. The navigation bar and the home hero open the download dialog. After hydration the visitor's platform comes first in the download dialog and the call to action, and opens the install section; the server renders catalog order. The data sheet, the install section's lead and start note, and the desktop page's macOS and Linux sections test `offeredPlatforms`, `offersMacos` or `offersLinux`, which read the same catalog, so they name macOS and Linux only while each is published. A command that several published platforms offer, such as the install script of macOS and Linux, shows once in the download dialog under its `sharedLabel`.

To add a platform, in this order:

1. In `release.ts`, add the platform's download keys to `ReleaseDownloads`, one per installer. In `scripts/website-release.mjs`, add the platform to the `downloads` table with each of those keys and its file name; the release script refuses a release that publishes a platform missing from that table. `pnpm website:release <version>` then writes its download into the `release` block under that key, once a release publishes that installer.
2. In `release.ts`, add a `PLATFORMS` entry with `id`, `name` (its copy key), `icon`, the browser `visitor` pattern, `variants`, `commands` and `install`. The pattern must not match Android, whose platform string also starts with "Linux". Each variant has its `download` key and the copy keys `label` (the download dialog), `install` (the install section's button) and `cta` (the call to action's button, when it is the platform's first published installer). `link` gives a non-first installer's text in the call to action's link line; `cta` stands in when there is no `link`. `commands` lists other ways to install, if any. `install` holds the copy of the platform's install rows: `title`, `detail` and `note`, plus `commands` when it has commands.
3. Add the platform's icon to `PATHS` in `components/icon/Icon.tsx`, under the name the entry's `icon` gives.
4. Add every copy key the entry references to `packages/i18n/src/descriptors/website-shell.ts`, each with a description for translators, and to both locales, `en-US` and `zh-CN`. Run `pnpm --filter @orchbee/i18n build`; the typecheck names a key that a locale lacks or the catalog does not know.
5. Update the sentences that name platforms. They are copy, not generated: the install section's lead and start note (`installLead*` and `installStartNote*` in `website-home`, chosen by `offeredPlatforms`), the data sheet's platform row (`components/home-page/components/statement/Statement.tsx`), and the desktop page's macOS and Linux sections (`DesktopBackground.tsx`, `DesktopHighlights.tsx`, `DesktopKeys.tsx`). Each needs the new platform in both locales.
6. Run `pnpm --filter @orchbee/website typecheck` and `pnpm --filter @orchbee/website test`. Then check the call to action and the install section at 400 and 1440 pixels in both locales, with a visitor of each platform.

Nothing else needs a change: the call-to-action buttons, the download dialog's groups, the install tabs and rows, and the visitor ordering all read the catalog.

`pnpm website:release <version>` (`scripts/website-release.mjs`) moves the download buttons and the update channel together, so they always name the same version. It reads the platform rows the release published from the feed documents its `SHA256SUMS` lists; Windows must be among them, and each must be a platform the website offers a download for. For each row it fetches the feed pair anonymously with the system curl, verifies the signature with `RELEASE_KEYS` before reading the feed, checks the feed against the reader's rules and the version, confirms the row's download is public, and downloads the archive the feed names to check its size and SHA-256. Only then does it write every row's tracked pair and the `release` block, and remove the pair of any platform of the channel the release did not publish; any failed check writes nothing. A draft or a missing asset answers 404 and is refused with what to do. Running it with an older version withdraws a bad release. `pnpm --filter @orchbee/website test` refuses a tracked pair that no release key signed or whose version differs from `release.version`.

## Type, brand, and look

Geist and Geist Mono set the pages; JetBrains Mono is reserved for the text of drawn terminals. Chinese text uses the visitor's system fonts. All three come from Fontsource packages and are served from the site itself; the build never inlines them, because the Content Security Policy loads fonts only from the site. Their notices are in the repository `THIRD_PARTY_NOTICES.md`.

The site has one dark look; the drawn desktop application carries the product's own theme scope. Agent states use the product's fixed status colors. The three text colors keep a contrast of at least 4.5:1 on the page and on the raised surface, the quietest one included; text inside a drawn window keeps the product's colors instead.

The navigation bar and the favicon use `resources/icon.svg`; `favicon.ico`, `apple-touch-icon.png`, and `og.png` are copied from `resources/brand/`. The mark carries its own tile and corner radius, so do not crop, recolor, or round it again in page styles.

## Privacy

The site stores nothing in the browser. Production has no cookies, third-party fonts, scripts, or pictures, analytics, beacons, or telemetry. Its Content Security Policy forbids inline styles, connections, forms, plugins, framing, and external resources. Hosting access logs are outside the website code.

## Verification

Run the applicable root gate in [Verification](../../docs/VERIFICATION.md), plus `pnpm website:build` when changing pages, static generation, fonts, or delivery. `pnpm --filter @orchbee/website test` checks the server-free rendering rules for every page in both languages and both release states: one headline, no `style` attribute, same-origin resources only, each release state's calls to action, and the canonical and language links of each page.

Review the generated site through `pnpm website:preview` in a real browser: full-page pictures at desktop and narrow widths in both languages, the published and unpublished states, and the 404 pages. Also check keyboard focus, the menu at narrow widths, copy buttons, language links, reduced motion, disabled JavaScript, and the absence of requests to other origins. These browser checks are manual evidence, not covered by the unit suite.
