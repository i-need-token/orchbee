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

## Codicons

Monaco ships its own Codicon font under the same `codicon` family as the product's
`@vscode/codicons` 0.0.45, and Monaco's is newer. Once the editor has loaded, both faces are
declared, and a glyph that both define can render from either, so a product icon may change
shape. Aligning the product's package with Monaco's font would leave one face for both.

## Size

Measured from the production build of `monaco-editor` 0.58.0 at admission; a change that grows
them reports the new sizes.

| Output                                           | Raw         | Gzip      |
| ------------------------------------------------ | ----------- | --------- |
| Editor core and features                         | 3,440,436 B | 874,392 B |
| Codicon registry                                 | 152,177 B   | 37,993 B  |
| Editor stylesheet                                | 156,817 B   | 22,737 B  |
| Monaco's Codicon font                            | 154,520 B   | 76,112 B  |
| Editor worker, loaded when a model needs it      | 200,137 B   | 64,566 B  |
| zh-CN message table, loaded only in zh-CN        | 96,957 B    | 26,219 B  |
| 81 grammar chunks, each loaded with its language | 422,967 B   | 148,741 B |
| Largest grammar chunk                            | 18,564 B    | 5,342 B   |
| Main bundle growth                               | 6,877 B     | 2,743 B   |

The main bundle grows because the added modules lengthen every module ID by a digit and the
loader's chunk map lists the new chunks; the File Bar's limit for it is 10 KB gzip.

## Upgrading Monaco

1. Pin the exact version in `apps/web/package.json`.
2. Compare the features and languages in the package's `esm/metadata.js` with `monaco-entry.ts`,
   and keep language services, the package root and `editor.main` out.
3. Move each zh-CN override in `@orchbee/i18n`, with its English in the override test, to the
   index that now carries that English.
4. Run a production build, load the editor in a Desktop window and confirm that the console
   reports no content security violation, then update the sizes above.
