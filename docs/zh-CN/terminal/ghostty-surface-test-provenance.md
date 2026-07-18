# Ghostty 终端面板符合性

## 范围

根据 Ghostty renderer、字体、shaper、搜索、surface 和相关 Swift 测试审计一个终端面板；排除由相同 WASM 产物提供的 VT 行为，也排除窗口、标签、拆分、更新、命令面板和其他应用 UI。

| 基线           | 值                                                                 |
| -------------- | ------------------------------------------------------------------ |
| 验证日期       | 2026-07-13                                                         |
| Ghostty commit | `91f66da24527fa02d92b5fd0b41cd020f553a64c`                         |
| Orchbee commit | `f13224bba131ad7cdebc2c7c4ddeca498da8c718` 加工作树审计            |
| WASM SHA-256   | `b0f39cfe981af36745c6b9c6919e9ac9cdf20aa507764e78bfb7a63ddc945c5b` |
| 产物比较       | 与 Ghostty `zig-out/bin/ghostty-vt.wasm` 字节完全相同              |
| 完整上游测试   | `src/` 下 3,343 个 Zig 声明；246 个 Swift 声明                     |
| 面板审计集合   | 261 个 Zig 声明加 13 个相关 Swift 声明                             |
| Orchbee 包测试 | 35 个文件中的 247 项测试                                           |

Zig 计数接受 `test {}`、`test name {}` 和 `test "name" {`；早期仅接受带引号和无名测试的计数并不完整。

## 处置分类

| 处置分类   | 数量 | 覆盖处理方式                                              |
| ---------- | ---: | --------------------------------------------------------- |
| 已覆盖     |  114 | 具有已跟踪 Orchbee 证据的适用声明                         |
| 能力缺口   |   52 | 终端面板缺失或不完整的适用可观察能力                      |
| 浏览器产品 |   70 | 由 `FontFaceSet`、Canvas 文本或 WebGPU 替代的准确原生算法 |
| 上游内部   |   24 | 没有对应物的 allocator 注入、原生缓存/容器或页面布局细节  |
| 应用层     |   14 | 配置解析、pasteboard 搜索 UI、传输 UI 或原生搜索焦点      |
| 总计       |  274 | 审计集合中的每个声明                                      |

按声明加权的适用覆盖率为 `114 / (114 + 52) = 68.7%`；这是测试来源指标，不是用户可见功能百分比。

## Orchbee 证据

| 代码       | 证据                                                                                               |
| ---------- | -------------------------------------------------------------------------------------------------- |
| `E-RENDER` | `src/rendering/__tests__/renderer.unit.test.ts`                                                    |
| `E-FIT`    | fit 测试和 `src/__tests__/surface-size.unit.test.ts`                                               |
| `E-SHAPE`  | `src/__tests__/text-shaping.unit.test.ts` 和 renderer 测试                                         |
| `E-SEARCH` | `src/search/__tests__/terminal-search.unit.test.ts` 和 composition 终端测试                        |
| `E-LINK`   | link-detector、link-provider、renderer 和 composition 终端测试                                     |
| `E-MOUSE`  | `src/input/__tests__/surface-mouse.unit.test.ts` 和 input-handler 测试                             |
| `E-IME`    | `src/input/__tests__/preedit.unit.test.ts` 和 input-handler 测试                                   |
| `E-SELECT` | `src/selection/__tests__/selection-manager.unit.test.ts`                                           |
| `E-FONT`   | font-metrics、glyph-constraints、Ghostty sprite、renderer、generated-table 和 composition 终端测试 |

下方命名的每个声明都具有对应行中的处置分类和证据；`<unnamed@line>` 标识基线 commit 中的无名上游测试。

## Renderer 清单

- `src/renderer/State.zig` — 已覆盖（`E-RENDER`）：`preedit range covers exact cell width`；`preedit range shifts left at right edge`。
- `src/renderer/cell.zig` — 已覆盖（`E-RENDER`）：`Contents`；`Contents clear retains other content`；`Contents clear last added content`；`Contents with zero-sized screen`。
- `src/renderer/cell.zig` — 已覆盖（`E-FONT`）：`Cell constraint widths`。
- `src/renderer/cursor.zig` — 已覆盖（`E-RENDER`）：`cursor: default uses configured style`；`cursor: blinking disabled`；`cursor: explicitly not visible`；`cursor: always block with preedit`。
- `src/renderer/link.zig` — 已覆盖（`E-LINK`）：`renderCellMap`；`renderCellMap hover links`；`renderCellMap mods no match`。
- `src/renderer/metal/shaders.zig` — 浏览器产品：`<unnamed@283>`；WebGPU shader 编译替代 Metal shader 编译。
- `src/renderer/shadertoy.zig` — 能力缺口：`spirv`；`spirv invalid`；`shadertoy to msl`；`shadertoy to glsl`。
- `src/renderer/size.zig` — 已覆盖（`E-FIT`）：`Size.balancePadding equal distributes whitespace equally`；`Size.balancePadding true shifts excess top to bottom`；`Padding balanced on zero`。
- `src/renderer/size.zig` — 已覆盖（`E-FIT`、`E-SELECT`）：`GridSize update exact`；`GridSize update rounding`；`coordinate conversion`。

## 字体清单

- `src/font/Atlas.zig` — 已覆盖（`E-RENDER`）：`happy path`；`exact fit`；`doesn't fit`；`fit multiple`；`writing data`；`grow`；`writing BGR data`；`grow BGR`。
- `src/font/Atlas.zig` — 浏览器产品：`writing data from a larger source`；Canvas 栅格化提供源裁剪。
- `src/font/Atlas.zig` — 上游内部：`grow OOM`；`init error`；`reserve error`；`grow error`；Zig allocator 故障注入没有浏览器等价物。
- `src/font/CodepointMap.zig` — 浏览器产品：`codepointmap`。
- `src/font/CodepointResolver.zig` — 浏览器产品：`getIndex`；`getIndex disabled font style`。
- `src/font/CodepointResolver.zig` — 能力缺口：`getIndex box glyph`；Orchbee 路由有限 sprite 子集，但没有实现完整 Ghostty sprite-face resolver。
- `src/font/Collection.zig` — 上游内部：`<unnamed@929>`。
- `src/font/Collection.zig` — 浏览器产品：`init`；`add full`；`add deferred without loading options`；`getFace`；`getIndex`；`completeStyles`；`hasCodepoint`；`hasCodepoint emoji default graphical`；`face metrics`。
- `src/font/Collection.zig` — 已覆盖（`E-FONT`）：`setSize`；`metrics`。
- `src/font/Collection.zig` — 已覆盖（`E-FONT`）：`adjusted sizes`。
- `src/font/DeferredFace.zig` — 浏览器产品：`fontconfig`；`coretext`。
- `src/font/Glyph.zig` — 已覆盖（`E-FONT`）：`Constraints`。
- `src/font/Metrics.zig` — 应用层：`formatConfig percent`；`formatConfig absolute`；`Modifier: parse absolute`；`Modifier: parse percent`。
- `src/font/Metrics.zig` — 已覆盖（`E-FONT`）：`Metrics: apply modifiers`；`Metrics: adjust cell height smaller`；`Metrics: adjust cell height larger`；`Metrics: adjust icon height by percentage`；`Metrics: adjust icon height by absolute pixels`；`Modifier: percent`；`Modifier: absolute`。
- `src/font/SharedGrid.zig` — 已覆盖（`E-FONT`）：`getIndex`；`renderGlyph error after cache insert rolls back cache entry`。
- `src/font/SharedGrid.zig` — 上游内部：`init error`。
- `src/font/SharedGridSet.zig` — 已覆盖（`E-FONT`）：`Key`；`Key different font points`；`Key different font DPI`。
- `src/font/SharedGridSet.zig` — 上游内部：`SharedGridSet`。
- `src/font/discovery.zig` — 浏览器产品：`descriptor hash`；`descriptor hash family names`；`fontconfig`；`fontconfig codepoint`；`coretext`；`coretext codepoint`；`coretext sorting`；`windows`。
- `src/font/face.zig` — 上游内部：`<unnamed@96>`。
- `src/font/face.zig` — 能力缺口：`Variation.Id: wght should be 2003265652`；`Variation.Id: slnt should be 1936486004`；未公开任意字体 variation。
- `src/font/face/coretext.zig` — 浏览器产品：`<unnamed@970>`；`name`；`emoji`；`in-memory`；`variable`；`variable set variation`；`svg font table`；`glyphIndex colored vs text`。
- `src/font/face/freetype.zig` — 浏览器产品：`face name`；`<unnamed@1111>`；`color emoji`；`mono to bgra`；`svg font table`；`bitmap glyph`；`bitmap glyph BDF`；`bitmap glyph PCF`；`bitmap glyph OTB`。
- `src/font/glyf_rasterize.zig` — 能力缺口：`<unnamed@457>`；`glyf_rasterize: empty outline returns empty bitmap`；`glyf_rasterize: square fills bitmap center`；`glyf_rasterize: quadratic contour renders`；`glyf_rasterize: consecutive off-curve points render`；`glyf_rasterize: units per em controls baseline scale`；`glyf_rasterize: degenerate outline returns full empty bitmap`；`glyf_rasterize: contour can start off curve with final on curve point`；`glyf_rasterize: contour can start with implied midpoint`；`glyf_rasterize: multiple contours render independently`；`glyf_rasterize: non-zero bearings preserve declared whitespace`；`glyf_rasterize: negative y coordinates descend below baseline`；`glyf_rasterize: two-cell bitmap and constraint render within width`；`glyf_rasterize: line height does not change unconstrained em scale`。
- `src/font/glyf_rasterize_png_test.zig` — 能力缺口：`glyf_rasterize: bubbletea glyph protocol examples match reference image`；打包 WASM 中禁用 Ghostty glyph graphics，renderer 也不支持。
- `src/font/main.zig` — 上游内部：`<unnamed@75>`。
- `src/font/opentype.zig` — 上游内部：`<unnamed@17>`。
- `src/font/opentype/glyf.zig` — 浏览器产品：`glyf`；`glyf: decode triangle`；`glyf: decode multiple contours`；`glyf: decode repeat and short vector flags`；`glyf: decode off curve and same coordinate flags`；`glyf: decode one-point contour`；`glyf: decode contour ending at max point index`；`glyf: reject glyphs with instructions and composite glyphs`；`glyf: reject truncated`；`glyf: reject endpoints out of order`；`glyf: reject too many points`；`glyf: zero-contour glyph can be header-only`；`glyf: zero-contour glyph can include instruction length`；`glyf: zero-contour glyph rejects instructions`。
- `src/font/opentype/head.zig` — 浏览器产品：`head`。
- `src/font/opentype/hhea.zig` — 浏览器产品：`hhea`。
- `src/font/opentype/os2.zig` — 浏览器产品：`OS/2`。
- `src/font/opentype/post.zig` — 浏览器产品：`post`。
- `src/font/opentype/sfnt.zig` — 浏览器产品：`FixedPoint`；`parse font`；`get table`。
- `src/font/opentype/svg.zig` — 浏览器产品：`SVG`。
- `src/font/shape.zig` — 上游内部：`<unnamed@96>`。
- `src/font/shaper/Cache.zig` — 已覆盖（`E-FONT`）：`Cache`。
- `src/font/shaper/coretext.zig` — 已覆盖（`E-SHAPE`、`E-FONT`）：`shape inconsolata ligs`；`shape monaspace ligs`；`shape emoji width`；`shape emoji width long`；`shape Combining characters`；`shape selection boundary`；`shape cursor boundary`；`shape cursor boundary and colored emoji`；`shape cell attribute change`；`shape nerd fonts`。
- `src/font/shaper/coretext.zig` — 浏览器产品：`shape`；`shape U+3C9 with JB Mono`；`shape variation selector VS15`；`shape variation selector VS16`。
- `src/font/shaper/coretext.zig` — 能力缺口：`run iterator`；`run iterator: empty cells with background set`；`shape left-replaced lig in last run`；`shape left-replaced lig in early run`；`shape with empty cells in between`；`shape Devanagari string`；`shape Tai Tham vowels (position differs from advance)`；`shape Tai Tham letters (position.y differs from advance)`；`shape Javanese ligatures`；`shape Chakma vowel sign with ligature (vowel sign renders first)`；`shape Bengali ligatures with out of order vowels`；`shape box glyphs`；`shape high plane sprite font codepoint`。
- `src/font/shaper/feature.zig` — 能力缺口：`Feature.fromString`；`FeatureList.fromString`；只公开字体连字布尔值。
- `src/font/shaper/harfbuzz.zig` — 已覆盖（`E-SHAPE`）：`shape inconsolata ligs`；`shape monaspace ligs`；`shape emoji width`；`shape emoji width long`；`shape Combining characters`；`shape selection boundary`；`shape cursor boundary`；`shape cursor boundary and colored emoji`；`shape cell attribute change`。
- `src/font/shaper/harfbuzz.zig` — 浏览器产品：`shape`；`shape variation selector VS15`；`shape variation selector VS16`。
- `src/font/shaper/harfbuzz.zig` — 能力缺口：`run iterator`；`run iterator: empty cells with background set`；`shape arabic forced LTR`；`shape with empty cells in between`；`shape Devanagari string`；`shape Tai Tham vowels (position differs from advance)`；`shape Tibetan characters`；`shape Tai Tham letters (run_offset.y differs from zero)`；`shape Javanese ligatures`；`shape Chakma vowel sign with ligature (vowel sign renders first)`；`shape Bengali ligatures with out of order vowels`；`shape box glyphs`。
- `src/font/shaper/noop.zig` — 上游内部：`<unnamed@141>`。
- `src/font/sprite.zig` — 上游内部：`<unnamed@37>`；`<unnamed@43>`。
- `src/font/sprite/Face.zig` — 能力缺口：`sprite face render all sprites`；`full height cursor sprites respect cursor height metric`；`<unnamed@664>`。Orchbee 以过程方式渲染有限的 box、block、triangle 和 Powerline 子集，但不支持所有 sprite 或 sprite cursor。

## 搜索清单

- `src/terminal/search/Thread.zig` — 上游内部：`<unnamed@848>`；Orchbee 搜索是同步的，没有由 worker 拥有的队列。
- `src/terminal/search/active.zig` — 已覆盖（`E-SEARCH`）：`simple search`；`clear screen and search`。
- `src/terminal/search/pagelist.zig` — 已覆盖（`E-SEARCH`）：`simple search`；`feed multiple pages with matches`；`feed multiple pages no matches`；`feed iteratively through multiple matches`；`feed with match spanning page boundary`；`feed with match spanning page boundary with newline`。
- `src/terminal/search/pagelist.zig` — 上游内部：`feed with pruned page`。
- `src/terminal/search/screen.zig` — 已覆盖（`E-SEARCH`）：`simple search`；`simple search with history`；`reload active with history change`；`active change contents`；`select next`；`select in active changes contents completely`；`select into history`；`select prev`；`select prev then next`；`select prev with history`；`select prev wraps when all matches are in history`；`select after all matches disappear drops the selection`；`screen search no scrollback has no history`；`select after clearing scrollback`。
- `src/terminal/search/screen.zig` — 上游内部：`reloadActive partial history cleanup on appendSlice error`；`reloadActive partial history cleanup on loop append error`。
- `src/terminal/search/sliding_window.zig` — 已覆盖（`E-SEARCH`）：`SlidingWindow empty on init`；`SlidingWindow single append`；`SlidingWindow single append case insensitive ASCII`；`SlidingWindow single append single char`；`SlidingWindow single append no match`；`SlidingWindow two pages`；`SlidingWindow two pages single char`；`SlidingWindow two pages match across boundary`；`SlidingWindow two pages no match across boundary with newline`；`SlidingWindow two pages no match across boundary with newline reverse`；`SlidingWindow single append match on boundary`；`SlidingWindow single append reversed`；`SlidingWindow single append no match reversed`；`SlidingWindow two pages reversed`；`SlidingWindow two pages match across boundary reversed`；`SlidingWindow single append match on boundary reversed`；`SlidingWindow single append soft wrapped`；`SlidingWindow single append reversed soft wrapped`；`SlidingWindow append whitespace only node`。
- `src/terminal/search/sliding_window.zig` — 上游内部：`SlidingWindow two pages no match prunes first page`；`SlidingWindow two pages no match keeps both pages`；`SlidingWindow single append across circular buffer boundary`；`SlidingWindow two pages no match prunes first page reversed`；`SlidingWindow two pages no match keeps both pages reversed`；`SlidingWindow single append across circular buffer boundary reversed`。
- `src/terminal/search/viewport.zig` — 已覆盖（`E-SEARCH`）：`simple search`；`clear screen and search`；`clear screen and search dirty tracking`；`history search, no active area`。

## Surface 和 Swift 清单

- `src/surface_mouse.zig` — 已覆盖（`E-MOUSE`）：`keyToMouseShape`，包括其在 macOS 和其他平台上的 13 个状态表场景。
- `src/apprt/gtk/class/surface.zig` — 应用层：`computeFraction`；这是 GTK 文件传输进度。
- `macos/Tests/Ghostty/Surface View/SurfaceView+SearchStateTests.swift` — 应用层：`init_withNilNeedle_readsPasteboardNeedle`；`init_withEmptyNeedle_readsPasteboardNeedle`；`init_withNeedle_setsNeedle`；`init_withNeedle_writesPasteboard`；`writePasteboardNeedle_writesPasteboard`；`readPasteboardNeedle_whenPasteboardNeedleIsNil`；`readPasteboardNeedle_whenPasteboardNeedleIsValid`；`readPasteboardNeedle_setsNeedleSelectionRange`。
- `macos/Tests/Ghostty/SurfaceViewAppKitTests.swift` — 已覆盖（`E-IME`）：`suppressesOnlySingleC0ControlTextWhileComposing`；`doesNotSuppressControlTextWhenNotComposing`；`doesNotSuppressMissingText`。参数化声明展开为八种情况，因此总共十种运行时情况均有体现。
- `macos/GhosttyUITests/GhosttyMouseStateTests.swift` — 已覆盖（`E-SELECT`）：`testSelectionFocusChange`。
- `macos/GhosttyUITests/GhosttyMouseStateTests.swift` — 应用层：`testSearchFocusState`。

## 剩余能力缺口

| 能力                          | 缺失声明数 | 当前限制                                                  |
| ----------------------------- | ---------: | --------------------------------------------------------- |
| Shadertoy/自定义 shader       |          4 | 没有 shader API 或 SPIR-V 转换                            |
| 字体 variation 和任意 feature |          4 | 只有字体系列、尺寸、样式 flag 和连字开关                  |
| Glyph graphics 栅格化         |         15 | 打包 WASM 和 renderer 声明不支持 glyph graphics           |
| 通用复杂文字 shaping run      |         26 | 只实现有限编程连字和逐 grapheme Canvas 栅格化             |
| 内置 Ghostty sprite 字体      |          3 | 存在有限过程子集；不受支持的范围和 sprite cursor 仍然缺失 |
| 总计                          |         52 |                                                           |

Kitty graphics 同样缺失，因为 Ghostty 对 `wasm32-freestanding` 禁用了它；范围内没有上游 renderer/font 声明衡量这个额外能力缺口。

## 升级验证

1. 将目标 Ghostty commit 和打包 WASM SHA-256 与本基线比较。
2. 在 Ghostty 检出于 `../ghostty` 时从仓库根目录运行 `pnpm audit:ghostty-panel`，或把 `GHOSTTY_SOURCE` 设为其根目录。
3. 上游 Nerd Font 表变化时运行 `pnpm generate:ghostty-panel-data`；审计会拒绝过期的生成输出。
4. 审计必须枚举 `test {}`、`test name {}` 和 `test "name" {` 语法，并拒绝缺失、重复或过期映射。
5. 只移植 Orchbee 所拥有行为的断言；保留浏览器产品、内部和应用排除项，并提供明确理由。
6. 运行单元测试、Node 集成测试、浏览器 surface 测试、类型检查、构建、lint、格式和产物检查。
