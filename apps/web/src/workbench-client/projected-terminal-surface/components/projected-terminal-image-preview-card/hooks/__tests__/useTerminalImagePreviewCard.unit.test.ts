// @vitest-environment jsdom

import { act, cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { useTerminalImagePreviewCard } from '../useTerminalImagePreviewCard';

describe('useTerminalImagePreviewCard', () => {
	afterEach(cleanup);

	it('hides a failed source and admits replacement content', () => {
		const initialContent = new Uint8Array([0, 1, 2]).buffer;
		const replacementContent = new Uint8Array([3, 4, 5]).buffer;
		const rendered = renderHook(
			({ content }) => useTerminalImagePreviewCard(content, 'image/png'),
			{ initialProps: { content: initialContent } },
		);

		expect(rendered.result.current.failed).toBe(false);
		act(() => rendered.result.current.reportFailure());
		expect(rendered.result.current.failed).toBe(true);

		rendered.rerender({ content: replacementContent });
		expect(rendered.result.current.failed).toBe(false);
		expect(rendered.result.current.source).toBe('data:image/png;base64,AwQF');
	});
});
