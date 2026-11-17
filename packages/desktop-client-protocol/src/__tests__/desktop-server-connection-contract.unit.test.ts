import { describe, expect, it } from 'vitest';

import { DesktopServerConnectionReplySchema } from '../desktop-server-connection-contract';

const BOOTSTRAP = {
	applicationVersion: '1.2.0',
	automaticWorkspaceAllowed: false,
	checkoutName: null,
	defaultDirectory: '/home/someone',
	distribution: 'release',
	loginShellAvailable: true,
	negotiatedCapabilities: ['compatibility-window', 'update-check'],
	quitAllAvailable: true,
	relaunchPlace: null,
	sessionRefusal: null,
	terminalHostName: 'host',
	windowUpdatesOffered: true,
};
const DOCUMENT = '00000000-0000-4000-8000-000000000001';

function bootstrapOf(changes: Readonly<Record<string, unknown>>): unknown {
	const reply = DesktopServerConnectionReplySchema.safeParse({
		bootstrap: { ...BOOTSTRAP, ...changes },
		document: DOCUMENT,
	});
	return reply.success ? reply.data.bootstrap : null;
}

describe('Desktop Server connection contract', () => {
	it('reads a malformed About or capability member as unknown instead of refusing the document', () => {
		for (const negotiatedCapabilities of [
			['Not A Name'],
			'update-check',
			Array.from({ length: 257 }, (_, index) => `capability-${index}`),
			undefined,
		]) {
			expect(bootstrapOf({ negotiatedCapabilities })).toEqual({
				...BOOTSTRAP,
				negotiatedCapabilities: null,
			});
		}
		for (const applicationVersion of ['1.2 .0', '', 7, undefined]) {
			expect(bootstrapOf({ applicationVersion })).toEqual({
				...BOOTSTRAP,
				applicationVersion: null,
			});
		}
		for (const windowUpdatesOffered of ['yes', null, undefined]) {
			expect(bootstrapOf({ windowUpdatesOffered })).toEqual({
				...BOOTSTRAP,
				windowUpdatesOffered: false,
			});
		}
	});
});
