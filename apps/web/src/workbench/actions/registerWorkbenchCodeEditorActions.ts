import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionMessageDescriptor,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { codeEditorMessages } from '@orchbee/i18n';

import { CODE_EDITOR_ACTION_IDS } from '../../code-editor';

interface RegisterWorkbenchCodeEditorActionsOptions {
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	/** The page's editor font zoom, in Monaco's steps of a tenth of the font size. */
	readonly zoom: {
		getLevel(): number;
		setLevel(level: number): void;
	};
}

const EDITOR_CATEGORY = {
	id: 'code-editor',
	title: codeEditorMessages.actionCategory,
};

/**
 * VS Code's Increase, Decrease and Reset Editor Font Size. Monaco keeps the size for every editor on
 * the page, so they exist once, in the Workbench's scope, wherever the File Bar is offered, and run
 * whether or not an editor shows. The terminals keep their own size.
 */
export function registerWorkbenchCodeEditorActions({
	runtime,
	scope,
	zoom,
}: RegisterWorkbenchCodeEditorActionsOptions): ActionDisposable {
	const zoomAction = (
		id: string,
		title: ActionMessageDescriptor,
		next: (level: number) => number,
	): ActionDisposable =>
		runtime.register(
			scope,
			defineAction({
				category: EDITOR_CATEGORY,
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'code-editor.font-zoom',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context) => {
						context.markCommitted();
						zoom.setLevel(next(zoom.getLevel()));
						return { kind: 'completed' };
					},
				},
				id,
				input: noActionInput,
				title,
			}),
		);
	const registrations = [
		// VS Code parity: editor.action.fontZoomIn.
		zoomAction(
			CODE_EDITOR_ACTION_IDS.fontZoomIn,
			codeEditorMessages.fontZoomIn,
			(level) => level + 1,
		),
		// VS Code parity: editor.action.fontZoomOut.
		zoomAction(
			CODE_EDITOR_ACTION_IDS.fontZoomOut,
			codeEditorMessages.fontZoomOut,
			(level) => level - 1,
		),
		// VS Code parity: editor.action.fontZoomReset.
		zoomAction(CODE_EDITOR_ACTION_IDS.fontZoomReset, codeEditorMessages.fontZoomReset, () => 0),
	];
	return {
		dispose: () => {
			for (const registration of registrations) registration.dispose();
		},
	};
}
