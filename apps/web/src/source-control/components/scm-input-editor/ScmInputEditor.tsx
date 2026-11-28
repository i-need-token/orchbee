import type { ActionScope } from '@orchbee/actions';
import { sourceControlMessages, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutChord, KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';
import { createPortal } from 'react-dom';

import { useWorkbenchKeyboardScope } from '../../../workbench-keyboard-scopes';
import type { ScmCommitInput } from '../../model/ScmCommitInput';
import styles from './ScmInputEditor.module.css';
import { useScmInputEditor } from './useScmInputEditor';

type Modifiers = KeyboardShortcutChord['modifiers'];

function chord(code: string, modifiers: Modifiers = []): KeyboardShortcutChord {
	return { code, modifiers };
}

const ARROWS = ['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight'] as const;

/**
 * The keys the input's editor keeps from wider bindings: those it handles itself, Escape, the
 * history arrows and Accept Input, and Monaco's editing chords that a window binding also holds,
 * such as copying a line down or adding a cursor below.
 */
const SHARED_YIELDS = [
	chord('Escape'),
	chord('ArrowUp'),
	chord('ArrowDown'),
	chord('ArrowUp', ['alt']),
	chord('ArrowDown', ['alt']),
	chord('Enter', ['mod']),
	chord('Enter', ['mod', 'shift']),
	chord('ArrowUp', ['alt', 'mod']),
	chord('ArrowDown', ['alt', 'mod']),
	...ARROWS.map((code) => chord(code, ['alt', 'mod', 'shift'])),
];

const INPUT_YIELDS: Record<KeyboardShortcutPlatform, readonly KeyboardShortcutChord[]> = {
	apple: [...SHARED_YIELDS, chord('KeyD', ['mod'])],
	standard: [
		...SHARED_YIELDS,
		...ARROWS.map((code) => chord(code, ['alt', 'shift'])),
		chord('KeyK', ['mod', 'shift']),
	],
};

interface ScmInputEditorProps {
	/** The scope the editor's keyboard bindings resolve their Actions in. */
	readonly actionScope: ActionScope;
	readonly input: ScmCommitInput;
	/** VS Code's Accept Input, `Mod+Enter`: commits with the message. */
	readonly onAccept: () => void;
	/** Gives what focuses the editor once it exists, and null once it is gone. */
	readonly onFocusTarget: (focus: (() => void) | null) => void;
	/** The editor measured its content anew; the row follows its height. */
	readonly onHeight: (height: number) => void;
	readonly placeholder: string;
	readonly platform: KeyboardShortcutPlatform;
	/** Whether the message cannot change, as while it commits. */
	readonly readOnly: boolean;
	/** The message VS Code's input validation gives, shown below the input while it has focus. */
	readonly validation: string | null;
}

/**
 * A repository's commit input, VS Code's SCM input box: the message in a code editor that grows with
 * it, with VS Code's validation message hanging below it.
 */
export function ScmInputEditor({
	actionScope,
	input,
	onAccept,
	onFocusTarget,
	onHeight,
	placeholder,
	platform,
	readOnly,
	validation,
}: ScmInputEditorProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const boxReference = React.useRef<HTMLDivElement>(null);
	const hostReference = React.useRef<HTMLDivElement>(null);
	useWorkbenchKeyboardScope(hostReference, {
		actionScope,
		kind: 'code-editor',
		yields: INPUT_YIELDS[platform],
	});
	const { dismissValidation, height, validationAnchor } = useScmInputEditor({
		box: boxReference,
		host: hostReference,
		input,
		onAccept,
		onFocusTarget,
		onHeight,
		overflowClass: styles['overflow'] ?? '',
		placeholder,
		platform,
		readOnly,
		validation: validation !== null,
	});
	const close = intl.formatMessage(sourceControlMessages.inputValidationClose);
	return (
		<div className={styles['root']} data-slot='scm-input'>
			<div
				className={styles['box']}
				data-validation={validationAnchor === null ? undefined : ''}
				data-warning={validation === null ? undefined : ''}
				ref={boxReference}
			>
				<div
					className={styles['editor']}
					ref={hostReference}
					style={{ '--ob-comp-source-control-input-height': `${height}px` }}
				/>
			</div>
			{validation === null || validationAnchor === null
				? null
				: createPortal(
						<div
							className={styles['validation']}
							style={{
								'--ob-comp-source-control-validation-block-start': `${validationAnchor.blockStart}px`,
								'--ob-comp-source-control-validation-inline-size': `${validationAnchor.inlineSize}px`,
								'--ob-comp-source-control-validation-inline-start': `${validationAnchor.inlineStart}px`,
							}}
						>
							<span className={styles['message']}>{validation}</span>
							<button
								aria-label={close}
								className={styles['close']}
								type='button'
								onClick={dismissValidation}
								onMouseDown={(event) => event.preventDefault()}
							>
								<ProductIcon name='close' size='section-action' />
							</button>
						</div>,
						document.body,
					)}
		</div>
	);
}
