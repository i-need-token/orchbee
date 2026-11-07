// @vitest-environment jsdom

import { act, render } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { TabIcon } from '../TabIcon';
import { TAB_ICON_CRASH_ANIMATION_MS } from '../utils/tab-icon';

const tab = {
	id: 'a',
	title: 'A',
	pinned: false,
	group: null,
} as const;

describe('TabIcon', () => {
	it('renders a host-owned semantic status without mutating native tab state', () => {
		const { container } = render(
			<TabIcon
				tab={{ ...tab, shouldDisplayFavicon: false }}
				statusAdornment={{
					accessibleLabel: 'Agent working',
					icon: <span data-agent-icon='' />,
					kind: 'active',
				}}
			/>,
		);

		expect(container.querySelector('[data-tab-icon]')?.getAttribute('data-status-adornment')).toBe(
			'active',
		);
		expect(container.querySelector('[data-agent-icon]')).toBeTruthy();
	});

	it('draws a host status mark in the status slot, outranking a glyph and drawing no status dot', () => {
		const status = {
			accessibleLabel: 'Agent working',
			icon: <span data-agent-icon='' />,
			kind: 'active' as const,
		};
		const markOnly = render(
			<TabIcon
				tab={{ ...tab, shouldDisplayFavicon: false }}
				statusAdornment={{ ...status, mark: <span data-agent-mark='' /> }}
			/>,
		);
		expect(
			markOnly.container.querySelector('[data-tab-status-mark] [data-agent-mark]'),
		).toBeTruthy();
		expect(markOnly.container.querySelector('[data-tab-icon-status]')).toBeNull();

		const markAndGlyph = render(
			<TabIcon
				tab={{ ...tab, shouldDisplayFavicon: false }}
				statusAdornment={{ ...status, glyph: '*', mark: <span data-agent-mark='' /> }}
			/>,
		);
		expect(markAndGlyph.container.querySelector('[data-tab-status-glyph]')).toBeNull();
		expect(markAndGlyph.container.querySelector('[data-agent-mark]')).toBeTruthy();
	});

	it('renders a stable host identity independently from its semantic status', () => {
		const { container } = render(
			<TabIcon
				tab={{ ...tab, shouldDisplayFavicon: false }}
				identityIcon={<span data-agent-identity='' />}
				statusAdornment={{
					accessibleLabel: 'Agent working',
					icon: <span data-status-fallback-icon='' />,
					kind: 'active',
				}}
			/>,
		);

		expect(container.querySelector('[data-tab-icon]')?.getAttribute('data-identity-icon')).toBe(
			'true',
		);
		expect(container.querySelector('[data-agent-identity]')).toBeTruthy();
		expect(container.querySelector('[data-status-fallback-icon]')).toBeNull();
		expect(container.querySelector('[data-tab-icon-status]')).toBeTruthy();
	});

	it('keeps a host identity visible without a semantic status', () => {
		const { container } = render(
			<TabIcon
				tab={{ ...tab, shouldDisplayFavicon: false }}
				identityIcon={<span data-ended-agent-identity='' />}
			/>,
		);

		expect(container.querySelector('[data-ended-agent-identity]')).toBeTruthy();
		expect(container.querySelector('[data-tab-icon-status]')).toBeNull();
	});

	it('renders the synchronized spinner and suppresses the default favicon while loading', () => {
		const { container } = render(<TabIcon tab={{ ...tab, networkState: 'loading' }} />);
		const icon = container.querySelector('[data-tab-icon]');
		expect(icon?.getAttribute('data-loading')).toBe('true');
		expect(icon?.hasAttribute('data-show-favicon')).toBe(false);
		expect(container.querySelector('[data-tab-strip-icon="spinner"]')).toBeTruthy();
	});

	it('renders attention and discard decorations with a custom favicon', () => {
		const { container } = render(
			<TabIcon
				tab={{ ...tab, faviconUrl: '/favicon.png', needsAttention: true, discarded: true }}
			/>,
		);
		expect(container.querySelector('[data-tab-icon]')?.getAttribute('data-attention')).toBe('true');
		expect(container.querySelector('[data-tab-icon]')?.getAttribute('data-discarded')).toBe('true');
		expect(container.querySelector('img[data-tab-icon-favicon]')).toBeTruthy();
	});

	it('animates a later crash but shows an initially crashed tab directly', () => {
		vi.useFakeTimers();
		const { container, rerender } = render(<TabIcon tab={tab} />);
		rerender(<TabIcon tab={{ ...tab, crashed: true }} />);
		expect(container.querySelector('[data-tab-icon]')?.getAttribute('data-crash-transition')).toBe(
			'true',
		);
		act(() => vi.advanceTimersByTime(TAB_ICON_CRASH_ANIMATION_MS));
		expect(container.querySelector('[data-tab-icon]')?.hasAttribute('data-crash-transition')).toBe(
			false,
		);
		vi.useRealTimers();

		const initial = render(<TabIcon tab={{ ...tab, crashed: true }} />);
		expect(
			initial.container.querySelector('[data-tab-icon]')?.hasAttribute('data-crash-transition'),
		).toBe(false);
	});
});
