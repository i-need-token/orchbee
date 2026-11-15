export const enUSWebsiteProductMessages = {
	'website.product.terminalInterface': 'The Orchbee terminal interface',
	'website.product.commandLine': 'A terminal running orchbee commands',
	'website.product.summary.attention':
		'{count, plural, one {# needs attention} other {# need attention}}',
	'website.product.summary.working': '{count} working',
	'website.product.summary.done': '{count} done',
	'website.product.summary.ready': '{count} ready',
} as const;

export type WebsiteProductMessageId = keyof typeof enUSWebsiteProductMessages;
