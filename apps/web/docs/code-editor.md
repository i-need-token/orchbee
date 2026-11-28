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

A file opens in the language its name selects, by VS Code's association rule and never from its
content. Monaco declares JSON only in its language service, which the editor leaves out, so
`registerJsonLanguages` declares JSON, JSON with Comments and JSON Lines with a tokenizer of its
own.

## Documents and views

`loadCodeEditor` resolves to the documents and views together with Monaco, so they load with the
editor rather than in the main bundle.

A document holds one file's content, and every tab that shows the file in the same editor shares it
through its Workspace's `CodeEditorFileDocuments`; the last tab to let go disposes of it.

A view keeps one editor for as long as it is mounted and moves it between documents, so switching
tabs creates no editor. Each file's scroll and selection outlive its tab, up to VS Code's 100 files.
A tab's owner may dispose of a document before the view unmounts, so a view saves the view state
when its model is about to be disposed of. The diff editor treats a side disposed of while it still
compares it as a bug, so the diff view lets go of both sides first.

## Keys, Actions and menu

Monaco's public API neither lists its key bindings nor evaluates their conditions. The editor's
keyboard scope therefore yields the single keys that `codeEditorKeyChords.ts` holds for each
operating system, as the pinned package's default keybinding registry declares them, so no wider
scope takes a key Monaco would handle. `Mod+K` never reaches Monaco, so VS Code's `Mod+K` editor
keys are Workbench sequences in the same scope, and each runs one of the editor's commands as an
Action.

The editor registers those Actions in a scope of its own from `getSupportedActions()`, which lists
only the actions whose precondition holds, and reads the list again as it takes the keys and before
each key or press reaches the Workbench. A key thus runs a command only where the editor offers it
then, as a VS Code key binding's condition includes its action's precondition, and Action search
lists the same Actions while the editor has or last had the keys, as VS Code's command palette lists
the focused editor's actions. The composition leaves out Monaco's command palette and Go to Line, so
Monaco binds neither F1 nor `Ctrl+G`; F1 opens Action search inside the File Bar and the overlay
instead.

Monaco keeps one font zoom for every editor on the page, so Increase, Decrease and Reset Editor Font
Size are Workbench Actions that exist once, and Monaco's own zoom actions stay out of Action search.

Monaco's context menu stays off, so the editor's menu looks and behaves as the product's other
menus do. The overlay shows VS Code's editor context menu with the items the editor offers; a parity
test holds it to VS Code's menu at the pinned commit and names each item it leaves out.

## Multi-diff

`CodeEditorMultiDiffView` lists many files' diffs in one scrolling view, laid out as VS Code's
multi-diff editor lays them out: files follow one another, each under a header that collapses it,
and none renders taller than the view. While the view scrolls through a taller file, the file stays
in sight and its diff editor scrolls instead, so no editor grows taller than the screen.

Only the files in sight hold a slot. Each slot keeps its diff editor and moves it between files, so
scrolling creates no editor beyond the most files the view has shown at once, and a file keeps its
computed diff while it stays listed, so a file that scrolls back does not compute it again. Until
its diff is computed, a file counts as VS Code's estimate of 500 pixels, and a placeholder counts as
100 below its header. A file wholly above the view that changes height moves the offset, so the
files in sight stay where they are.

As in VS Code's multi-diff, the files share one sideways offset, which the files in sight bound:
the view's scrollbar reaches as far as the widest of them overflows, and in each diff the side that
overflows further takes the offset while the diff editor scrolls the other with it. Unlike VS Code,
a diff editor that scrolls itself sideways, as to show its cursor, moves the shared offset, so every
file follows it rather than the editor jumping back when the view next scrolls.

The wheel scrolls the view up and down as the Sidebar's lists scroll and sideways as VS Code's
scrollable elements do, and moves the view and the editors in sight in the same frame; a natively
scrolled view would show a pinned file's editor a frame behind. Hovers and other widgets that
overflow an editor render in one layer over the view, outside every file's frame.

The tab's owner draws Collapse All Diffs and Expand All Diffs from a handle the view gives it; both
keep the file at the top of the view at the top. The owner also names the key the view state is kept
under. The view state places the view by the file at its top, so a tab returns to the same file
after files are added above it, and keeps each file's collapse, selections and measured height.
VS Code keeps those heights in a view model that outlives the view; here the view state carries
them, so a returning tab measures no file above the view again. A sideways offset waits until a file
in sight is wide enough to reach it, and scrolling first gives it up, as VS Code gives up a scroll
restore that the person overtakes.

## Image preview

Images show from `data:` URLs, because the production policy's `img-src` admits `data:` but not
`blob:`. As in VS Code, the preview reads the whole file whatever its size, and a reload that fails
keeps the image it shows.

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
| Theme, JSON languages, documents and views       | 39,484 B    | 12,398 B    |
| Views' stylesheet                                | 6,086 B     | 1,672 B     |
| Main bundle growth                               | 13,248 B    | 4,451 B     |
| Main stylesheet growth                           | 5,353 B     | 828 B       |
| Production web build growth                      | 4,690,264 B | 1,271,379 B |

The main bundle grows because the added modules lengthen every module ID by a digit, the loader's
chunk map lists the new chunks, and `@orchbee/i18n` carries the editor's messages with every other
domain's; the File Bar's limit for it is 10 KB gzip. The main stylesheet grows by the syntax roles
of both appearances and every named theme. The views' stylesheet carries a copy of the virtual
tree's, which the multi-diff's import of the tree's scrolling arithmetic brings along.

## Upgrading Monaco

1. Pin the exact version in `apps/web/package.json`.
2. Compare the features and languages in the package's `esm/metadata.js` with `monaco-entry.ts`,
   and keep language services, the package root and `editor.main` out.
3. Move each zh-CN override in `@orchbee/i18n`, with its English in the override test, to the
   index that now carries that English.
4. Confirm that each product icon's name keeps its code point in the package's
   `esm/vs/base/common/codiconsLibrary.js`.
5. Read the default key bindings again. Bundle a page that imports the features `monaco-entry.ts`
   imports and reads `KeybindingsRegistry.getDefaultKeybindings()` from the package's
   `platform/keybinding/common/keybindingsRegistry`, and load it under a macOS, a Windows and a
   Linux user agent, since Monaco takes the operating system from it. Update `codeEditorKeyChords.ts`
   by the rules its comment states, and confirm that each `code-editor` row in
   `WORKBENCH_SCOPED_SHORTCUTS` still names the command Monaco binds to its `Mod+K` key.
6. Run a production build, load the editor in a Desktop window and confirm that the console
   reports no content security violation, then update the sizes above.
