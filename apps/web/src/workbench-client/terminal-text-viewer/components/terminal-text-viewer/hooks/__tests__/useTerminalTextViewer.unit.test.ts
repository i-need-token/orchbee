// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { TerminalTextViewerController } from '../../../../terminal-text-viewer-controller';
import { useTerminalTextViewer } from '../useTerminalTextViewer';

function I18nWrapper({ children }: { readonly children: ReactNode }) {
	return createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
}

afterEach(() => cleanup());

describe('useTerminalTextViewer', () => {
	it('projects localized immutable output, search segments, and close focus restoration', () => {
		const controller = new TerminalTextViewerController();
		const restoreFocus = vi.fn<() => void>();
		controller.open({
			initialTopOffset: null,
			kind: 'text',
			notice: 'truncated-start',
			source: 'command-output',
			text: 'Alpha beta ALPHA',
		});
		const rendered = renderHook(
			() =>
				useTerminalTextViewer({
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

	it('projects localized scrollback labels and its exact initial anchor', () => {
		const controller = new TerminalTextViewerController();
		controller.open({
			initialTopOffset: 6,
			kind: 'text',
			notice: null,
			source: 'scrollback',
			text: 'older\nvisible',
		});
		const rendered = renderHook(
			() =>
				useTerminalTextViewer({
					controller,
					platform: 'apple',
					restoreFocus: () => undefined,
				}),
			{ wrapper: I18nWrapper },
		);

		expect(rendered.result.current).toMatchObject({
			contentLabel: 'Retained terminal scrollback',
			copyLabel: 'Copy scrollback',
			searchLabel: 'Find in scrollback',
			title: 'Scrollback',
		});
		expect(rendered.result.current.segments).toEqual([
			{ kind: 'text', text: 'older\n' },
			{ kind: 'anchor' },
			{ kind: 'text', text: 'visible' },
		]);
		controller.dispose();
	});
});
