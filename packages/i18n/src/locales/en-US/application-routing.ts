export const enUSApplicationRoutingMessages = {
	'application.routing.unavailable.description':
		'The Project, Workspace, or Tab in this address no longer exists.',
	'application.routing.unavailable.title': 'This page isn’t available',
} as const;

export type ApplicationRoutingMessageId = keyof typeof enUSApplicationRoutingMessages;
