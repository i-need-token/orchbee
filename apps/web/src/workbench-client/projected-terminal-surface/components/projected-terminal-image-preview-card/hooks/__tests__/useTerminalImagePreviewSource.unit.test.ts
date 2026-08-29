// @vitest-environment jsdom

import type { TerminalImagePreviewMediaType } from '@orchbee/terminal';
import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { useTerminalImagePreviewSource } from '../useTerminalImagePreviewSource';

describe('useTerminalImagePreviewSource', () => {
	afterEach(cleanup);

	it('memoizes an exact data URL for the current bytes and media type', () => {
		const content = new Uint8Array([0, 1, 2]).buffer;
		const rendered = renderHook(
			({ mediaType }) => useTerminalImagePreviewSource(content, mediaType),
			{ initialProps: { mediaType: 'image/png' as TerminalImagePreviewMediaType } },
		);

		expect(rendered.result.current).toBe('data:image/png;base64,AAEC');
		rendered.rerender({ mediaType: 'image/webp' });
		expect(rendered.result.current).toBe('data:image/webp;base64,AAEC');
	});
});
