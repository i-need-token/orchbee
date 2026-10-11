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

Publish only `apps/website/dist/` to an HTTPS static host. There is no request-time rendering, server process, account, analytics service, or reporting endpoint. React renders HTML during the build; the build renderer is removed from the output. Production JavaScript only enhances appearance, the mobile navigation disclosure, and the workspace illustration.

English lives at `/`; Simplified Chinese lives at `/zh-CN/`. Both URLs contain complete HTML, localized metadata, canonical and reciprocal language links, Open Graph and Twitter metadata, and WebSite structured data. The build emits `robots.txt` and `sitemap.xml` for `https://orchbee.com`. The host must serve directory indexes and return a real 404 for unknown paths rather than rewriting them to the home page. Deployment and DNS are separate operations.

## Content and visual ownership

`src/website/Website.tsx` owns the page narrative; its private `Showcase` owns the clearly labeled workspace illustration. Replace the illustration with approved product screenshots when they are available, keeping meaningful text outside images and maintaining an accessible description. Example tasks are illustrative, not product evidence. Early-development copy and links must be reviewed against the current product before release; do not advertise downloads or planned cross-device access as delivered.

Project calls to action point to the [GitHub repository](https://github.com/i-need-token/orchbee), which the project owner plans to make public. Keep that destination while public access is being prepared; check its availability before launch. Add download and documentation destinations when they are ready, updating both languages together.

`@orchbee/i18n` owns the `websiteMessages` descriptors and paired catalogs. Locale URLs are explicit language choices; switching uses ordinary document navigation so links remain crawlable and work without JavaScript. New locales require matching static routes, catalogs, metadata, language navigation, and sitemap alternatives.

The visual vocabulary combines restrained type hierarchy and product-led sections from [Linear](https://linear.app/) and [Cursor](https://cursor.com/), compact navigation from [Vercel](https://vercel.com/), and feature grouping from [Supabase](https://supabase.com/). [Granola](https://www.granola.ai/) and [Tailscale](https://tailscale.com/) inform light-theme clarity. Shared tokens own both theme mappings, brand accents, and display typography; CSS Modules own the page composition. Reference websites are inspiration, not asset sources.

## Appearance and privacy

Appearance defaults to the system and responds to live system changes until a visitor explicitly chooses light or dark. The blocking head script resolves appearance before paint. Without JavaScript, complete content remains readable in the default dark theme, with native language links, anchors, and FAQ disclosures.

The only stored value is a disposable JSON object `{ "appearance": "system" | "light" | "dark" }` under `orchbee:website:appearance:v1`. The composition entry injects storage; `src/preferences/site-preferences-storage.ts` owns strict validation and safe fallback. Missing, malformed, unavailable, and quota-rejected storage never prevents reading or changing the page. Defaults are not written; only explicit changes persist. The preference is local to this origin and is not synchronized across tabs.

Production has no cookies, third-party font or asset requests, analytics, beacons, or telemetry. Its Content Security Policy forbids connections, forms, plugins, and external resources. Hosting access logs are outside the website code; configure hosting separately to match the intended privacy policy.

## Verification

Run the affected root gate in [Verification](../../docs/VERIFICATION.md), plus `pnpm website:build` when changing static generation or assets. `pnpm --filter @orchbee/website test` checks appearance boundaries and server-free, bilingual content rendering.

Review the generated static site in a real browser: desktop and narrow mobile screenshots in both languages and themes; navigation, keyboard focus and Escape, all illustration selections, FAQ, language links, persisted appearance and live system changes. Also check 320px layouts, increased text size, reduced motion, disabled JavaScript, denied storage, direct locale loads, SEO artifacts, and the absence of third-party or reporting requests. These browser checks are manual evidence, not covered by the unit suite.
