import { ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type { TerminalColorThemePickerView } from '../../../../types/terminal-settings-surface';
import styles from './ColorThemePicker.module.css';

interface ColorThemePickerProps {
	readonly describedBy: string | undefined;
	readonly picker: TerminalColorThemePickerView;
}

/**
 * A listbox of one slot's themes. Moving through it previews the highlighted theme; Enter, Space,
 * or a click commits it; Escape or leaving the list restores the committed theme.
 */
export function ColorThemePicker({ describedBy, picker }: ColorThemePickerProps): React.ReactNode {
	const { label, options, selectedId, settingId, onCommit, onPreview } = picker;
	const [activeId, setActiveId] = React.useState<string | null>(null);
	const highlightedId = activeId ?? selectedId;
	const optionElementId = (themeId: string): string => `${settingId}-option-${themeId}`;

	const restore = (): void => {
		setActiveId(null);
		onPreview(null);
	};
	const commit = (themeId: string): void => {
		setActiveId(null);
		if (themeId === selectedId) onPreview(null);
		else onCommit(themeId);
	};
	const highlight = (index: number): void => {
		const option = options[Math.min(Math.max(index, 0), options.length - 1)];
		if (option === undefined) return;
		setActiveId(option.id);
		onPreview(option.id === selectedId ? null : option.id);
		document.getElementById(optionElementId(option.id))?.scrollIntoView({ block: 'nearest' });
	};
	const onKeyDown = (event: React.KeyboardEvent<HTMLDivElement>): void => {
		const index = options.findIndex((option) => option.id === highlightedId);
		switch (event.key) {
			case 'ArrowDown':
				event.preventDefault();
				highlight(index + 1);
				return;
			case 'ArrowUp':
				event.preventDefault();
				highlight(index - 1);
				return;
			case 'Home':
				event.preventDefault();
				highlight(0);
				return;
			case 'End':
				event.preventDefault();
				highlight(options.length - 1);
				return;
			case 'Enter':
			case ' ':
				event.preventDefault();
				commit(highlightedId);
				return;
			case 'Escape':
				// Without a preview to restore, Escape belongs to Settings.
				if (activeId === null || activeId === selectedId) return;
				event.preventDefault();
				restore();
		}
	};

	return (
		<div
			aria-activedescendant={optionElementId(highlightedId)}
			aria-describedby={describedBy}
			aria-label={label}
			className={styles['root']}
			role='listbox'
			tabIndex={0}
			onBlur={() => {
				if (activeId !== null) restore();
			}}
			onKeyDown={onKeyDown}
		>
			{options.map((option) => (
				<div
					aria-selected={option.id === selectedId}
					className={styles['option']}
					data-highlighted={option.id === highlightedId}
					id={optionElementId(option.id)}
					key={option.id}
					role='option'
					onClick={() => commit(option.id)}
				>
					<svg
						aria-hidden='true'
						className={styles['swatch']}
						preserveAspectRatio='none'
						viewBox={`0 0 ${option.swatch.length} 1`}
					>
						{option.swatch.map((cell, index) => (
							<rect fill={cell.color} height={1} key={cell.role} width={1} x={index} y={0} />
						))}
					</svg>
					<span className={styles['name']}>{option.name}</span>
					<span className={styles['check']}>
						{option.id === selectedId ? <ProductIcon name='check' /> : null}
					</span>
				</div>
			))}
		</div>
	);
}
