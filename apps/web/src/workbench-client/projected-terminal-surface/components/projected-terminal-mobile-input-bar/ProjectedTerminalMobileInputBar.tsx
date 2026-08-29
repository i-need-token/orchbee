import { projectedTerminalMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import type { PointerEvent, ReactNode } from 'react';

import type {
	MobileTerminalInputState,
	MobileTerminalKey,
	MobileTerminalModifier,
} from '../../../projected-terminal-renderer';
import { projectedTerminalMobileInputKeys } from './constants/projected-terminal-mobile-input-keys';
import styles from './ProjectedTerminalMobileInputBar.module.css';

export interface ProjectedTerminalMobileInputBarProps {
	readonly state: MobileTerminalInputState;
	onKey(key: MobileTerminalKey): void;
	onKeyboardToggle(): void;
	onModifierToggle(modifier: MobileTerminalModifier): void;
}

export function ProjectedTerminalMobileInputBar({
	state,
	onKey,
	onKeyboardToggle,
	onModifierToggle,
}: ProjectedTerminalMobileInputBarProps): ReactNode {
	const intl = useOrchbeeIntl();
	const available = state.readiness.kind === 'ready';
	const preserveTerminalFocus = (event: PointerEvent<HTMLButtonElement>): void => {
		event.preventDefault();
	};
	return (
		<div
			aria-label={intl.formatMessage(projectedTerminalMessages.mobileControls)}
			className={styles['root']}
			data-slot='projected-terminal-mobile-input-bar'
			role='toolbar'
		>
			<Button
				aria-label={intl.formatMessage(
					state.focused
						? projectedTerminalMessages.mobileHideKeyboard
						: projectedTerminalMessages.mobileShowKeyboard,
				)}
				aria-pressed={state.focused}
				className={styles['key']}
				data-slot='projected-terminal-mobile-keyboard'
				disabled={!state.focused && !available}
				onClick={onKeyboardToggle}
				onPointerDown={preserveTerminalFocus}
				size='icon-lg'
				variant='ghost'
			>
				<span aria-hidden='true' className={styles['keyboardGlyph']}>
					⌨
				</span>
			</Button>
			{projectedTerminalMobileInputKeys.slice(0, 2).map((key) => (
				<Button
					aria-label={intl.formatMessage(key.message)}
					className={styles['key']}
					data-mobile-terminal-key={key.intent}
					disabled={!available}
					key={key.intent}
					onClick={() => onKey(key.intent)}
					onPointerDown={preserveTerminalFocus}
					variant='ghost'
				>
					{key.label}
				</Button>
			))}
			<Button
				aria-label={intl.formatMessage(projectedTerminalMessages.mobileControl)}
				aria-pressed={state.control}
				className={styles['key']}
				data-mobile-terminal-modifier='control'
				disabled={!available}
				onClick={() => onModifierToggle('control')}
				onPointerDown={preserveTerminalFocus}
				variant='ghost'
			>
				Ctrl
			</Button>
			<Button
				aria-label={intl.formatMessage(projectedTerminalMessages.mobileAlt)}
				aria-pressed={state.alt}
				className={styles['key']}
				data-mobile-terminal-modifier='alt'
				disabled={!available}
				onClick={() => onModifierToggle('alt')}
				onPointerDown={preserveTerminalFocus}
				variant='ghost'
			>
				Alt
			</Button>
			{projectedTerminalMobileInputKeys.slice(2).map((key) => (
				<Button
					aria-label={intl.formatMessage(key.message)}
					className={styles['key']}
					data-mobile-terminal-key={key.intent}
					disabled={!available}
					key={key.intent}
					onClick={() => onKey(key.intent)}
					onPointerDown={preserveTerminalFocus}
					size='icon-lg'
					variant='ghost'
				>
					{key.label}
				</Button>
			))}
		</div>
	);
}
