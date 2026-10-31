# 第三方声明

Orchbee 包含以下第三方软件和视觉资产。收录这些内容并不表示其创作者认可 Orchbee。

## Ghostty VT

- 版权声明：Copyright (c) 2024 Mitchell Hashimoto, Ghostty contributors
- 来源：<https://github.com/ghostty-org/ghostty>
- 固定版本：`9c3ec931d64561a8407dde7ac984ce156ae91539`
- 许可证：MIT
- 在 Orchbee 中的用途：为原生和 WebAssembly 运行时提供终端模拟

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

- 创建者：Herdr contributors；上游许可证未单独写明版权持有人
- 来源：<https://github.com/herdrdev/herdr>
- 固定版本：`e0507237ad8d41f60aece10d00ef29ce2339ca34`（Herdr 0.9.1）
- 原始路径：`src/detect/manifests/*.toml` 和 `src/detect/manifest.rs`
- 许可证：Apache License 2.0
- 在 Orchbee 中的用途：
  [`apps/orchbee-server/resources/agent-detection/`](apps/orchbee-server/resources/agent-detection/)
  下随包分发的 Agent 屏幕检测规则，以及
  [`apps/orchbee-server/src/agent_screen/regions.rs`](apps/orchbee-server/src/agent_screen/regions.rs)
  中改编的提示区域匹配
- 修改内容：本地规则修正、解析器和采样集成由 Orchbee 维护；`grok.toml` 增加了浏览器认证阻塞规则和登录状态不确定时的保护；区域匹配使用 UTF-8 安全的 CRLF 偏移
- 许可证文本与声明：[`LICENSE`](apps/orchbee-server/resources/agent-detection/LICENSE) 和
  [`NOTICE`](apps/orchbee-server/resources/agent-detection/NOTICE)

## JetBrains Mono

- 版权声明：Copyright 2020 The JetBrains Mono Project Authors
- 来源：<https://github.com/JetBrains/JetBrainsMono>
- 版本：2.304
- 许可证：SIL Open Font License 1.1
- 在 Orchbee 中的用途：为 Browser 和 Desktop 提供未经修改的终端可变字体
- 许可证文本：[`JetBrainsMono-OFL.txt`](apps/web/src/workbench-client/projected-terminal-renderer/fonts/JetBrainsMono-OFL.txt)

## Nerd Fonts Symbols Only

- 版权声明：Copyright (c) 2014 Ryan L McIntyre
- 来源：<https://github.com/ryanoasis/nerd-fonts>
- 版本：3.4.0
- 许可证：MIT
- 在 Orchbee 中的用途：为 Browser 和 Desktop 提供未经修改的终端符号后备字体
- 许可证文本：[`SymbolsNerdFont-LICENSE.txt`](apps/web/src/workbench-client/projected-terminal-renderer/fonts/SymbolsNerdFont-LICENSE.txt)

## Codicons

- 创建者：Microsoft Corporation and contributors
- 来源：<https://github.com/microsoft/vscode-codicons>
- 许可证：[Creative Commons Attribution 4.0 International](https://creativecommons.org/licenses/by/4.0/)
- 在 Orchbee 中的用途：以 Orchbee 主题颜色和尺寸呈现未经修改的字形

Codicons 采用独立于 Orchbee 的许可。上游版权和许可仍归其各自所有者所有。

## Lobe Icons

- 版权声明：Copyright (c) 2023 LobeHub
- 来源：<https://github.com/lobehub/lobe-icons>
- 包与固定版本：`@lobehub/icons-static-svg@1.94.0`，`fbd2d56e3f734e889f1373e71c8368cc4e60e0d7`
- 许可证：MIT
- 在 Orchbee 中的用途：经适配的 SVG path data，用于 Claude Code、Cline、Codex、Gemini CLI、GitHub Copilot、Grok、OpenCode、Pi 与 Qwen Code Agent identity icon

这些图标所表示的产品名称与商标仍归各自所有者所有。描述性使用不表示其认可 Orchbee。

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

- 版权声明：Copyright (c) 2025 Mario Zechner；Copyright (c) 2025-2026 Can Bölük；Copyright (c) 2026 Stencil Labs, Inc.
- 来源：<https://omp.sh/favicon.svg>
- 来源快照：于 2026-08-29 获取，SHA-256 `9419975a0c24961341221c4cec18703db26a989fa037768f92cda74e3769fe05`
- 许可证：MIT
- 在 Orchbee 中的用途：经适配的 SVG path 与 gradient data，用于 OMP Agent identity icon

OMP 产品名称与商标仍归其各自所有者所有。描述性使用不表示其认可 Orchbee。

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

<!-- documentation-pairing: english-sha256=a769e469966d1a5b929882c5fd8cda785e59cc1eff52afccf520800a7018fd67 zh-cn-sha256=31d15447d6217063de31f69a7021ee97ee73f25e9acc6809520f55f0adefbde9 -->
