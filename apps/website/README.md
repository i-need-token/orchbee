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

The site is static files in `apps/website/dist/`. There is no request-time rendering, server process, account, analytics service, or reporting endpoint. React renders HTML during the build, and the build renderer is removed from the output. Production JavaScript activates that markup in place: `src/website/activateWebsite.ts` adds the appearance button, copy buttons, a language link that keeps the visitor's place, the header progress, the particle swarm, the hero story, and the screen recordings. Every scene is complete and readable before activation and without JavaScript.

English lives at `/`; Simplified Chinese lives at `/zh-CN/`. Both URLs contain complete HTML, localized metadata, canonical and reciprocal language links, Open Graph and Twitter metadata with `/og.png`, and WebSite structured data, all naming `https://orchbee.com`. The build also emits `robots.txt`, `sitemap.xml`, and an unindexed `404.html` in each language directory. `public/_headers` sets security and caching headers for Cloudflare and is not served itself.

The site also serves the signed update feed that `orchbee update` reads: `/update/<channel>/<platform>/feed.json` and `feed.json.sig`, with `Cache-Control: no-cache`, so a channel move or a withdrawal is seen at once. The tracked source is `public/update/<channel>/<platform>/feed.json.signed` and `feed.json.sig`: the signature covers the exact bytes, so the document keeps a name the formatter leaves alone, and `scripts/generate-pages.mjs` renames it to `feed.json` in `dist/`, byte for byte, and fails the build when a folder holds anything but the pair. A channel or platform without files answers 404. Nothing is tracked before the first update-capable release; the address and the format are frozen from then on, see [Release feed](../../proposals/platform/release-distribution.md#release-feed). A deploy switches both files together; a client that catches one old and one new file is refused and succeeds on its next try.

`wrangler.jsonc` publishes `dist/` as a Cloudflare Worker with static assets only. Deploying it takes these steps:

1. Build the site with `pnpm website:build` from the repository root.
2. Sign in to the Cloudflare account that holds the active `orchbee.com` zone with `pnpm dlx wrangler@4 login`.
3. Deploy with `pnpm dlx wrangler@4 deploy --config apps/website/wrangler.jsonc`. On the first deploy, Wrangler attaches `orchbee.com` and `www.orchbee.com` as custom domains and creates their DNS records and certificates.
4. Check that `https://orchbee.com/zh-CN` redirects to `/zh-CN/`, and that an unknown address under `/zh-CN/` returns status 404 with the Chinese page.

Both host names serve the same files; canonical links send search engines to `https://orchbee.com`. The Worker has no `workers.dev` address and no preview addresses.

## Content and release state

`src/website/Website.tsx` composes one page: the home page or the not-found page. The home page implements the owner's design draft in eight scenes: the closed-window hero, one session in the terminal interface and the desktop app, agent status, orchestration through the CLI, privacy, the terminal core, installation, and the closing. Layout, type, color, motion, and tone follow the draft; the product is the truth boundary. A statement the product, the repository README, or the documentation does not support is removed rather than marked as planned; only work the README lists as in development, such as remote machines, carries a status tag.

Every product window shows a real capture from `resources/readme/`. `scripts/generate-website-pictures.mjs` at the repository root derives `public/pictures/` from them: WebP stills, crops for narrow screens, and VP9 WebM recordings with still posters. It may crop and scale but never redraws or composes, and `picture-sources.json` records the digest of each source it used. Run `pnpm --filter @orchbee/website pictures` after a capture changes; the build runs the generator with `--check` and fails when a source, an output, or a size no longer matches. A scene without a capture for its subject leaves the window out instead of drawing one.

Recordings stay muted and play only while in view; under reduced motion they wait on their poster until the visitor presses play, and the swarm stays still. The hero story and the header read their localized strings from a JSON attribute rendered with the page, so the script never holds a second catalog.

`src/website/release.ts` is the only release switch. While `release` is `{ published: false }`, every call to action offers the [GitHub repository](https://github.com/i-need-token/orchbee) and a source build. Once a release is published, its value names the version and the address of its Windows installer; the hero, install section, and closing then offer the installer and `npm install -g orchbee@next`, and point to the release notes.

`pnpm website:release <version>` (`scripts/website-release.mjs`) moves the download button and the update channel together, so they always name the same version. It fetches the published release's feed pair anonymously with the system curl, verifies the signature with `RELEASE_KEYS` before reading the feed, checks the feed against the reader's rules and the version, confirms the installer is public, and downloads the archive the feed names to check its size and SHA-256. Only then does it write the tracked pair and the `release` block; any failed check writes nothing. A draft or a missing asset answers 404 and is refused with what to do. Running it with an older version withdraws a bad release. `pnpm --filter @orchbee/website test` refuses a tracked pair that no release key signed or whose version differs from `release.version`.

`@orchbee/i18n` owns the `websiteMessages` descriptors and paired catalogs; English and Chinese are phrased independently. Commands stay in English in both languages. Locale URLs are explicit language choices, so switching uses ordinary links that work without JavaScript. New locales require matching static routes, catalogs, metadata, language navigation, 404 pages, and sitemap alternatives.

## Type and brand

Mona Sans and JetBrains Mono come from Fontsource packages and are served from the site itself; the build never inlines them, because the Content Security Policy loads fonts only from the site. Their notices are in the repository `THIRD_PARTY_NOTICES.md`.

The header, footer, hero orbit, closing mark, and favicon use `resources/icon.svg`; `favicon.ico`, `apple-touch-icon.png`, and `og.png` are copied from `resources/brand/`. The mark carries its own tile and corner radius, so do not crop, recolor, or round it again in page styles.

## Appearance and privacy

Appearance defaults to the system and responds to live system changes until a visitor picks light or dark with the header button, which cycles dark, light, and system. The blocking head script resolves appearance before paint. Without JavaScript, the complete content remains readable in the default dark theme.

The only stored value is a disposable JSON object `{ "appearance": "system" | "light" | "dark" }` under `orchbee:website:appearance:v1`. The composition entry injects storage; `src/preferences/site-preferences-storage.ts` owns strict validation and safe fallback. Missing, malformed, unavailable, and quota-rejected storage never prevents reading or changing the page. Defaults are not written; only explicit changes persist.

Production has no cookies, third-party fonts, scripts, or pictures, analytics, beacons, or telemetry. Its Content Security Policy forbids connections, forms, plugins, framing, and external resources. Hosting access logs are outside the website code.

## Verification

Run the affected root gate in [Verification](../../docs/VERIFICATION.md), plus `pnpm website:build` when changing static generation, fonts, or delivery. `pnpm --filter @orchbee/website test` checks appearance storage and the server-free rendering rules: every scene in both languages, same-origin references to generated pictures that exist, and the calls to action of each release state.

Review the generated site through `pnpm website:preview` in a real browser: full-page pictures at desktop and narrow widths in both languages and themes, each recording mid-run, the released state, and the 404 pages. Also check keyboard focus, the appearance button and live system changes, copy buttons, language links, reduced motion, disabled JavaScript, denied storage, and the absence of requests to other origins. These browser checks are manual evidence, not covered by the unit suite.
