import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
} from '@orchbee/actions';
import { codeEditorMessages } from '@orchbee/i18n';
import { useEffect, useMemo } from 'react';

import {
	codeEditorCommandActionId,
	type CodeEditorActionEntry,
	type CodeEditorTextHandle,
} from '../../../../../../../../../../../code-editor';
import type { FileOverlayEditorHost } from '../../../../../../../../../file-overlay-contract';

/** Monaco's own font size actions, which the Workbench's page-wide ones stand in for. */
const LEFT_OUT: ReadonlySet<string> = new Set([
	'editor.action.fontZoomIn',
	'editor.action.fontZoomOut',
	'editor.action.fontZoomReset',
]);

const CATEGORY = {
	id: 'code-editor',
	title: codeEditorMessages.actionCategory,
};

/**
 * The actions the editor offers as Actions, in a scope of its own under the Workbench's: its
 * keyboard scope resolves its `Mod+K` keys there, and Action search lists them as VS Code's command
 * palette lists the focused editor's supported actions, by the editor's name for each and its
 * English one.
 *
 * They follow what the editor offers while it has the keys, read again as it takes them and before
 * each key or press reaches the Workbench, so a key runs an action only where the editor offers it
 * then, as VS Code's do. Focus in Action search or a menu keeps them as they read last; focus in
 * another of the Workbench's regions takes them away until the editor has the keys again.
 */
export function useFileTabEditorActions(
	handle: CodeEditorTextHandle | null,
	host: FileOverlayEditorHost,
): ActionScope {
	const { actionRuntime, actionScope, inFocusRegion } = host;
	const scope = useMemo(
		() => actionRuntime.createScope('code-editor', actionScope),
		[actionRuntime, actionScope],
	);
	useEffect(() => {
		if (handle === null) return undefined;
		const hasKeys = (): boolean => {
			const active = document.activeElement;
			return active !== null && handle.contains(active);
		};
		let current = hasKeys();
		const registrations = new Map<string, ActionDisposable>();
		const register = ({ alias, id, label }: CodeEditorActionEntry): ActionDisposable =>
			actionRuntime.register(
				scope,
				defineAction({
					category: CATEGORY,
					execution: {
						concurrency: 'parallel',
						concurrencyKey: () => 'code-editor',
						mode: 'foreground',
					},
					getAvailability: () => (current ? { kind: 'enabled' } : { kind: 'hidden' }),
					handler: {
						execute: (context) => {
							context.markCommitted();
							if (!hasKeys()) handle.focus();
							handle.run(id);
							return { kind: 'completed' };
						},
					},
					id: codeEditorCommandActionId(id),
					input: noActionInput,
					// VS Code lists an editor action by its label, or by its identity without one.
					title: {
						...(alias === '' ? {} : { defaultMessage: alias }),
						id: codeEditorMessages.command.id,
						values: { label: label === '' ? id : label },
					},
				}),
			);
		const read = (): void => {
			if (!hasKeys()) return;
			const offered = new Map(
				handle
					.supportedActions()
					.filter(({ id }) => !LEFT_OUT.has(id))
					.map((entry) => [entry.id, entry]),
			);
			for (const [id, registration] of registrations) {
				if (offered.has(id)) continue;
				registration.dispose();
				registrations.delete(id);
			}
			for (const [id, entry] of offered) {
				if (!registrations.has(id)) registrations.set(id, register(entry));
			}
		};
		const onFocusIn = ({ target }: FocusEvent): void => {
			if (!(target instanceof Node)) return;
			read();
			const next = handle.contains(target) || (current && !inFocusRegion(target));
			if (next === current) return;
			current = next;
			actionRuntime.notifyChanged();
		};
		// The window sees a key or press before the Workbench's resolver, which listens on the document.
		window.addEventListener('keydown', read, true);
		window.addEventListener('pointerdown', read, true);
		document.addEventListener('focusin', onFocusIn, true);
		read();
		return () => {
			window.removeEventListener('keydown', read, true);
			window.removeEventListener('pointerdown', read, true);
			document.removeEventListener('focusin', onFocusIn, true);
			for (const registration of registrations.values()) registration.dispose();
		};
	}, [actionRuntime, handle, inFocusRegion, scope]);
	return scope;
}
