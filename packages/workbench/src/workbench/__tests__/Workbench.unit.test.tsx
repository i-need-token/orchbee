import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';

import type {
	WorkbenchSession,
	WorkbenchSessionService,
	WorkbenchSessionSnapshot,
} from '../../sessions';
import { Workbench } from '../Workbench';

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
		return 'session-created';
	}

	closeSession(): void {}
}

describe('Workbench', () => {
	it('composes a management sidebar and terminal-first workspace', () => {
		const sessionService = new StaticSessionService([
			{ id: 'session-1', title: 'Local shell', status: 'running', terminal },
			{ id: 'session-2', title: 'Agent output', status: 'running', terminal },
		]);

		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(Workbench, { sessions: sessionService }),
			),
		);

		expect(markup).toContain('Local shell');
		expect(markup).toContain('Agent output');
		expect(markup).toContain('Local runtime');
		expect(markup).toContain('2 sessions');
		expect(markup).toContain('Terminal actions');
	});

	it('renders an actionable empty state', () => {
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'en-US' },
				createElement(Workbench, { sessions: new StaticSessionService([]) }),
			),
		);

		expect(markup).toContain('No sessions are open');
		expect(markup).toContain('New terminal');
	});

	it('renders its product messages in Simplified Chinese', () => {
		const markup = renderToStaticMarkup(
			createElement(
				OrchbeeI18nProvider,
				{ locale: 'zh-CN' },
				createElement(Workbench, { sessions: new StaticSessionService([]) }),
			),
		);

		expect(markup).toContain('暂无打开的会话');
		expect(markup).toContain('新建终端');
	});
});
