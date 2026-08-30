import type { BrowserAuthenticationMessageId } from '../en-US';

export const zhCNBrowserAuthenticationMessages = {
	'browser.authentication.description': '请输入在 Orchbee 桌面端创建的个人访问令牌。',
	'browser.authentication.documentTitle': '登录 — Orchbee',
	'browser.authentication.error.invalidOrigin':
		'此次登录请求并非来自当前 Orchbee 地址，请刷新页面后重试。',
	'browser.authentication.error.invalidPersonalAccessToken': '该个人访问令牌无效。',
	'browser.authentication.error.invalidRequest': '请输入有效的个人访问令牌。',
	'browser.authentication.error.rateLimited': '失败次数过多，请等待一分钟后重试。',
	'browser.authentication.error.required': '请输入个人访问令牌。',
	'browser.authentication.error.unavailable': 'Orchbee 无法完成登录，请检查连接后重试。',
	'browser.authentication.personalAccessTokenLabel': '个人访问令牌',
	'browser.authentication.personalAccessTokenPlaceholder': '粘贴个人访问令牌',
	'browser.authentication.securityNotice':
		'令牌只会发送到当前 Orchbee 地址用于登录，不会保存在浏览器存储中。局域网浏览器访问使用 HTTP，凭据和终端流量不会加密；请仅在可信网络中使用。',
	'browser.authentication.submit': '登录',
	'browser.authentication.submitting': '正在登录…',
	'browser.authentication.title': '登录 Orchbee',
} as const satisfies Record<BrowserAuthenticationMessageId, string>;
