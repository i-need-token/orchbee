import type { ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	KEYBOARD_SHORTCUT_SEQUENCE_LEADER,
	formatKeyboardShortcutChord,
	formatKeyboardShortcutPress,
} from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import {
	resolveKeyboardScopes,
	type KeyboardScopeBinding,
	type KeyboardScopeFrame,
	type KeyboardSequencePending,
	type WorkbenchKeyboardScopeRegistry,
} from '../../workbench-keyboard-scopes';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import {
	isWorkbenchExclusiveFocusTarget,
	isWorkbenchTextEntryWithin,
} from '../utils/workbench-focus-dom';
import {
	resolveWorkbenchShortcut,
	workbenchScopeAdmitsSequences,
	type WorkbenchScopedShortcutBinding,
	type WorkbenchScopedShortcutDefinition,
	type WorkbenchScopedShortcutScope,
	type WorkbenchShortcutBinding,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';

/** How long the status notice says a second step named nothing, as VS Code says it. */
const MISSED_SEQUENCE_NOTICE_MILLISECONDS = 10_000;

/** What the status notice says about a sequence: it waits for its second step, or that step missed. */
export type WorkbenchShortcutSequenceNotice =
	| { readonly kind: 'missed'; readonly leader: string; readonly step: string }
	| { readonly kind: 'waiting'; readonly leader: string };

/** The File Bar family's keys and the regions that own them, where the File Bar is offered. */
export interface WorkbenchShortcutScopes {
	readonly bindings: readonly WorkbenchScopedShortcutBinding[];
	/** Runs a scoped shortcut's Action in `scope`, or the Workbench's, if it is available there. */
	readonly invoke: (
		definition: WorkbenchScopedShortcutDefinition,
		scope: ActionScope | undefined,
	) => boolean;
	readonly registry: WorkbenchKeyboardScopeRegistry;
	readonly reportSequence: (notice: WorkbenchShortcutSequenceNotice | null) => void;
}

type WorkbenchShortcutTarget =
	| { readonly kind: 'configured-command' }
	| { readonly kind: 'workbench' }
	| {
			readonly actionScope: ActionScope | undefined;
			readonly definition: WorkbenchScopedShortcutDefinition;
			readonly kind: 'scoped';
	  };

const CONFIGURED_COMMAND_TARGET: WorkbenchShortcutTarget = { kind: 'configured-command' };
const WORKBENCH_TARGET: WorkbenchShortcutTarget = { kind: 'workbench' };

/**
 * The Workbench's one key listener. It walks the scopes around focus from narrowest to widest: a
 * registered code editor, widget, file overlay and File Bar, then configured commands, then the
 * window's shortcuts; the first available match takes the key, and any other key goes on to the
 * focused element, the terminal's own bindings included. `Mod+K` starts a sequence where a
 * registered scope admits one.
 *
 * It listens in the document's capture phase. A window shortcut keeps its key from the focused
 * element; every other key it takes also skips the document's later listeners, as a configured
 * command's key always has, so the terminal's bindings never see it.
 */
export function useWorkbenchShortcuts(
	active: boolean,
	platform: WorkbenchShortcutPlatform,
	bindings: WorkbenchShortcutBindings,
	invokeShortcut: (binding: WorkbenchShortcutBinding, source: 'keyboard') => boolean,
	invokeConfiguredCommandKey: (event: KeyboardEvent) => boolean,
	scopes?: WorkbenchShortcutScopes,
): React.RefObject<HTMLDivElement | null> {
	const rootReference = React.useRef<HTMLDivElement>(null);
	const pending = React.useRef<KeyboardSequencePending | null>(null);
	const notice = React.useRef<{
		readonly kind: WorkbenchShortcutSequenceNotice['kind'];
		readonly timer: ReturnType<typeof setTimeout>;
	} | null>(null);
	const scopedBindings = React.useMemo(
		() => groupScopedBindings(scopes?.bindings ?? []),
		[scopes?.bindings],
	);

	const clearNotice = useMemoizedFn((): void => {
		if (notice.current === null) return;
		clearTimeout(notice.current.timer);
		notice.current = null;
		scopes?.reportSequence(null);
	});
	const showNotice = useMemoizedFn(
		(next: WorkbenchShortcutSequenceNotice, milliseconds: number): void => {
			if (notice.current !== null) clearTimeout(notice.current.timer);
			scopes?.reportSequence(next);
			const timer = setTimeout(() => {
				notice.current = null;
				if (next.kind === 'waiting') pending.current = null;
				scopes?.reportSequence(null);
			}, milliseconds);
			notice.current = { kind: next.kind, timer };
		},
	);
	/** Ends a waiting sequence without a word, as a blur or a key outside the Workbench's reach does. */
	const forgetSequence = useMemoizedFn((): void => {
		pending.current = null;
		if (notice.current?.kind === 'waiting') clearNotice();
	});

	const handleKeyDown = useMemoizedFn((event: KeyboardEvent): void => {
		// A dialog or a menu keeps every key but those a picker inside it declares.
		const exclusive = isWorkbenchExclusiveFocusTarget(event.target);
		if (pending.current !== null && (!active || exclusive)) forgetSequence();
		if (!active) return;
		const frames = scopeFrames(event.target, scopes?.registry, scopedBindings, exclusive);
		if (exclusive && frames.length === 0) return;
		if (!exclusive) {
			frames.push(
				{ bindings: [], fallback: CONFIGURED_COMMAND_TARGET, sequences: false },
				{
					bindings: scopedBindings.get('shell') ?? [],
					fallback: WORKBENCH_TARGET,
					sequences: false,
				},
			);
		}
		const now = Date.now();
		const resolution = resolveKeyboardScopes(
			{ event, frames, now, pending: pending.current, platform },
			(target) => {
				switch (target.kind) {
					case 'configured-command':
						return invokeConfiguredCommandKey(event);
					case 'scoped':
						return scopes?.invoke(target.definition, target.actionScope) ?? false;
					case 'workbench': {
						const binding = resolveWorkbenchShortcut(event, platform, bindings);
						return binding !== null && invokeShortcut(binding, 'keyboard');
					}
				}
			},
		);
		const leader = (): string =>
			formatKeyboardShortcutChord(KEYBOARD_SHORTCUT_SEQUENCE_LEADER, platform);
		switch (resolution.kind) {
			case 'none':
				if (pending.current !== null && resolution.pending === null) forgetSequence();
				return;
			case 'pending':
				if (pending.current !== resolution.pending) {
					pending.current = resolution.pending;
					showNotice({ kind: 'waiting', leader: leader() }, resolution.pending.deadline - now);
				}
				break;
			case 'missed':
				pending.current = null;
				showNotice(
					{ kind: 'missed', leader: leader(), step: formatKeyboardShortcutPress(event, platform) },
					MISSED_SEQUENCE_NOTICE_MILLISECONDS,
				);
				break;
			case 'match':
				if (pending.current !== null) {
					pending.current = null;
					clearNotice();
				}
				if (resolution.target.kind === 'workbench') {
					event.preventDefault();
					event.stopPropagation();
					return;
				}
				break;
		}
		event.preventDefault();
		event.stopImmediatePropagation();
	});
	React.useEffect(() => {
		const ownerDocument = rootReference.current?.ownerDocument;
		if (ownerDocument === undefined) return;
		const ownerWindow = ownerDocument.defaultView;
		ownerDocument.addEventListener('keydown', handleKeyDown, true);
		ownerWindow?.addEventListener('blur', forgetSequence);
		return () => {
			ownerDocument.removeEventListener('keydown', handleKeyDown, true);
			ownerWindow?.removeEventListener('blur', forgetSequence);
			pending.current = null;
			if (notice.current !== null) clearTimeout(notice.current.timer);
			notice.current = null;
		};
	}, [forgetSequence, handleKeyDown]);
	return rootReference;
}

type ScopedBindingGroups = ReadonlyMap<
	WorkbenchScopedShortcutScope,
	readonly KeyboardScopeBinding<WorkbenchShortcutTarget>[]
>;

/** The offered scoped bindings by scope, in their given order, resolving in the Workbench's scope. */
function groupScopedBindings(
	bindings: readonly WorkbenchScopedShortcutBinding[],
): ScopedBindingGroups {
	const groups = new Map<
		WorkbenchScopedShortcutScope,
		KeyboardScopeBinding<WorkbenchShortcutTarget>[]
	>();
	for (const { definition, sequence } of bindings) {
		const group = groups.get(definition.scope) ?? [];
		group.push({ sequence, target: { actionScope: undefined, definition, kind: 'scoped' } });
		groups.set(definition.scope, group);
	}
	return groups;
}

/**
 * The frames of the registered scopes around `target`, narrowest first. The File Bar's frame also
 * follows the file overlay's, since the overlay counts as inside the File Bar; a widget's frame is
 * left out while focus is in a text field inside it.
 */
function scopeFrames(
	target: EventTarget | null,
	registry: WorkbenchKeyboardScopeRegistry | undefined,
	groups: ScopedBindingGroups,
	pickersOnly: boolean,
): KeyboardScopeFrame<WorkbenchShortcutTarget>[] {
	const frames: KeyboardScopeFrame<WorkbenchShortcutTarget>[] = [];
	if (registry === undefined) return frames;
	const entries = registry
		.scopesAt(target)
		.filter((entry) => !pickersOnly || entry.scope.kind === 'quick-input');
	const bindingsIn = (
		scope: WorkbenchScopedShortcutScope,
		actionScope: ActionScope | undefined,
	): readonly KeyboardScopeBinding<WorkbenchShortcutTarget>[] =>
		(groups.get(scope) ?? []).map((binding) =>
			binding.target.kind === 'scoped' && actionScope !== undefined
				? { ...binding, target: { ...binding.target, actionScope } }
				: binding,
		);
	for (const { element, scope } of entries) {
		switch (scope.kind) {
			case 'code-editor':
				frames.push({
					bindings: bindingsIn('code-editor', scope.actionScope),
					sequences: true,
					yields: scope.yields,
				});
				break;
			case 'widget':
				if (isWorkbenchTextEntryWithin(target, element)) break;
				frames.push({
					bindings: bindingsIn(`widget:${scope.widget}`, scope.actionScope),
					sequences: true,
				});
				break;
			case 'file-overlay':
			case 'file-bar':
			case 'quick-input':
				frames.push({
					bindings: bindingsIn(scope.kind, scope.actionScope),
					sequences: workbenchScopeAdmitsSequences(scope.kind),
				});
				break;
		}
	}
	if (
		entries.some((entry) => entry.scope.kind === 'file-overlay') &&
		!entries.some((entry) => entry.scope.kind === 'file-bar')
	) {
		frames.push({ bindings: bindingsIn('file-bar', undefined), sequences: true });
	}
	return frames;
}
