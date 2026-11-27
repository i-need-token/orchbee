import { describe, expect, it } from 'vitest';

import { CodeEditorMultiDiffLayout } from '../CodeEditorMultiDiffLayout';

describe('CodeEditorMultiDiffLayout', () => {
	it('renders a file taller than the view at its height and scrolls its editor through it', () => {
		const layout = new CodeEditorMultiDiffLayout();
		layout.reset([100, 1000, 100]);
		layout.setViewportHeight(300);

		layout.scrollTo(400);
		expect(layout.visibleRange()).toEqual({ first: 1, last: 1 });
		expect(layout.geometry(1)).toEqual({
			frameHeight: 300,
			height: 1000,
			innerScrollTop: 300,
			top: 100,
		});

		layout.scrollTo(Number.MAX_SAFE_INTEGER);
		expect(layout.scrollTop).toBe(900);
		expect(layout.visibleRange()).toEqual({ first: 1, last: 2 });
		expect(layout.geometry(1).innerScrollTop).toBe(700);
	});

	it('keeps the files in sight in place when a file above them changes height', () => {
		const layout = new CodeEditorMultiDiffLayout();
		layout.reset([500, 500, 500]);
		layout.setViewportHeight(300);
		layout.scrollTo(600);

		layout.setHeight(0, 200);
		expect(layout.geometry(1).top - layout.scrollTop).toBe(-100);

		layout.setHeight(1, 800);
		expect(layout.geometry(1).top - layout.scrollTop).toBe(-100);
	});
});
