import type { ApplicationRoutingMessageId } from '../en-US';

export const zhCNApplicationRoutingMessages = {
	'application.routing.unavailable.description': '这个地址中的项目、工作区或标签页已经不存在。',
	'application.routing.unavailable.title': '此页面不可用',
} as const satisfies Readonly<Record<ApplicationRoutingMessageId, string>>;
