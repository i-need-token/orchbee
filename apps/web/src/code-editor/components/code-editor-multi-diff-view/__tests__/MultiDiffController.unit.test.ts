import { describe, expect, it } from 'vitest';

import { CodeEditorTextDocument } from '../../../CodeEditorTextDocument';
import type { editor } from '../../../monaco-entry';
import {
	MULTI_DIFF_BOTTOM_PADDING,
	MULTI_DIFF_HEADER_HEIGHT,
} from '../constants/multi-diff-geometry';
import { MultiDiffController, type MultiDiffEntry } from '../MultiDiffController';

/** Every file's whole height while expanded. */
const FILE_HEIGHT = 500;
const CONTENT_HEIGHT = FILE_HEIGHT - MULTI_DIFF_HEADER_HEIGHT - MULTI_DIFF_BOTTOM_PADDING;
const VIEW_HEIGHT = 300;

/** A side whose read never lands, which a controller without editors never asks for. */
function unreadSide(): CodeEditorTextDocument<editor.ITextModel> {
	return new CodeEditorTextDocument<editor.ITextModel>({
		createModel: () => {
			throw new Error('The controller reads no file.');
		},
		limit: 1,
		read: () => new Promise(() => undefined),
		updateModel: () => undefined,
	});
}

function listed(keys: readonly string[]): MultiDiffEntry[] {
	return keys.map((key) => ({
		description: '',
		key,
		modified: unreadSide(),
		name: key,
		original: unreadSide(),
	}));
}

const KEYS = Array.from({ length: 20 }, (_, index) => `f${index}`);

/** A controller showing the files with their diffs measured, in a view `VIEW_HEIGHT` tall. */
function mounted(keys: readonly string[] = KEYS): MultiDiffController {
	const controller = new MultiDiffController();
	controller.setEntries(listed(keys));
	controller.setViewport(800, VIEW_HEIGHT);
	for (const key of keys) controller.setContentHeight(key, CONTENT_HEIGHT);
	return controller;
}

function slotOf(controller: MultiDiffController, key: string): number {
	return controller.slots.findIndex((slot) => slot?.entry.key === key);
}

describe('MultiDiffController', () => {
	it('keeps the file at the top of the view there as every file collapses and expands', () => {
		const controller = mounted();
		controller.scrollTo(10 * FILE_HEIGHT + 200);

		controller.collapseAll();
		expect(controller.allCollapsed).toBe(true);
		expect(controller.scrollTop).toBe(10 * MULTI_DIFF_HEADER_HEIGHT);

		controller.expandAll();
		expect(controller.allCollapsed).toBe(false);
		expect(controller.scrollTop).toBe(10 * FILE_HEIGHT);
	});

	it('returns to where a view state left it, and to its sideways offset once a file reaches it', () => {
		const left = mounted();
		left.toggle('f15');
		left.scrollTo(10 * FILE_HEIGHT + 200);
		const selections = [
			{
				positionColumn: 4,
				positionLineNumber: 7,
				selectionStartColumn: 1,
				selectionStartLineNumber: 7,
			},
		];
		left.setSlotEditor(slotOf(left, 'f10'), {
			key: 'f10',
			scrollTo: () => undefined,
			selections: () => selections,
		});
		left.setOverflow(slotOf(left, 'f10'), 400);
		left.scrollLeftTo(250);
		const state = left.viewState();

		// A file added above the one at the top, not yet measured, leaves that file at the top.
		const restored = new MultiDiffController();
		restored.setEntries(listed(['added', ...KEYS]));
		restored.restore(state);
		restored.setViewport(800, VIEW_HEIGHT);
		expect(restored.scrollTop).toBe(11 * FILE_HEIGHT + 200);
		expect(restored.viewState().files.get('f15')?.collapsed).toBe(true);
		expect(restored.selectionsOf('f10')).toBe(selections);
		expect(restored.scrollLeft).toBe(0);
		restored.setOverflow(slotOf(restored, 'f10'), 400);
		expect(restored.scrollLeft).toBe(250);

		// Scrolling before such a file shows gives up the sideways offset.
		const scrolled = mounted();
		scrolled.restore(state);
		scrolled.scrollTo(0);
		scrolled.setOverflow(0, 400);
		expect(scrolled.scrollLeft).toBe(0);
	});
});
