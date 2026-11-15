import type { WebsiteCliMessageId } from '../en-US';

export const zhCNWebsiteCliMessages = {
	'website.cli.metaTitle': '命令行 — Orchbee',
	'website.cli.metaDescription': '人可以执行的每一项操作，Agent 都可以通过 orchbee 命令行完成。',
	'website.cli.lead': '人可以执行的每一项操作，Agent 都可以通过 <code>orchbee</code> 命令行完成。',
} as const satisfies Readonly<Record<WebsiteCliMessageId, string>>;
