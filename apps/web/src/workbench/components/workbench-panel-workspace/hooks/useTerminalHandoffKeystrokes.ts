import * as React from 'react';

/**
 * A handoff takes about 100 ms. One that outlasts this is dropped whole: the person may have typed
 * it again by now, and a part of a command is a different command.
 */
const TYPE_AHEAD_LIMIT_MILLISECONDS = 1_000;
const COMPOSITION_KEY_CODE = 229;
const TERMINAL_VIEWPORT_SELECTOR = '[data-slot="workbench-terminal-viewport"]';

type KeystrokeInit = KeyboardEventInit & { readonly key: string };

interface HeldKeystroke {
	readonly init: KeystrokeInit;
	readonly type: string;
}

interface KeystrokeHold {
	release(): void;
	stop(): void;
}

/**
 * Carries typing across a handoff. Every layer is inert until the new one is ready, so focus falls to
 * the document and keys would reach nothing, as they would whenever a Tab's layout or selection
 * changes under someone typing, including through another Client.
 */
export function useTerminalHandoffKeystrokes(settled: boolean): void {
	const hold = React.useRef<KeystrokeHold | null>(null);
	React.useLayoutEffect(() => {
		if (!settled) hold.current ??= holdKeystrokes();
	}, [settled]);
	// A parent's Effects run after its children's, so the new layer's terminal has focus by now.
	React.useEffect(() => {
		if (!settled) return;
		hold.current?.release();
		hold.current = null;
	}, [settled]);
	React.useEffect(
		() => () => {
			hold.current?.stop();
			hold.current = null;
		},
		[],
	);
}

function holdKeystrokes(): KeystrokeHold {
	const held: HeldKeystroke[] = [];
	const pressed = new Set<string>();
	let firstHeldAt = 0;
	const keep = (event: KeyboardEvent): void => {
		if (
			!reachesNothing(event.target) ||
			event.defaultPrevented ||
			event.isComposing ||
			event.keyCode === COMPOSITION_KEY_CODE
		) {
			return;
		}
		// The release of a key pressed before this belongs to the terminal that saw the press.
		if (event.type === 'keydown') pressed.add(event.code);
		else if (!pressed.delete(event.code)) return;
		// As the terminal would, or Tab would move the keyboard to the sidebar for good.
		event.preventDefault();
		if (held.length === 0) firstHeldAt = performance.now();
		held.push({ init: keystrokeInit(event), type: event.type });
	};
	window.addEventListener('keydown', keep);
	window.addEventListener('keyup', keep);
	const stop = (): void => {
		window.removeEventListener('keydown', keep);
		window.removeEventListener('keyup', keep);
	};
	return {
		release: () => {
			stop();
			if (performance.now() - firstHeldAt > TYPE_AHEAD_LIMIT_MILLISECONDS) return;
			for (const { init, type } of held) {
				const target = document.activeElement;
				if (target === null || target.closest(TERMINAL_VIEWPORT_SELECTOR) === null) return;
				const proceeded = target.dispatchEvent(new KeyboardEvent(type, init));
				// A dispatched key has no default action. A terminal reads the insertion a real one makes,
				// and mixing keys with and without one would lose those without.
				if (proceeded && type === 'keydown' && insertsText(init)) {
					document.execCommand('insertText', false, init.key);
				}
			}
		},
		stop,
	};
}

/** A layer that just became inert can keep focus until the browser next renders. */
function reachesNothing(target: EventTarget | null): boolean {
	return (
		target === document.body || (target instanceof Element && target.closest('[inert]') !== null)
	);
}

function insertsText({ altKey, ctrlKey, key, metaKey }: KeystrokeInit): boolean {
	return key.length === 1 && altKey !== true && ctrlKey !== true && metaKey !== true;
}

function keystrokeInit(event: KeyboardEvent): KeystrokeInit {
	return {
		altKey: event.altKey,
		bubbles: true,
		cancelable: true,
		code: event.code,
		composed: true,
		ctrlKey: event.ctrlKey,
		key: event.key,
		location: event.location,
		metaKey: event.metaKey,
		modifierAltGraph: event.getModifierState('AltGraph'),
		modifierCapsLock: event.getModifierState('CapsLock'),
		modifierNumLock: event.getModifierState('NumLock'),
		repeat: event.repeat,
		shiftKey: event.shiftKey,
	};
}
