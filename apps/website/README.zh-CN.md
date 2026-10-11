# Orchbee 官网

## 命令

从仓库根目录运行：

```sh
pnpm website
pnpm website:build
pnpm website:preview
```

开发与静态预览使用 `http://127.0.0.1:3100`。开发时将共享工作区导出解析到源码，编辑翻译与 Token 不需要重新构建库。构建命令会先准备声明的本地化与 Token 依赖，再构建应用。

## 静态交付

只将 `apps/website/dist/` 发布到 HTTPS 静态托管服务。没有请求时渲染、服务端进程、账号、分析服务或上报端点。React 在构建阶段渲染 HTML，构建渲染器会从产物中删除。生产 JavaScript 只增强外观、移动导航展开控件和工作区示意。

英语页面位于 `/`，简体中文页面位于 `/zh-CN/`。两个 URL 都包含完整 HTML、本地化元数据、规范链接及互相对应的语言链接、Open Graph 和 Twitter 元数据，以及 WebSite 结构化数据。构建会为 `https://orchbee.com` 输出 `robots.txt` 与 `sitemap.xml`。托管服务必须提供目录索引，并对未知路径返回真实的 404，不能全部重写到首页。部署与 DNS 是独立操作。

## 内容与视觉归属

`src/website/Website.tsx` 拥有页面叙事，其私有 `Showcase` 拥有已明确标注的工作区示意。真实产品截图获得批准后可替换示意，保留图片之外的有意义文本和可访问说明。示例任务用于演示，不是产品能力证据。发布前必须根据产品现状审查早期开发文案与链接，不得把下载或规划中的跨设备访问宣传为已交付。

项目行动按钮指向 [GitHub 仓库](https://github.com/i-need-token/orchbee)，项目负责人计划将其公开。准备公开访问期间保留该地址，上线前检查可用性。下载与文档地址准备就绪后再接入，并同时更新两种语言。

`@orchbee/i18n` 拥有 `websiteMessages` 描述符和成对语言目录。语言 URL 是显式的语言选择；切换使用普通文档导航，让链接保持可抓取并在禁用 JavaScript 时可用。新增语言需要对应的静态路由、文案目录、元数据、语言导航及站点地图替代链接。

视觉语言结合了 [Linear](https://linear.app/) 与 [Cursor](https://cursor.com/) 克制的排版层级和以产品为中心的区块、[Vercel](https://vercel.com/) 的紧凑导航，以及 [Supabase](https://supabase.com/) 的功能分组。[Granola](https://www.granola.ai/) 和 [Tailscale](https://tailscale.com/) 用于参考浅色主题的清晰表达。共享 Token 拥有两套主题映射、品牌强调色和展示排版，CSS Modules 拥有页面组合。参考网站仅提供灵感，不提供素材。

## 外观与隐私

外观默认跟随系统，并实时响应系统变化，直到访问者显式选择浅色或深色。阻塞式头部脚本在绘制前解析外观。禁用 JavaScript 后，默认深色主题下仍可阅读完整内容，使用原生语言链接、锚点和 FAQ 展开控件。

唯一存储的值是键 `orchbee:website:appearance:v1` 下可丢弃的 JSON 对象 `{ "appearance": "system" | "light" | "dark" }`。组合入口注入存储能力，`src/preferences/site-preferences-storage.ts` 拥有严格验证和安全回退。存储缺失、格式错误、不可用或超过配额均不影响阅读和切换页面。不会写入默认值，只有显式变更才持久化。偏好仅属于当前源，不会跨标签页同步。

生产环境没有 Cookie、第三方字体或资源请求、分析统计、信标或遥测。其内容安全策略禁止连接、表单、插件和外部资源。托管访问日志不属于官网代码，应单独配置托管服务以符合预期的隐私政策。

## 验证

运行[验证规范](../../docs/zh-CN/VERIFICATION.md)中的受影响根门禁；修改静态生成或资源时还需运行 `pnpm website:build`。`pnpm --filter @orchbee/website test` 检查外观边界以及无需服务端的双语内容渲染。

在真实浏览器中检查生成的静态站点：两种语言和主题下的桌面与窄屏手机截图；导航、键盘焦点与 Escape、所有示意切换、FAQ、语言链接、外观持久化及系统实时变化。同时检查 320px 布局、放大文字、减少动态效果、禁用 JavaScript、存储被拒绝、直接加载语言页面、SEO 产物，以及没有第三方或上报请求。这些浏览器检查属于人工证据，不在单元测试覆盖范围内。
