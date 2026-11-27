# Code editor

`src/code-editor/` owns Monaco. It is the only module that imports `monaco-editor`, and the rest of
the application reaches the editor only through its lazy entry, so the editor loads as chunks of
its own and never in the main bundle. Once a window shows the [File Bar](workbench.md#release-withheld-file-bar),
those chunks load at idle, so the first file a person opens does not wait for them.

## Worker and content security

Monaco computes diffs and links in its editor worker. Left to itself, it starts that worker from a
`blob:` URL, which the production policy's `script-src 'self'` refuses. `MonacoEnvironment.getWorker`
starts it instead as a same-origin module worker that the build emits as its own chunk. No
language worker is bundled. The worker's fallback URL inside Monaco still makes the build emit a
158-byte `editorWebWorkerMain` asset that nothing loads.

## Language

Monaco's messages, services and worker environment are page-wide, and its modules read their
messages while they evaluate. The language of the first load therefore stays until the page
reloads. A zh-CN load first installs the pinned package's own zh-CN table and writes
`zhCNMonacoMessageOverrides` from `@orchbee/i18n` over it.

## Theme

Monaco's theme is page-wide, so one theme serves every editor and follows the document's: it is
defined when the editor loads and again whenever `data-theme` or `data-color-theme` changes on the
root element, without a reload. Monaco parses only hex colors, so the system tokens are read through
a 1×1 canvas, which resolves `oklch()` and `color-mix()` into sRGB bytes. Monaco takes token colors
as opaque, so text colors are painted over the canvas color first; fills keep their alpha and layer
as the page's do. The syntax roles color the tokens of every language, and the theme inherits no
rules from Monaco's own, whose colors would show through every named theme. Colors the theme leaves
out keep Monaco's defaults for the polarity.

## File languages

Monaco declares JSON only in its language service, which the editor leaves out, so
`registerJsonLanguages` declares JSON, JSON with Comments and JSON Lines with a tokenizer of its
own.

## Codicons

Monaco ships a newer Codicon font than the product's `@vscode/codicons` 0.0.45 under the same
`codicon` family, and its stylesheet sizes every codicon on the page. The web build therefore
rewrites that stylesheet: Monaco's font becomes `monaco-codicon`, and its codicon rule applies only
under `.monaco-editor`, `.monaco-diff-editor` and `.monaco-component`, the roots Monaco themes.
Once the editor loads, product icons keep the product's font and sizes, while Monaco's icons keep
the newer font, which draws 117 code points the product's lacks. The build fails when Monaco's
stylesheet no longer matches the rewrite.

Monaco's icon rules, written at runtime, still give every element named for a codicon Monaco's
code point for that name, product icons included, so a product icon keeps its glyph only while both
packages agree on its code point. In 0.58.0 every product icon does; among the names both define,
only `git-fetch` and `vm-pending` differ. Menus that Monaco draws in a shadow root still name
`codicon` and take their marks from the product's font, which has them.

## Size

Measured from production builds with `monaco-editor` 0.58.0, against the build without the editor;
a change that grows them reports the new sizes.

| Output                                           | Raw         | Gzip        |
| ------------------------------------------------ | ----------- | ----------- |
| Editor core and features                         | 3,440,439 B | 874,395 B   |
| Codicon registry                                 | 152,177 B   | 37,993 B    |
| Editor stylesheet                                | 156,892 B   | 22,758 B    |
| Monaco's Codicon font                            | 154,520 B   | 76,112 B    |
| Editor worker, loaded when a model needs it      | 200,137 B   | 64,566 B    |
| zh-CN message table, loaded only in zh-CN        | 96,957 B    | 26,219 B    |
| 81 grammar chunks, each loaded with its language | 422,967 B   | 148,741 B   |
| Largest grammar chunk                            | 18,564 B    | 4,463 B     |
| Theme and JSON languages, loaded with the editor | 9,155 B     | 2,956 B     |
| Main bundle growth                               | 7,536 B     | 3,057 B     |
| Main stylesheet growth                           | 5,154 B     | 795 B       |
| Production web build growth                      | 4,647,938 B | 1,258,837 B |

The main bundle grows because the added modules lengthen every module ID by a digit and the
loader's chunk map lists the new chunks; the File Bar's limit for it is 10 KB gzip. The main
stylesheet grows by the syntax roles of both appearances and every named theme.

## Upgrading Monaco

1. Pin the exact version in `apps/web/package.json`.
2. Compare the features and languages in the package's `esm/metadata.js` with `monaco-entry.ts`,
   and keep language services, the package root and `editor.main` out.
3. Move each zh-CN override in `@orchbee/i18n`, with its English in the override test, to the
   index that now carries that English.
4. Confirm that each product icon's name keeps its code point in the package's
   `esm/vs/base/common/codiconsLibrary.js`.
5. Run a production build, load the editor in a Desktop window and confirm that the console
   reports no content security violation, then update the sizes above.
