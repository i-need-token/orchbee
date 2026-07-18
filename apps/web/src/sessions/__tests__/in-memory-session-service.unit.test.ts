import { describe, expect, it } from 'vitest';

import { createInMemorySessionService } from '../in-memory-session-service';

describe('createInMemorySessionService', () => {
	it('publishes deterministic session creation and closure', () => {
		const service = createInMemorySessionService();
		let notificationCount = 0;
		const unsubscribe = service.subscribe(() => {
			notificationCount += 1;
		});

		const sessionId = service.createSession();
		expect(service.getSnapshot().sessions.map((session) => session.id)).toContain(sessionId);

		service.closeSession(sessionId);
		expect(service.getSnapshot().sessions.map((session) => session.id)).not.toContain(sessionId);
		expect(notificationCount).toBe(2);

		unsubscribe();
	});

	it('replays terminal history and handles simulated commands', () => {
		const service = createInMemorySessionService();
		const session = service.getSnapshot().sessions[0];
		if (session === undefined) throw new Error('Expected the initial in-memory session.');
		const output: string[] = [];
		const unsubscribe = session.terminal.subscribeOutput((data) => output.push(data));

		session.terminal.resize({ cols: 120, rows: 40 });
		session.terminal.write('status\r');

		expect(output.join('')).toContain('120 columns × 40 rows');
		expect(output.join('')).toContain('$ ');

		unsubscribe();
	});
});
