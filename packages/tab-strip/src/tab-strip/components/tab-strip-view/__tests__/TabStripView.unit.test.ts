import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const stylesheet = readFileSync(
	resolve(process.cwd(), 'src/tab-strip/components/tab-strip-view/TabStripView.module.css'),
	'utf8',
);

describe('TabStripView visual interaction contract', () => {
	it('does not add a shadow to the vertical drag presentation', () => {
		expect(stylesheet).not.toMatch(/^\.ct-drag-ghost\s*\{[^}]*\b(?:box-shadow|filter):/mu);
	});

	it('shows the vertical tab hover surface without a transition', () => {
		expect(stylesheet).not.toMatch(
			/^\.ct-tab\[data-orientation='vertical'\] \.ct-tab__hover-layer\s*\{[^}]*\b(?:animation|transition):/mu,
		);
	});

	it('disables vertical slot transitions only when the tab count changes', () => {
		expect(stylesheet).toMatch(
			/^\.ct-vertical-scroll__content\[data-tab-count-changed\] \.ct-vertical-unpinned__unit\s*\{[^}]*\btransition:\s*none;/mu,
		);
	});

	it('disables scroll anchoring only during a drag', () => {
		expect(stylesheet).toMatch(
			/^\.ct-tab-strip\[data-dragging\] \.ct-vertical-scroll,[^}]*\boverflow-anchor:\s*none;/mu,
		);
		expect(stylesheet).not.toMatch(
			/^\.ct-vertical-scroll(?:__content)?\s*\{[^}]*\boverflow-anchor:\s*none;/mu,
		);
	});
});
