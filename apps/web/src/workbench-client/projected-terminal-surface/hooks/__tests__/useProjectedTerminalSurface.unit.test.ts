// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { terminalPresentationId, type WorkbenchTerminalPresentation } from '@orchbee/workbench';
import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type { TerminalProjectionClient } from '../../../terminal-projection';
import { ProjectedTerminalSurfaceCapabilityContext } from '../../contexts/projected-terminal-surface-capability';
import { useProjectedTerminalSurface } from '../useProjectedTerminalSurface';

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

function TestHarness({
	presentation,
}: {
	readonly presentation: WorkbenchTerminalPresentation;
}): ReactNode {
	const view = useProjectedTerminalSurface(presentation);
	return createElement('div', {
		'data-projection-state': view.projectionState,
		ref: view.root,
	});
}

describe('useProjectedTerminalSurface', () => {
	it('acquires projection for a running presentation and releases it on unmount', async () => {
		const release = vi.fn<() => void>();
		const open = vi.fn<TerminalProjectionClient['open']>(() => ({
			release,
			result: new Promise(() => undefined),
		}));
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			presentationId: terminalPresentationId('presentation-1'),
			title: 'Terminal 1',
		};
		const container = document.createElement('div');
		document.body.appendChild(container);
		const root = createRoot(container);

		await act(async () => {
			root.render(
				createElement(
					ProjectedTerminalSurfaceCapabilityContext.Provider,
					{
						value: {
							documentElement: document.documentElement,
							preferredLocales: ['en-US'],
							projectionClient: { open },
							shortcutPlatform: 'apple',
							startStoppedTerminalSession: () => undefined,
						},
					},
					createElement(
						OrchbeeI18nProvider,
						{ locale: 'en-US' },
						createElement(TestHarness, { presentation }),
					),
				),
			);
		});

		expect(open).toHaveBeenCalledWith(presentation.presentationId);
		expect(container.firstElementChild?.getAttribute('data-projection-state')).toBe('requested');
		await act(async () => root.unmount());
		expect(release).toHaveBeenCalledOnce();
		container.remove();
	});
});
