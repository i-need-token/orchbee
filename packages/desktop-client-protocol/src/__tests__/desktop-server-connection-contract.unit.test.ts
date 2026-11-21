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

	it('reads a refusal it cannot name as unknown, and a blocking process it cannot read as unnamed', () => {
		// A reason a newer Main names leaves the window its general sentence.
		for (const sessionRefusal of [
			{ reason: 'not-yet-named' },
			{ ioKind: 'not-a-kind', osError: 5, reason: 'server-not-started' },
			'recovery-required',
		]) {
			expect(bootstrapOf({ sessionRefusal })).toEqual({ ...BOOTSTRAP, sessionRefusal: null });
		}
		const blockingProcess = {
			processId: 4242,
			program: 'orchbee-terminal-host.exe',
			role: 'terminal-host',
		};
		const blocked = { blockingProcess, reason: 'recovery-required' };
		expect(bootstrapOf({ sessionRefusal: blocked })).toEqual({
			...BOOTSTRAP,
			sessionRefusal: blocked,
		});
		expect(
			bootstrapOf({
				sessionRefusal: {
					blockingProcess: { ...blockingProcess, processId: 0 },
					reason: 'recovery-required',
				},
			}),
		).toEqual({ ...BOOTSTRAP, sessionRefusal: { reason: 'recovery-required' } });
	});
});
