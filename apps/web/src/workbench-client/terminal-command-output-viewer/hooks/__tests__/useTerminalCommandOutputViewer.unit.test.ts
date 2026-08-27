// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TerminalCommandOutputViewerController } from '../../model/terminal-command-output-viewer-controller';
import { useTerminalCommandOutputViewer } from '../useTerminalCommandOutputViewer';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useTerminalCommandOutputViewer', () => {
	it('projects localized immutable output, search segments, and close focus restoration', () => {
		const controller = new TerminalCommandOutputViewerController();
		const restoreFocus = vi.fn<() => void>();
		controller.open({
			endRow: 4n,
			generation: 2n,
			kind: 'output',
			revision: 3n,
			startRow: 3n,
			text: 'Alpha beta ALPHA',
			truncatedStart: true,
		});
		const rendered = renderHook(
			() =>
				useTerminalCommandOutputViewer({
					controller,
					platform: 'apple',
					restoreFocus,
				}),
			{ wrapper: I18nWrapper },
		);

		expect(rendered.result.current).toMatchObject({
			copyLabel: 'Copy command output',
			statusText: '',
			title: 'Last command output',
			truncatedText: 'Earlier lines were evicted from retained history.',
		});
		expect(rendered.result.current.statusId).not.toBe(rendered.result.current.titleId);

		act(() => controller.setQuery('alpha'));
		expect(rendered.result.current.statusText).toBe('1 of 2 matches');
		expect(
			rendered.result.current.segments.filter((segment) => segment.kind === 'match'),
		).toHaveLength(2);

		act(() => rendered.result.current.onClose());
		expect(controller.getSnapshot().isOpen).toBe(false);
		expect(restoreFocus).toHaveBeenCalledOnce();
		controller.dispose();
	});
});
