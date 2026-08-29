// @vitest-environment jsdom

import { cleanup, fireEvent, render } from '@testing-library/react';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { afterEach, describe, expect, it } from 'vitest';

import { ProjectedTerminalImagePreviewCard } from '../ProjectedTerminalImagePreviewCard';

describe('ProjectedTerminalImagePreviewCard', () => {
	afterEach(cleanup);

	it('renders a noninteractive data image below an upper-half path', () => {
		const markup = renderToStaticMarkup(
			createElement(ProjectedTerminalImagePreviewCard, {
				preview: {
					anchor: {
						height: 18,
						left: 40,
						surfaceHeight: 400,
						surfaceWidth: 600,
						top: 60,
						width: 8,
					},
					content: new Uint8Array([0x89, 0x50, 0x4e, 0x47]).buffer,
					kind: 'visible',
					mediaType: 'image/png',
					path: '/tmp/output image.png',
				},
			}),
		);

		expect(markup).toContain('data-placement="below"');
		expect(markup).toContain('data:image/png;base64,iVBORw==');
		expect(markup).toContain('output image.png');
		expect(markup).toContain('aria-hidden="true"');
		expect(markup).toContain('--ob-comp-projected-terminal-image-preview-inline-start:40px');
	});

	it('places the card above a lower-half path and normalizes Windows filenames', () => {
		const markup = renderToStaticMarkup(
			createElement(ProjectedTerminalImagePreviewCard, {
				preview: {
					anchor: {
						height: 18,
						left: 400,
						surfaceHeight: 400,
						surfaceWidth: 600,
						top: 320,
						width: 8,
					},
					content: new Uint8Array([0x42, 0x4d]).buffer,
					kind: 'visible',
					mediaType: 'image/bmp',
					path: 'C:\\images\\result.bmp',
				},
			}),
		);

		expect(markup).toContain('data-placement="above"');
		expect(markup).toContain('result.bmp');
	});

	it('silently removes content the browser cannot decode', () => {
		const rendered = render(
			createElement(ProjectedTerminalImagePreviewCard, {
				preview: {
					anchor: {
						height: 18,
						left: 40,
						surfaceHeight: 400,
						surfaceWidth: 600,
						top: 60,
						width: 8,
					},
					content: new Uint8Array([0x89, 0x50, 0x4e, 0x47]).buffer,
					kind: 'visible',
					mediaType: 'image/png',
					path: '/tmp/broken.png',
				},
			}),
		);
		const image = rendered.container.querySelector('img');
		if (image === null) throw new Error('Expected the image preview to render before decode.');

		fireEvent.error(image);

		expect(
			rendered.container.querySelector('[data-slot="projected-terminal-image-preview-card"]'),
		).toBeNull();
	});
});
