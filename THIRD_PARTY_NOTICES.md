# Third-Party Notices

Orchbee includes the following third-party software and visual assets. Their inclusion does not
imply endorsement by their creators.

## Ghostty VT

- Copyright notice: Copyright (c) 2024 Mitchell Hashimoto, Ghostty contributors
- Source: <https://github.com/ghostty-org/ghostty>
- Pinned revision: `9c3ec931d64561a8407dde7ac984ce156ae91539`
- License: MIT
- Use in Orchbee: terminal emulation for native and WebAssembly runtimes

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Herdr Agent Screen Detection Rules

- Creator: Herdr contributors; the upstream license names no separate copyright holder
- Source: <https://github.com/herdrdev/herdr>
- Pinned revision: `e0507237ad8d41f60aece10d00ef29ce2339ca34` (Herdr 0.9.1)
- Original paths: `src/detect/manifests/*.toml` and `src/detect/manifest.rs`
- License: Apache License 2.0
- Use in Orchbee: bundled Agent screen detection rules in
  [`apps/orchbee-server/resources/agent-detection/`](apps/orchbee-server/resources/agent-detection/),
  and the prompt-region matching adapted in
  [`apps/orchbee-server/src/agent_screen/regions.rs`](apps/orchbee-server/src/agent_screen/regions.rs)
- Modifications: Orchbee owns local rule corrections, its parser, and its sampling integration;
  `grok.toml` adds browser-authentication blockers and an uncertain-login guard; region matching
  uses UTF-8-safe CRLF offsets
- License text and notice: [`LICENSE`](apps/orchbee-server/resources/agent-detection/LICENSE) and
  [`NOTICE`](apps/orchbee-server/resources/agent-detection/NOTICE)

## VS Code Terminal Colors

- Copyright notice: Copyright (c) 2015 - present Microsoft Corporation
- Source: <https://github.com/microsoft/vscode>
- Pinned revision: `c647c1cfa2c2a4b1bbf12cb4db413fe05a0d9044`
- Original path: `src/vs/workbench/contrib/terminal/common/terminalColorRegistry.ts`
- License: MIT
- Use in Orchbee: the light and dark terminal ANSI 0–15 values, unmodified, of the Orchbee Light and
  Orchbee Dark themes in [`crates/color-themes/themes/`](crates/color-themes/themes/)

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## iTerm2 Color Schemes

- Copyright notice: Copyright (c) 2011 to Present Mark Badolato
- Source: <https://github.com/mbadolato/iTerm2-Color-Schemes>
- Pinned revision: `38bd77ce06b376f2e49e78bd318cf1255d87719b`
- Original paths: the Ghostty exports under `ghostty/` that a theme file names as its source
- License: MIT; the upstream states that the copyright and license of each theme remain with its
  author, credited in that theme's own entry below
- Use in Orchbee: the published terminal color values of the themes in
  [`crates/color-themes/themes/`](crates/color-themes/themes/) whose source is this compilation

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Gruvbox Color Theme

- Creator: Pavel Pertsev; the upstream states the MIT/X11 license and names no separate copyright line
- Source: <https://github.com/morhetz/gruvbox>
- Pinned revision: `ef8864bb42bf244f0295d1c5a403b27e3d139695`
- Values from: iTerm2 Color Schemes `ghostty/Gruvbox Dark` and `ghostty/Gruvbox Light`
- License: MIT
- Use in Orchbee: terminal and interface color values of the Gruvbox Dark and Gruvbox Light themes
  in [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Modifications: lightness correction to the legibility floors of Gruvbox Dark ANSI 1 and Gruvbox
  Light ANSI 2, 3, and 6; each theme file records the original value beside the corrected one

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Rosé Pine Color Theme

- Copyright notice: Copyright (c) Rosé Pine
- Source: <https://github.com/rose-pine/ghostty>
- Pinned revision: `d8b4ec06e0cde80fb2bfbf59021ff0a332c77388`
- Original paths: `dist/rose-pine` and `dist/rose-pine-dawn`
- License: MIT
- Use in Orchbee: terminal and interface color values of the Rosé Pine and Rosé Pine Dawn themes in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Modifications: lightness correction to the legibility floor of Rosé Pine Dawn ANSI 3 and 6; the
  theme file records the original value beside the corrected one

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## JetBrains Mono

- Copyright notice: Copyright 2020 The JetBrains Mono Project Authors
- Source: <https://github.com/JetBrains/JetBrainsMono>
- Version: 2.304
- License: SIL Open Font License 1.1
- Use in Orchbee: unmodified variable terminal text faces provisioned for Browser and Desktop
- License text: [`JetBrainsMono-OFL.txt`](apps/web/src/workbench-client/projected-terminal-renderer/fonts/JetBrainsMono-OFL.txt)

## Nerd Fonts Symbols Only

- Copyright notice: Copyright (c) 2014 Ryan L McIntyre
- Source: <https://github.com/ryanoasis/nerd-fonts>
- Version: 3.4.0
- License: MIT
- Use in Orchbee: unmodified fallback terminal-symbol face provisioned for Browser and Desktop
- License text: [`SymbolsNerdFont-LICENSE.txt`](apps/web/src/workbench-client/projected-terminal-renderer/fonts/SymbolsNerdFont-LICENSE.txt)

## Codicons

- Creator: Microsoft Corporation and contributors
- Source: <https://github.com/microsoft/vscode-codicons>
- License: [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)
- Use in Orchbee: unmodified glyphs rendered with Orchbee theme colors and sizes

Codicons is licensed separately from Orchbee. The upstream copyright and license remain with their respective owners.

## Lobe Icons

- Copyright notice: Copyright (c) 2023 LobeHub
- Source: <https://github.com/lobehub/lobe-icons>
- Package and revision: `@lobehub/icons-static-svg@1.94.0`, `fbd2d56e3f734e889f1373e71c8368cc4e60e0d7`
- License: MIT
- Use in Orchbee: adapted SVG path data for the Claude Code, Cline, Codex, Gemini CLI, GitHub Copilot, Grok, OpenCode, Pi, and Qwen Code Agent identity icons

The product names and trademarks represented by these icons remain the property of their respective owners. Their descriptive use does not imply endorsement.

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## OMP Favicon

- Copyright notice: Copyright (c) 2025 Mario Zechner; Copyright (c) 2025-2026 Can Bölük; Copyright (c) 2026 Stencil Labs, Inc.
- Source: <https://omp.sh/favicon.svg>
- Source snapshot: fetched 2026-08-29, SHA-256 `9419975a0c24961341221c4cec18703db26a989fa037768f92cda74e3769fe05`
- License: MIT
- Use in Orchbee: adapted SVG path and gradient data for the OMP Agent identity icon

The OMP product name and trademark remain the property of their respective owners. Its descriptive use does not imply endorsement.

> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
> associated documentation files (the "Software"), to deal in the Software without restriction,
> including without limitation the rights to use, copy, modify, merge, publish, distribute,
> sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or
> substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
> NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
> NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
> DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT
> OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.
