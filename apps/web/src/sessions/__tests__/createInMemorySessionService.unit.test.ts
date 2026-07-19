import { describe, expect, it } from 'vitest';

import { createInMemorySessionService } from '../createInMemorySessionService';

describe('createInMemorySessionService', () => {
	it('publishes deterministic session creation and closure', () => {
		const service = createInMemorySessionService('en-US');
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
		const service = createInMemorySessionService('en-US');
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

	it('formats product-authored session content in the selected locale', () => {
		const service = createInMemorySessionService('zh-CN');
		const session = service.getSnapshot().sessions[0];
		if (session === undefined) throw new Error('Expected the initial in-memory session.');
		const output: string[] = [];
		const unsubscribe = session.terminal.subscribeOutput((data) => output.push(data));

		session.terminal.write('help\r');

		expect(session.title).toBe('本地 Shell');
		expect(output.join('')).toContain('此终端由内存会话适配器提供支持');
		expect(output.join('')).toContain('可用命令');

		unsubscribe();
	});
});
