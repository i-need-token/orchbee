// @vitest-environment jsdom

import { renderHook } from '@testing-library/react';
import { createElement, type ReactNode } from 'react';
import { describe, expect, it } from 'vitest';

import { DEFAULT_TAB_STRIP_MESSAGES } from '../../constants/default-tab-strip-messages';
import { TabStripMessagesContext } from '../../contexts/tab-strip-messages-context';
import { useTabStripMessages } from '../useTabStripMessages';

describe('useTabStripMessages', () => {
	it('uses the English reference messages without a host provider', () => {
		const { result } = renderHook(useTabStripMessages);

		expect(result.current).toBe(DEFAULT_TAB_STRIP_MESSAGES);
	});

	it('uses the complete host message contract when provided', () => {
		const messages = { ...DEFAULT_TAB_STRIP_MESSAGES, newTab: '新建标签页' };
		const wrapper = ({ children }: { readonly children: ReactNode }) =>
			createElement(TabStripMessagesContext.Provider, { value: messages }, children);
		const { result } = renderHook(useTabStripMessages, { wrapper });

		expect(result.current.newTab).toBe('新建标签页');
	});
});
