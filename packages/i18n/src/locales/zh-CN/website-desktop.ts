import type { WebsiteDesktopMessageId } from '../en-US';

export const zhCNWebsiteDesktopMessages = {
	'website.desktop.metaTitle': '桌面应用 — Orchbee',
	'website.desktop.metaDescription':
		'在独立窗口中呈现同一个会话：标签栏、侧边栏，以及可以用鼠标排布的面板。终端使用 WebGPU 渲染。',
	'website.desktop.lead':
		'在独立窗口中呈现同一个会话：标签栏、侧边栏，以及可以用鼠标排布的面板。终端使用 WebGPU 渲染。',
} as const satisfies Readonly<Record<WebsiteDesktopMessageId, string>>;
