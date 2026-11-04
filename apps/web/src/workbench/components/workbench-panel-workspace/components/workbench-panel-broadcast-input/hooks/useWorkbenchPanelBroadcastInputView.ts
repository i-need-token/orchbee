import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchTerminalBroadcastInput } from '../../../../../types/workbench-terminal-surface';

const MODIFIER_CODES = new Set([
	'AltLeft',
	'AltRight',
	'ControlLeft',
	'ControlRight',
	'MetaLeft',
	'MetaRight',
	'ShiftLeft',
	'ShiftRight',
]);

interface UseWorkbenchPanelBroadcastInputViewOptions {
	readonly end: () => void;
	readonly enqueue: (input: WorkbenchTerminalBroadcastInput) => void;
	readonly toggleVisibility: () => void;
}

interface WorkbenchPanelBroadcastInputView {
	readonly end: () => void;
	readonly inputReference: React.RefObject<HTMLInputElement | null>;
	readonly onBeforeInput: React.FormEventHandler<HTMLInputElement>;
	readonly onBlur: React.FocusEventHandler<HTMLInputElement>;
	readonly onCompositionEnd: React.CompositionEventHandler<HTMLInputElement>;
	readonly onCompositionStart: React.CompositionEventHandler<HTMLInputElement>;
	readonly onDrop: React.DragEventHandler<HTMLInputElement>;
	readonly onInput: React.FormEventHandler<HTMLInputElement>;
	readonly onKeyDown: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onKeyUp: React.KeyboardEventHandler<HTMLInputElement>;
	readonly onPaste: React.ClipboardEventHandler<HTMLInputElement>;
	readonly toggleVisibility: React.MouseEventHandler<HTMLButtonElement>;
}

export function useWorkbenchPanelBroadcastInputView({
	end,
	enqueue,
	toggleVisibility,
}: UseWorkbenchPanelBroadcastInputViewOptions): WorkbenchPanelBroadcastInputView {
	const inputReference = React.useRef<HTMLInputElement>(null);
	const composing = React.useRef(false);
	const pressedModifierCodes = React.useRef(new Set<string>());
	const enqueueKey = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>): void => {
		const modifierCodes = pressedModifierCodes.current;
		enqueue({
			code: event.code,
			key: event.key,
			kind: 'key',
			modifiers: {
				alt: event.altKey,
				altGraph: event.getModifierState('AltGraph'),
				altLeft: modifierCodes.has('AltLeft'),
				altRight: modifierCodes.has('AltRight'),
				capsLock: event.getModifierState('CapsLock'),
				control: event.ctrlKey,
				controlRight: modifierCodes.has('ControlRight'),
				meta: event.metaKey,
				metaRight: modifierCodes.has('MetaRight'),
				numLock: event.getModifierState('NumLock'),
				shift: event.shiftKey,
				shiftRight: modifierCodes.has('ShiftRight'),
			},
			repeat: event.repeat,
		});
	});
	const onBeforeInput = useMemoizedFn((event: React.FormEvent<HTMLInputElement>): void => {
		const inputType = Reflect.get(event.nativeEvent, 'inputType');
		if (inputType === 'insertFromDrop' || inputType === 'insertFromPaste') {
			event.preventDefault();
		}
	});
	const onBlur = useMemoizedFn((): void => pressedModifierCodes.current.clear());
	const onCompositionEnd = useMemoizedFn(
		(event: React.CompositionEvent<HTMLInputElement>): void => {
			composing.current = false;
			const target = event.currentTarget;
			globalThis.queueMicrotask(() => {
				const text = target.value;
				target.value = '';
				if (text !== '') enqueue({ kind: 'text', text });
			});
		},
	);
	const onCompositionStart = useMemoizedFn((): void => {
		composing.current = true;
	});
	const onDrop = useMemoizedFn((event: React.DragEvent<HTMLInputElement>): void => {
		event.preventDefault();
		event.stopPropagation();
	});
	const onInput = useMemoizedFn((event: React.FormEvent<HTMLInputElement>): void => {
		const inputType = Reflect.get(event.nativeEvent, 'inputType');
		if (inputType === 'insertFromDrop' || inputType === 'insertFromPaste') {
			event.currentTarget.value = '';
			return;
		}
		if (composing.current || Reflect.get(event.nativeEvent, 'isComposing') === true) return;
		const text = event.currentTarget.value;
		event.currentTarget.value = '';
		if (text !== '') enqueue({ kind: 'text', text });
	});
	const onKeyDown = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>): void => {
		event.stopPropagation();
		if (MODIFIER_CODES.has(event.code)) {
			pressedModifierCodes.current.add(event.code);
			event.preventDefault();
			return;
		}
		if (
			Reflect.get(event.nativeEvent, 'isComposing') === true ||
			event.key === 'Dead' ||
			event.key === 'Process'
		) {
			return;
		}
		if (event.metaKey) {
			event.preventDefault();
			return;
		}
		if (event.code === 'Escape' && event.ctrlKey && event.altKey) {
			event.preventDefault();
			toggleVisibility();
			return;
		}
		if (event.code === 'Escape' && event.ctrlKey) {
			event.preventDefault();
			end();
			return;
		}
		if (event.code === '' || event.code === 'Unidentified') {
			event.preventDefault();
			return;
		}
		if (event.code === 'KeyV' && event.ctrlKey) {
			event.preventDefault();
			return;
		}
		if (event.code === 'Insert' && event.shiftKey) {
			event.preventDefault();
			return;
		}
		if (!event.ctrlKey && !event.altKey && event.key.length === 1) return;
		event.preventDefault();
		enqueueKey(event);
	});
	const onKeyUp = useMemoizedFn((event: React.KeyboardEvent<HTMLInputElement>): void => {
		event.stopPropagation();
		pressedModifierCodes.current.delete(event.code);
	});
	const onPaste = useMemoizedFn((event: React.ClipboardEvent<HTMLInputElement>): void => {
		event.preventDefault();
		event.stopPropagation();
	});
	const toggleVisibilityAndRestoreFocus = useMemoizedFn(
		(_event: React.MouseEvent<HTMLButtonElement>): void => {
			toggleVisibility();
			globalThis.queueMicrotask(() => inputReference.current?.focus({ preventScroll: true }));
		},
	);

	return {
		end,
		inputReference,
		onBeforeInput,
		onBlur,
		onCompositionEnd,
		onCompositionStart,
		onDrop,
		onInput,
		onKeyDown,
		onKeyUp,
		onPaste,
		toggleVisibility: toggleVisibilityAndRestoreFocus,
	};
}
