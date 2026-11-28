# Third-Party Notices

Orchbee includes the following third-party software and visual assets. Their inclusion does not
imply endorsement by their creators.

This list covers what the repository carries outside the production dependencies that Cargo and
pnpm resolve: vendored, adapted and prebuilt software, the libraries compiled into the Terminal
Host, fonts, color values, icons and artwork. Every staged set of native programs and every Desktop
application carries a generated `THIRD_PARTY_NOTICES.txt` that adds the crates, npm packages and
toolchain runtimes it contains, with their license texts.

[`scripts/third-party-notices.mjs`](scripts/third-party-notices.mjs) reads each section. Its
`Carried by` item names `native programs`, `Desktop application` or `website`, each optionally
narrowed by ` on Windows`, ` on macOS` or ` on Linux` and by ` in development and test builds`.
Its license text is its quoted text and the files its `License text` item links, unless that item
reads `none;` and the reason.

## Ghostty VT

- Copyright notice: Copyright (c) 2024 Mitchell Hashimoto, Ghostty contributors
- Source: <https://github.com/ghostty-org/ghostty>
- Pinned revision: `9c3ec931d64561a8407dde7ac984ce156ae91539`
- License: MIT
- Use in Orchbee: terminal emulation in the Terminal Host
- Carried by: native programs

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

## simdutf

- Copyright notice: Copyright 2021 The simdutf authors
- Source: <https://github.com/simdutf/simdutf>, through the copy in Ghostty's `pkg/simdutf/vendor/`
- Version: 9.0.0
- License: MIT, chosen from Apache-2.0 or MIT
- Use in Orchbee: the Unicode validation and transcoding that Ghostty VT compiles into the Terminal
  Host
- Carried by: native programs

> Copyright 2021 The simdutf authors
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of
> this software and associated documentation files (the "Software"), to deal in
> the Software without restriction, including without limitation the rights to
> use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of
> the Software, and to permit persons to whom the Software is furnished to do so,
> subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS
> FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR
> COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
> IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
> CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

## Highway

- Copyright notice: Copyright (c) The Highway Project Authors
- Source: <https://github.com/google/highway>
- Pinned revision: `66486a10623fa0d72fe91260f96c892e41aceb06`, as the pinned Ghostty revision
  fetches it
- License: BSD-3-Clause, chosen from Apache-2.0 or BSD-3-Clause
- Use in Orchbee: the portable SIMD library that Ghostty VT compiles into the Terminal Host
- Carried by: native programs

> Copyright (c) The Highway Project Authors. All rights reserved.
>
> Redistribution and use in source and binary forms, with or without modification,
> are permitted provided that the following conditions are met:
>
> 1.  Redistributions of source code must retain the above copyright notice, this
>     list of conditions and the following disclaimer.
>
> 2.  Redistributions in binary form must reproduce the above copyright notice,
>     this list of conditions and the following disclaimer in the documentation
>     and/or other materials provided with the distribution.
>
> 3.  Neither the name of the copyright holder nor the names of its
>     contributors may be used to endorse or promote products derived from
>     this software without specific prior written permission.
>
> THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
> AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
> IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
> DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
> FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
> DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
> SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
> CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
> OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
> OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

## Wuffs

- Copyright notice: Copyright 2023 The Wuffs Authors
- Source: <https://github.com/google/wuffs>
- Pinned revision: `7411f488fe2e2c205c3d3b3d28638b7356522930`, as the pinned Ghostty revision
  fetches it
- License: MIT, chosen from Apache-2.0 or MIT
- Use in Orchbee: the image decoders that Ghostty VT compiles into the Terminal Host
- Carried by: native programs

> Copyright 2023 The Wuffs Authors
>
> Permission is hereby granted, free of charge, to any
> person obtaining a copy of this software and associated
> documentation files (the "Software"), to deal in the
> Software without restriction, including without
> limitation the rights to use, copy, modify, merge,
> publish, distribute, sublicense, and/or sell copies of
> the Software, and to permit persons to whom the Software
> is furnished to do so, subject to the following
> conditions:
>
> The above copyright notice and this permission notice
> shall be included in all copies or substantial portions
> of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
> ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
> TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
> PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
> SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
> CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
> OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
> IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
> DEALINGS IN THE SOFTWARE.

## uucode

- Copyright notices: Copyright (c) 2026 Jacob Sandlund; the UTF-8 decoder: Copyright (c) 2008-2009
  Bjoern Hoehrmann; the Unicode Character Database: Copyright © 1991-2025 Unicode, Inc.
- Source: <https://github.com/jacobsandlund/uucode>, with the decoder from
  <https://bjoern.hoehrmann.de/utf-8/decoder/dfa/> and the data from <https://www.unicode.org/ucd/>
- Pinned revision: `2826a37a4562284fdacd8fa029d49509cc9bffcd`, as the pinned Ghostty revision
  fetches it
- License: MIT; the Unicode Character Database: Unicode License V3
- Use in Orchbee: the Unicode property tables that Ghostty VT builds from the Unicode Character
  Database and compiles into the Terminal Host
- Carried by: native programs

> MIT License
>
> Copyright (c) 2026 Jacob Sandlund
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of
> this software and associated documentation files (the "Software"), to deal in
> the Software without restriction, including without limitation the rights to
> use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies
> of the Software, and to permit persons to whom the Software is furnished to do
> so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

> Copyright (c) 2008-2009 Bjoern Hoehrmann <bjoern@hoehrmann.de>
>
> Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

> UNICODE LICENSE V3
>
> COPYRIGHT AND PERMISSION NOTICE
>
> Copyright © 1991-2025 Unicode, Inc.
>
> NOTICE TO USER: Carefully read the following legal agreement. BY
> DOWNLOADING, INSTALLING, COPYING OR OTHERWISE USING DATA FILES, AND/OR
> SOFTWARE, YOU UNEQUIVOCALLY ACCEPT, AND AGREE TO BE BOUND BY, ALL OF THE
> TERMS AND CONDITIONS OF THIS AGREEMENT. IF YOU DO NOT AGREE, DO NOT
> DOWNLOAD, INSTALL, COPY, DISTRIBUTE OR USE THE DATA FILES OR SOFTWARE.
>
> Permission is hereby granted, free of charge, to any person obtaining a
> copy of data files and any associated documentation (the "Data Files") or
> software and any associated documentation (the "Software") to deal in the
> Data Files or Software without restriction, including without limitation
> the rights to use, copy, modify, merge, publish, distribute, and/or sell
> copies of the Data Files or Software, and to permit persons to whom the
> Data Files or Software are furnished to do so, provided that either (a)
> this copyright and permission notice appear with all copies of the Data
> Files or Software, or (b) this copyright and permission notice appear in
> associated Documentation.
>
> THE DATA FILES AND SOFTWARE ARE PROVIDED "AS IS", WITHOUT WARRANTY OF ANY
> KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
> MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT OF
> THIRD PARTY RIGHTS.
>
> IN NO EVENT SHALL THE COPYRIGHT HOLDER OR HOLDERS INCLUDED IN THIS NOTICE
> BE LIABLE FOR ANY CLAIM, OR ANY SPECIAL INDIRECT OR CONSEQUENTIAL DAMAGES,
> OR ANY DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS,
> WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION,
> ARISING OUT OF OR IN CONNECTION WITH THE USE OR PERFORMANCE OF THE DATA
> FILES OR SOFTWARE.
>
> Except as contained in this notice, the name of a copyright holder shall
> not be used in advertising or otherwise to promote the sale, use or other
> dealings in these Data Files or Software without prior written
> authorization of the copyright holder.

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
  `grok.toml` adds browser-authentication blockers and an uncertain-login guard; `mimo.toml` adapts
  the OpenCode 1.x rules of `opencode.toml` to MiMo Code; region matching uses UTF-8-safe CRLF
  offsets. Rule files that `NOTICE` names as authored by Orchbee are not
  derived from Herdr
- License text and notice: [`LICENSE`](apps/orchbee-server/resources/agent-detection/LICENSE) and
  [`NOTICE`](apps/orchbee-server/resources/agent-detection/NOTICE)
- Carried by: native programs

## ripgrep

- Copyright notice: Copyright (c) 2015 Andrew Gallant
- Source: <https://github.com/BurntSushi/ripgrep>
- Pinned revision: `3a612f88b805e14aef45bfa43e25a54abc6297fc` (ripgrep 15.0.0)
- Package: `@vscode/ripgrep-universal@1.18.0`, whose prebuilt `rg` programs are carried unmodified
- License: MIT, chosen from the Unlicense or MIT
- Use in Orchbee: the search program of the File Bar's process, which development and test builds
  carry beside it
- Included components: PCRE2 10.46, under BSD-3-Clause with the PCRE2 exception; in the Linux
  programs, jemalloc 5.3.0, under BSD-2-Clause; and the Rust crates `aho-corasick`, `anyhow`,
  `bstr`, `cfg-if`, `crossbeam-deque`, `crossbeam-epoch`, `crossbeam-utils`, `encoding_rs`,
  `encoding_rs_io`, `itoa`, `lexopt`, `libc`, `log`, `memchr`, `memmap2`, `pcre2`, `pcre2-sys`,
  `regex-automata`, `regex-syntax`, `ryu`, `same-file`, `serde`, `serde_core`, `serde_json`,
  `termcolor`, `textwrap` and `walkdir`, with `winapi-util`, `windows-link` and `windows-sys` on
  Windows and `tikv-jemallocator` and `tikv-jemalloc-sys` on Linux, each under MIT except `ryu`,
  under BSL-1.0, and the WHATWG data in `encoding_rs`, under BSD-3-Clause
- Carried by: native programs in development and test builds
- License text: [`ripgrep-notices.txt`](apps/orchbee-files/licenses/ripgrep-notices.txt), which
  also travels beside `rg` and holds each component's version, license and full license text

## Microsoft ConPTY

- Copyright notice: Copyright (c) Microsoft Corporation
- Source: <https://github.com/microsoft/terminal>, through the `Microsoft.Windows.Console.ConPTY`
  NuGet package that [`windows-conpty.json`](apps/terminal-host/windows-conpty.json) pins
- License: MIT
- Use in Orchbee: the unmodified `conpty.dll` and `OpenConsole.exe` that the Windows Terminal Host
  starts its terminals through
- Included components: the third-party software that Windows Terminal's `NOTICE.md` lists, which
  [`Microsoft.ConPTY-NOTICE.txt`](apps/terminal-host/licenses/Microsoft.ConPTY-NOTICE.txt)
  reproduces from the pinned release's tag
- Carried by: native programs on Windows
- License text: [`Microsoft.ConPTY-LICENSE.txt`](apps/terminal-host/licenses/Microsoft.ConPTY-LICENSE.txt)
  and [`Microsoft.ConPTY-NOTICE.txt`](apps/terminal-host/licenses/Microsoft.ConPTY-NOTICE.txt)

## VS Code Terminal Colors

- Copyright notice: Copyright (c) 2015 - present Microsoft Corporation
- Source: <https://github.com/microsoft/vscode>
- Pinned revision: `c647c1cfa2c2a4b1bbf12cb4db413fe05a0d9044`
- Original path: `src/vs/workbench/contrib/terminal/common/terminalColorRegistry.ts`
- License: MIT
- Use in Orchbee: the light and dark terminal ANSI 0–15 values, unmodified, of the Orchbee Light and
  Orchbee Dark themes in [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Carried by: native programs, Desktop application

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
- Carried by: native programs, Desktop application

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
- Carried by: native programs, Desktop application

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
- Carried by: native programs, Desktop application

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

## Catppuccin Color Theme

- Copyright notice: Copyright (c) 2021 Catppuccin
- Source: <https://github.com/catppuccin/ghostty>
- Pinned revision: `b0b03ccee7ae8f16b13bd4fdfe267616defdb2b7`
- Original path: `themes/catppuccin-mocha.conf`
- License: MIT
- Use in Orchbee: terminal and interface color values, unmodified, of the Catppuccin Mocha theme in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Carried by: native programs, Desktop application

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

## Flexoki Color Theme

- Copyright notice: Copyright (c) 2023 Steph Ango
- Source: <https://github.com/kepano/flexoki>
- Pinned revision: `8d723bac4a9ac46adfdf99d42155286977aac72a`
- Values from: iTerm2 Color Schemes `ghostty/Flexoki Dark` and `ghostty/Flexoki Light`
- License: MIT
- Use in Orchbee: terminal and interface color values of the Flexoki Dark and Flexoki Light themes
  in [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Modifications: lightness correction to the legibility floor of Flexoki Light ANSI 8; the theme
  file records the original value beside the corrected one
- Carried by: native programs, Desktop application

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

## Nightfox Color Theme

- Copyright notice: Copyright (c) 2021 James Simpson
- Source: <https://github.com/EdenEast/nightfox.nvim>
- Pinned revision: `4dacd3f0185a2227bdf3b6c0975a8f0bf87cac9a`
- Values from: iTerm2 Color Schemes `ghostty/Nightfox` and `ghostty/Dayfox`
- License: MIT
- Use in Orchbee: terminal and interface color values, unmodified, of the Nightfox and Dayfox themes
  in [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Carried by: native programs, Desktop application

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

## Dracula Color Theme

- Copyright notice: Copyright (c) 2013-present Dracula Theme
- Source: <https://github.com/dracula/iterm>
- Pinned revision: `e5749bcd07764ad6e154038892b118ea45f88cc0`
- Values from: iTerm2 Color Schemes `ghostty/Dracula`
- License: MIT
- Use in Orchbee: terminal and interface color values, unmodified, of the Dracula theme in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Carried by: native programs, Desktop application

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

## Tokyo Night Color Theme

- Creator: Folke Lemaitre; the upstream license names no separate copyright holder
- Source: <https://github.com/folke/tokyonight.nvim>
- Pinned revision: `cdc07ac78467a233fd62c493de29a17e0cf2b2b6`
- Original path: `extras/ghostty/tokyonight_night`
- License: Apache License 2.0
- Use in Orchbee: terminal and interface color values of the Tokyo Night theme in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Modifications: ANSI 8 is lightened from `#414868` to `#434b6b` to reach the 2:1 legibility
  floor; the theme file records the original value beside the corrected one
- License text: [`tokyo-night-LICENSE.txt`](crates/color-themes/licenses/tokyo-night-LICENSE.txt)
- Carried by: native programs, Desktop application

> Licensed under the Apache License, Version 2.0 (the "License"); you may not use this file except
> in compliance with the License. You may obtain a copy of the License at
> <http://www.apache.org/licenses/LICENSE-2.0>.
>
> Unless required by applicable law or agreed to in writing, software distributed under the License
> is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express
> or implied. See the License for the specific language governing permissions and limitations under
> the License.

## Tomorrow Color Theme

- Copyright notice: Copyright (C) 2011 Chris Kempson
- Source: <https://github.com/chriskempson/tomorrow-theme>
- Pinned revision: `ccf6666d888198d341b26b3a99d0bc96500ad503`
- Values from: iTerm2 Color Schemes `ghostty/Tomorrow Night`
- License: MIT
- Use in Orchbee: terminal and interface color values of the Tomorrow Night theme in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Modifications: lightness correction to the legibility floor of ANSI 8; the theme file records the
  original value beside the corrected one
- Carried by: native programs, Desktop application

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

## Kanagawa Color Theme

- Copyright notice: Copyright (c) 2021 Tommaso Laurenzi
- Source: <https://github.com/rebelot/kanagawa.nvim>
- Pinned revision: `bb85e4bfc8d89b0e62c8fa53ccdd13d12e2f77b3`
- Original path: `extras/ghostty/kanagawa-wave`
- License: MIT
- Use in Orchbee: terminal and interface color values, unmodified, of the Kanagawa Wave theme in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Carried by: native programs, Desktop application

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

## Tango Color Palette

- Creator: Tango Desktop Project
- Source: <http://tango.freedesktop.org>
- Values from: iTerm2 Color Schemes `ghostty/Builtin Tango Light`
- License: public domain; the project states that the Tango color palette is public domain and asks
  for, without requiring, credit
- Use in Orchbee: terminal and interface color values of the Tango Light theme in
  [`crates/color-themes/themes/`](crates/color-themes/themes/)
- Modifications: lightness correction to the legibility floors of ANSI 3, 10, 11, and 14; the theme
  file records the original value beside the corrected one
- Carried by: native programs, Desktop application
- License text: none; the palette is in the public domain

## JetBrains Mono

- Copyright notice: Copyright 2020 The JetBrains Mono Project Authors
- Source: <https://github.com/JetBrains/JetBrainsMono>
- Version: 2.304
- License: SIL Open Font License 1.1
- Use in Orchbee: unmodified variable terminal text faces provisioned for Browser and Desktop; the
  website serves the unmodified variable face from the `@fontsource-variable/jetbrains-mono` 5.3
  package (Google Fonts build v24) for the text of drawn terminals
- License text: [`JetBrainsMono-OFL.txt`](apps/web/src/workbench-client/projected-terminal-renderer/fonts/JetBrainsMono-OFL.txt)
- Carried by: Desktop application, website

## Geist and Geist Mono

- Copyright notice: Copyright 2024 The Geist Project Authors (https://github.com/vercel/geist-font)
- Source: <https://github.com/vercel/geist-font>, through the `@fontsource-variable/geist` 5.3
  package (Google Fonts build v5) and the `@fontsource-variable/geist-mono` 5.3 package (Google Fonts
  build v6)
- License: SIL Open Font License 1.1
- Use in Orchbee: unmodified variable faces the website serves from its own origin for headings,
  body text, labels and commands
- License text: `LICENSE` in each of the two packages
- Carried by: website

## Nerd Fonts Symbols Only

- Copyright notice: Copyright (c) 2014 Ryan L McIntyre
- Source: <https://github.com/ryanoasis/nerd-fonts>
- Version: 3.4.0
- License: MIT
- Use in Orchbee: unmodified fallback terminal-symbol face provisioned for Browser and Desktop
- License text: [`SymbolsNerdFont-LICENSE.txt`](apps/web/src/workbench-client/projected-terminal-renderer/fonts/SymbolsNerdFont-LICENSE.txt)
- Carried by: Desktop application

## Codicons

- Creator: Microsoft Corporation and contributors
- Source: <https://github.com/microsoft/vscode-codicons>
- License: [Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)
- Use in Orchbee: unmodified glyphs rendered with Orchbee theme colors and sizes, in the product and in the website's drawings of the desktop application; the Desktop application carries them as the `@vscode/codicons` package, whose license its generated notices reproduce
- Carried by: website

Codicons is licensed separately from Orchbee. The upstream copyright and license remain with their respective owners.

## Seti File Icons

- Copyright notices: Copyright (c) 2014 Jesse Weed (Seti UI); Copyright (c) 2015 - present
  Microsoft Corporation (Visual Studio Code)
- Source: <https://github.com/microsoft/vscode>, which builds the icons from Seti UI
  <https://github.com/jesseweed/seti-ui>
- Pinned revisions: VS Code `ef5ca45a6e545e4767e0117905ee9d6120b86e11`; Seti UI
  `2d6c5e68b4ded73c92dac291845ee44e1182d511`
- Original paths: `extensions/theme-seti/icons/seti.woff`,
  `extensions/theme-seti/icons/vs-seti-icon-theme.json`, and the file associations of VS Code's
  built-in languages
- License: MIT
- Use in Orchbee: the unmodified icon font and icon theme of the file icons in
  [`packages/ui/src/file-icon/`](packages/ui/src/file-icon/), drawn in Orchbee theme colors
- License text and notice: [`NOTICE`](packages/ui/src/file-icon/upstream/NOTICE)
- Carried by: Desktop application

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

## Lobe Icons

- Copyright notice: Copyright (c) 2023 LobeHub
- Source: <https://github.com/lobehub/lobe-icons>
- Package and revision: `@lobehub/icons-static-svg@1.94.0`, `fbd2d56e3f734e889f1373e71c8368cc4e60e0d7`
- License: MIT
- Use in Orchbee: adapted SVG path data for the Amp, Antigravity, Claude Code, Cline, CodeBuddy, Codex, Cursor, Devin, Gemini CLI, GitHub Copilot, Grok, Hermes Agent, Kilo, Kimi, Kiro, MastraCode, MiMo Code, MiniMax Code, OpenClaw, OpenCode, Pi, Qoder CLI, and Qwen Code Agent identity icons, plus the Huawei and Meta marks for DevEco Code and Muse Code; the website's drawings of the desktop application use the Claude Code, Codex, and OpenCode icons
- Carried by: Desktop application, website

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

## Agent product artwork

- Use in Orchbee: the identity icons of the Agents below
- Carried by: Desktop application
- License text: none; the assets are their owners' marks, which Orchbee shows only to identify
  their Agents

These assets are distributed to identify the respective Agents in Orchbee. The marks and product
names belong to their owners.

| Agent        | Asset source                                                                                                                                                                  |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| AtomCode     | [AtomCode website logo](https://cdn-news.gitcode.com/news/logo_1794046182088.png)                                                                                             |
| Command Code | [Command Code website favicon](https://commandcode.ai/favicon/2024/favicon-32x32.png)                                                                                         |
| Droid        | [Factory website favicon](https://www.factory.ai/favicon.svg)                                                                                                                 |
| Jcode        | [Jcode website favicon](https://jcode.sh/favicon.svg)                                                                                                                         |
| Letta Code   | [Letta website favicon](https://www.letta.com/assets/images/favicon.jpg)                                                                                                      |
| Maki         | [Maki website favicon](https://maki.sh/favicon-32x32.png)                                                                                                                     |
| MothX        | [MothX documentation logo](https://mothx.net/assets/mothx-small.png)                                                                                                          |
| Prime Agent  | [Prime Intellect website favicon](https://cdn.prod.website-files.com/66239f0441b09824acb92c75/671b6f3bbf6572d40beb8cf2_Positive.jpg)                                          |
| Reasonix     | [Reasonix desktop icon](https://github.com/hardylab/Reasonix/blob/33902e96f3dae3d88dd74a7f12d859a7b6d21119/desktop/build/linux/icons/hicolor/32x32/apps/reasonix-desktop.png) |
| Snow         | [Snow JetBrains plugin icon](https://github.com/MayDay-wpf/snow-cli/tree/6f008a0e55e8819fb40b3f229e67fc746a725944/JetBrains/src/main/resources/icons)                         |

## OMP Favicon

- Copyright notice: Copyright (c) 2025 Mario Zechner; Copyright (c) 2025-2026 Can Bölük; Copyright (c) 2026 Stencil Labs, Inc.
- Source: <https://omp.sh/favicon.svg>
- Source snapshot: fetched 2026-08-29, SHA-256 `9419975a0c24961341221c4cec18703db26a989fa037768f92cda74e3769fe05`
- License: MIT
- Use in Orchbee: adapted SVG path and gradient data for the OMP Agent identity icon
- Carried by: Desktop application

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

## Inno Setup

- Copyright notice: Copyright (C) 1997-2026 Jordan Russell. All rights reserved. Portions Copyright (C) 2000-2026 Martijn Laan. All rights reserved.
- Source: <https://github.com/jrsoftware/issrc>
- Pinned release: Inno Setup 6.7.1 (tag `is-6_7_1`)
- Original path: `Files/Languages/Unofficial/ChineseSimplified.isl`
- License: Inno Setup License
- Use in Orchbee: the Windows installer of the Desktop application is built with Inno Setup and
  carries its setup and uninstall programs; the Simplified Chinese messages of the installer are in
  [`apps/desktop/installer/windows/ChineseSimplified.isl`](apps/desktop/installer/windows/ChineseSimplified.isl)
- Modifications: the message file is a modified copy; its header names the three changed messages
- Carried by: Desktop application on Windows

> This software is provided "as-is," without any express or implied warranty. In no event shall
> the author be held liable for any damages arising from the use of this software.
>
> Permission is granted to anyone to use this software for any purpose, including commercial
> applications, and to alter and redistribute it, provided that the following conditions are met:
>
> 1. All redistributions of source code files must retain all copyright notices that are currently
>    in place, and this list of conditions without modification.
>
> 2. All redistributions in binary form must retain all occurrences of the above copyright notice
>    and web site addresses that are currently in place (for example, in the About boxes).
>
> 3. The origin of this software must not be misrepresented; you must not claim that you wrote
>    the original software. If you use this software to distribute a product, an acknowledgment
>    in the product documentation would be appreciated but is not required.
>
> 4. Modified versions in source or binary form must be plainly marked as such, and must not
>    be misrepresented as being the original software.

## AppImage Runtime

- Copyright notices: Copyright (c) 2004-23 probonopd; libfuse: Copyright (C) 2001-2007 Miklos
  Szeredi; squashfuse: Copyright (c) 2012 Dave Vasilevsky and Phillip Lougher; zstd: Copyright (c)
  Meta Platforms, Inc. and affiliates; zlib: (C) 1995-2026 Jean-loup Gailly and Mark Adler;
  mimalloc: Copyright (c) 2018-2021 Microsoft Corporation, Daan Leijen; musl: Copyright © 2005-2020
  Rich Felker, et al.
- Source: <https://github.com/AppImage/type2-runtime>, at the commit that `TYPE2_RUNTIME_COMMIT` in
  [`release.yml`](.github/workflows/release.yml) pins
- License: MIT; libfuse: LGPL-2.1; squashfuse: BSD-2-Clause; zstd: BSD-3-Clause, chosen from
  BSD-3-Clause or GPL-2.0; zlib: Zlib; mimalloc: MIT; musl: MIT
- Use in Orchbee: the unmodified runtime that starts the Linux AppImage, with the libraries it links
  statically
- Carried by: Desktop application on Linux
- License text: [`appimage-runtime-notices.txt`](apps/desktop/installer/linux/appimage-runtime-notices.txt)

## Tailwind CSS

- Copyright notice: Copyright (c) Tailwind Labs, Inc.
- Source: <https://github.com/tailwindlabs/tailwindcss>, through the `tailwindcss` package
- License: MIT
- Use in Orchbee: the base styles and utility classes that the build of `@orchbee/ui` compiles into
  its stylesheet
- Carried by: Desktop application

> MIT License
>
> Copyright (c) Tailwind Labs, Inc.
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## tw-animate-css

- Copyright notice: Copyright (c) 2025 Wombosvideo
- Source: <https://github.com/Wombosvideo/tw-animate-css>, through the `tw-animate-css` package
- License: MIT
- Use in Orchbee: the animation utilities that the build of `@orchbee/ui` compiles into its
  stylesheet
- Carried by: Desktop application

> MIT License
>
> Copyright (c) 2025 Wombosvideo
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## shadcn/ui

- Copyright notice: Copyright (c) 2023 shadcn
- Source: <https://github.com/shadcn-ui/ui>, through its registry and the `shadcn` package
- License: MIT
- Use in Orchbee: the components in [`packages/ui/src/components/ui/`](packages/ui/src/components/ui/),
  which started from the registry's components, and the `shadcn/tailwind.css` stylesheet that the
  build of `@orchbee/ui` compiles into its own
- Carried by: Desktop application

> MIT License
>
> Copyright (c) 2023 shadcn
>
> Permission is hereby granted, free of charge, to any person obtaining a copy
> of this software and associated documentation files (the "Software"), to deal
> in the Software without restriction, including without limitation the rights
> to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
> copies of the Software, and to permit persons to whom the Software is
> furnished to do so, subject to the following conditions:
>
> The above copyright notice and this permission notice shall be included in all
> copies or substantial portions of the Software.
>
> THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
> IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
> FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
> AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
> LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
> OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
> SOFTWARE.

## Visual Studio Code Behavior

- Copyright notice: Copyright (c) 2015 - present Microsoft Corporation
- Source: <https://github.com/microsoft/vscode>
- License: MIT
- Use in Orchbee: the editor, file tab, list scrolling, Source Control and tree modules of the web
  application reimplement behavior that Visual Studio Code specifies, without copying its source;
  each module's `NOTICE.reference` names what follows Visual Studio Code closely
- Carried by: Desktop application
- License text: [`code-editor`](apps/web/src/code-editor/NOTICE.reference),
  [`file-overlay`](apps/web/src/file-overlay/NOTICE.reference),
  [`list-scrolling`](apps/web/src/list-scrolling/NOTICE.reference),
  [`source-control`](apps/web/src/source-control/NOTICE.reference) and
  [`virtual-tree`](apps/web/src/virtual-tree/NOTICE.reference)

## Chromium Tab Algorithms

- Copyright notice: Copyright 2026 The Chromium Authors
- Source: <https://chromium.googlesource.com/chromium/src>
- License: BSD-3-Clause
- Use in Orchbee: the TypeScript adaptation of Chromium's tab interaction algorithms in
  [`@orchbee/tab-strip`](packages/tab-strip/)
- Carried by: Desktop application
- License text: [`NOTICE.reference`](packages/tab-strip/NOTICE.reference)
