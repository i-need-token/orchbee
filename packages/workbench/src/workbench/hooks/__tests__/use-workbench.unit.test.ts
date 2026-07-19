import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type {
	WorkbenchSession,
	WorkbenchSessionService,
	WorkbenchSessionSnapshot,
} from '../../../sessions';
import { useWorkbench } from '../use-workbench';

const terminal = {
	subscribeOutput: () => () => undefined,
	write: () => undefined,
	resize: () => undefined,
};

class StaticSessionService implements WorkbenchSessionService {
	private readonly snapshot: WorkbenchSessionSnapshot;

	constructor(sessions: readonly WorkbenchSession[]) {
		this.snapshot = { sessions };
	}

	getSnapshot(): WorkbenchSessionSnapshot {
		return this.snapshot;
	}

	subscribe(): () => void {
		return () => undefined;
	}

	createSession(): string {
		return 'created-session';
	}

	closeSession(): void {}
}

function WorkbenchHookHarness({ service }: { readonly service: WorkbenchSessionService }) {
	const workbench = useWorkbench(service);
	return createElement(
		'output',
		{ 'data-active-session': workbench.activeSessionId },
		workbench.sessions.length,
	);
}

describe('useWorkbench', () => {
	it('prepares the initial session and layout model', () => {
		const service = new StaticSessionService([
			{ id: 'session-1', title: 'Local shell', status: 'running', terminal },
		]);

		const markup = renderToStaticMarkup(createElement(WorkbenchHookHarness, { service }));

		expect(markup).toContain('data-active-session="session-1"');
		expect(markup).toContain('>1</output>');
	});
});
