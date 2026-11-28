import { useMemoizedFn } from '@orchbee/foundation/react';
import { resolveOrchbeeLocale, useOrchbeeIntl } from '@orchbee/i18n';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import * as React from 'react';

import { loadCodeEditor } from '../../../code-editor';
import type { ScmCommitInput } from '../../model/ScmCommitInput';

type CodeEditorRuntime = Awaited<ReturnType<typeof loadCodeEditor>>;
type InputEditor = ReturnType<CodeEditorRuntime['editor']['create']>;
type InputEditorOptions = NonNullable<Parameters<CodeEditorRuntime['editor']['create']>[1]>;

/**
 * VS Code's SCM input editor, `getSimpleEditorOptions` with the SCM input's own: plain text that
 * wraps and grows, without line numbers, folding, rulers, minimap or scrollbars, whose widgets
 * render above the view. Monaco features the SCM input leaves out stay off: bracket matching,
 * occurrence and Unicode highlights, sticky scroll, and Monaco's context menu, whose command palette
 * would bypass Orchbee's keyboard shortcuts.
 */
const INPUT_EDITOR_OPTIONS = {
	automaticLayout: true,
	contextmenu: false,
	dragAndDrop: true,
	dropIntoEditor: { enabled: true },
	fixedOverflowWidgets: true,
	folding: false,
	glyphMargin: false,
	guides: { indentation: false },
	hideCursorInOverviewRuler: true,
	lineDecorationsWidth: 6,
	lineNumbers: 'off',
	matchBrackets: 'never',
	minimap: { enabled: false },
	occurrencesHighlight: 'off',
	overviewRulerBorder: false,
	overviewRulerLanes: 0,
	padding: { bottom: 2, top: 2 },
	quickSuggestions: false,
	renderLineHighlight: 'none',
	renderWhitespace: 'none',
	revealHorizontalRightPadding: 5,
	scrollBeyondLastLine: false,
	scrollbar: { alwaysConsumeMouseWheel: false, horizontal: 'hidden', vertical: 'hidden' },
	selectOnLineNumbers: false,
	selectionHighlight: false,
	stickyScroll: { enabled: false },
	unicodeHighlight: { ambiguousCharacters: false, invisibleCharacters: false },
	wordWrap: 'on',
	wrappingIndent: 'none',
	wrappingStrategy: 'advanced',
} as const satisfies InputEditorOptions;

/** VS Code's `scm.inputFontSize`, for a host whose computed size cannot be read. */
const DEFAULT_FONT_SIZE = 13;

/** The padding above and below the text, which the content height includes. */
const VERTICAL_PADDING = 4;

/** VS Code's `scm.inputMaxLineCount`: the input grows up to ten lines and scrolls beyond them. */
const MAX_LINES = 10;

/** No modifier is held. */
function unmodified(event: KeyboardEvent): boolean {
	return !event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
}

/** VS Code's Accept Input, `Mod+Enter`, which the input handles itself. */
function accepts(event: KeyboardEvent, platform: KeyboardShortcutPlatform): boolean {
	if (event.code !== 'Enter' || event.altKey || event.shiftKey) return false;
	return platform === 'apple' ? event.metaKey && !event.ctrlKey : event.ctrlKey && !event.metaKey;
}

interface ScmInputEditorOptions {
	/** The bordered box around the editor, which the validation message hangs below. */
	readonly box: React.RefObject<HTMLDivElement | null>;
	/** The element Monaco renders in. */
	readonly host: React.RefObject<HTMLDivElement | null>;
	readonly input: ScmCommitInput;
	/** VS Code's Accept Input: commits with the input's message. */
	readonly onAccept: () => void;
	/** Gives what focuses the editor once it exists, and null once it is gone. */
	readonly onFocusTarget: (focus: (() => void) | null) => void;
	/** The editor measured its content anew: its height, clamped to one to ten lines. */
	readonly onHeight: (height: number) => void;
	/** The class of the layer Monaco's hovers and other overflowing widgets render in. */
	readonly overflowClass: string;
	readonly placeholder: string;
	readonly platform: KeyboardShortcutPlatform;
	/** Whether the message cannot change, as while it commits. */
	readonly readOnly: boolean;
	/** Whether the input has a validation message to show while its text has focus. */
	readonly validation: boolean;
}

/** Where the validation message shows: below the input, as wide as it. */
interface ScmInputValidationAnchor {
	readonly inlineSize: number;
	readonly inlineStart: number;
	readonly blockStart: number;
}

interface ScmInputEditor {
	/** The editor's height, which its host takes at once while its row follows the model. */
	readonly height: number;
	/** Where the validation message shows; null while it does not. */
	readonly validationAnchor: ScmInputValidationAnchor | null;
	/** VS Code's Close on the message, or Escape: hides it until the cursor or text moves. */
	readonly dismissValidation: () => void;
}

/**
 * One repository's commit input in Monaco, as VS Code's `SCMInputWidget`: the text model follows the
 * input's message both ways, and the editor reports its content height so the row grows with it.
 * `Mod+Enter` accepts the message. Escape first hides the validation message, then leaves Monaco to
 * cancel a selection or close its find widget, and otherwise clears the message; it never leaves the
 * input. Up at the start and Down at the end walk the message history, and Alt+Up and Alt+Down
 * walk it from anywhere. The validation message shows while the text has focus, until Escape, its
 * Close button or a scroll of the tree hides it; moving the cursor or editing shows it again.
 */
export function useScmInputEditor({
	box,
	host,
	input,
	onAccept,
	onFocusTarget,
	onHeight,
	overflowClass,
	placeholder,
	platform,
	readOnly,
	validation,
}: ScmInputEditorOptions): ScmInputEditor {
	const locale = resolveOrchbeeLocale([useOrchbeeIntl().locale]);
	const [runtime, setRuntime] = React.useState<CodeEditorRuntime | null>(null);
	const [instance, setInstance] = React.useState<InputEditor | null>(null);
	const [height, setHeight] = React.useState(input.height);
	const [focused, setFocused] = React.useState(false);
	const [dismissed, setDismissed] = React.useState(false);
	const [validationAnchor, setValidationAnchor] = React.useState<ScmInputValidationAnchor | null>(
		null,
	);
	const validationShown = validation && focused && !dismissed;
	const accept = useMemoizedFn(onAccept);
	const giveFocusTarget = useMemoizedFn(onFocusTarget);
	const reportHeight = useMemoizedFn(onHeight);
	const isValidationShown = useMemoizedFn(() => validationShown);
	const initialOptions = useMemoizedFn(() => ({ ariaLabel: placeholder, placeholder, readOnly }));
	const dismissValidation = useMemoizedFn(() => setDismissed(true));

	React.useEffect(() => {
		let current = true;
		// A failed load leaves the empty box; a commit then asks for its message instead.
		loadCodeEditor(locale).then(
			(loaded) => {
				if (current) setRuntime(loaded);
			},
			() => undefined,
		);
		return () => {
			current = false;
		};
	}, [locale]);

	React.useLayoutEffect(() => {
		const element = host.current;
		if (runtime === null || element === null) return undefined;
		const { ownerDocument } = element;
		const style = ownerDocument.defaultView?.getComputedStyle(element);
		const fontSize = Number.parseFloat(style?.fontSize ?? '') || DEFAULT_FONT_SIZE;
		// VS Code's SCM input sets its line height to one and a half times its font size.
		const lineHeight = Math.round(fontSize * 1.5);
		const minHeight = lineHeight + VERTICAL_PADDING;
		const maxHeight = MAX_LINES * lineHeight + VERTICAL_PADDING;
		const overflow = ownerDocument.createElement('div');
		// Monaco's theme variables apply under `.monaco-editor`, which its widgets need wherever they render.
		overflow.classList.add('monaco-editor', overflowClass);
		ownerDocument.body.append(overflow);
		const model = runtime.editor.createModel(input.value, 'plaintext');
		const created = runtime.editor.create(element, {
			...INPUT_EDITOR_OPTIONS,
			...initialOptions(),
			...(style === undefined ? {} : { fontFamily: style.fontFamily }),
			fontSize,
			lineHeight,
			model,
			overflowWidgetsDomNode: overflow,
		});
		if (input.selections !== null) created.setSelections(input.selections);

		const measure = (): void => {
			const next = Math.min(Math.max(created.getContentHeight(), minHeight), maxHeight);
			setHeight(next);
			reportHeight(next);
		};
		const subscriptions = [
			created.onDidContentSizeChange((event) => {
				if (event.contentHeightChanged) measure();
			}),
			model.onDidChangeContent(() => {
				input.type(model.getValue());
				setDismissed(false);
			}),
			created.onDidChangeCursorPosition(() => setDismissed(false)),
			created.onDidFocusEditorText(() => setFocused(true)),
			created.onDidBlurEditorText(() => setFocused(false)),
		];
		const unsubscribe = input.subscribe((change) => {
			const { value } = input;
			if (model.getValue() === value) return;
			model.pushStackElement();
			model.pushEditOperations(
				null,
				[{ range: model.getFullModelRange(), text: value }],
				() => null,
			);
			const range = model.getFullModelRange();
			const position =
				change === 'history-previous' ? range.getStartPosition() : range.getEndPosition();
			created.setPosition(position);
			created.revealPositionInCenterIfOutsideViewport(position);
		});

		// Runs before Monaco, whose own key handling listens on the same element as the key bubbles.
		const onKeyDownCapture = (event: KeyboardEvent): void => {
			if (event.isComposing || event.keyCode === 229) return;
			if (accepts(event, platform)) {
				event.preventDefault();
				event.stopPropagation();
				if (!event.repeat) accept();
				return;
			}
			const alone = unmodified(event);
			const altOnly = event.altKey && !event.ctrlKey && !event.metaKey && !event.shiftKey;
			if (event.code === 'Escape' && alone && isValidationShown()) {
				event.preventDefault();
				event.stopPropagation();
				setDismissed(true);
				return;
			}
			if (event.code !== 'ArrowUp' && event.code !== 'ArrowDown') return;
			const position = created.getPosition();
			const range = model.getFullModelRange();
			const edge =
				position !== null &&
				(event.code === 'ArrowUp'
					? position.equals(range.getStartPosition())
					: position.equals(range.getEndPosition()));
			if (!altOnly && !(alone && edge)) return;
			event.preventDefault();
			event.stopPropagation();
			if (event.code === 'ArrowUp') input.showPrevious();
			else input.showNext();
		};
		// Added after Monaco's listener, so it hears whether Monaco took the key.
		const onKeyDown = (event: KeyboardEvent): void => {
			if (event.code !== 'Escape' || event.isComposing || !unmodified(event)) return;
			const selection = created.getSelection();
			if (!event.defaultPrevented && (selection === null || selection.isEmpty())) input.clear();
			event.preventDefault();
			event.stopPropagation();
		};
		element.addEventListener('keydown', onKeyDownCapture, true);
		element.addEventListener('keydown', onKeyDown);
		measure();
		setInstance(created);
		giveFocusTarget(() => created.focus());
		return () => {
			giveFocusTarget(null);
			setInstance(null);
			setFocused(false);
			element.removeEventListener('keydown', onKeyDownCapture, true);
			element.removeEventListener('keydown', onKeyDown);
			unsubscribe();
			for (const subscription of subscriptions) subscription.dispose();
			input.selections = created.getSelections();
			created.dispose();
			model.dispose();
			overflow.remove();
		};
	}, [
		accept,
		giveFocusTarget,
		host,
		initialOptions,
		input,
		isValidationShown,
		overflowClass,
		platform,
		reportHeight,
		runtime,
	]);

	React.useLayoutEffect(() => {
		instance?.updateOptions({ ariaLabel: placeholder, placeholder });
	}, [instance, placeholder]);

	React.useLayoutEffect(() => {
		instance?.updateOptions({ readOnly });
	}, [instance, readOnly]);

	// The message stays where it showed; VS Code hides it when the tree scrolls, as it does here.
	React.useLayoutEffect(() => {
		const element = box.current;
		if (!validationShown || element === null) {
			setValidationAnchor(null);
			return undefined;
		}
		const bounds = element.getBoundingClientRect();
		setValidationAnchor({
			blockStart: bounds.bottom,
			inlineSize: bounds.width,
			inlineStart: bounds.left,
		});
		const view = element.ownerDocument.defaultView;
		const tree = element.closest('[role="tree"]');
		tree?.addEventListener('wheel', dismissValidation, { passive: true });
		view?.addEventListener('resize', dismissValidation);
		return () => {
			tree?.removeEventListener('wheel', dismissValidation);
			view?.removeEventListener('resize', dismissValidation);
		};
	}, [box, dismissValidation, height, validationShown]);

	return { dismissValidation, height, validationAnchor };
}
