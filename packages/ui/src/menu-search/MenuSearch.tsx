import { useEffect, useRef, type KeyboardEvent, type ReactNode } from 'react';

import styles from './MenuSearch.module.css';

interface MenuSearchProps {
	/** The matching menu rows. */
	readonly children: ReactNode;
	/** What the list says while no row matches. */
	readonly emptyLabel?: string | undefined;
	readonly label: string;
	readonly onQueryChange: (query: string) => void;
	readonly query: string;
}

const ROW = '[role="menuitem"]:not([aria-disabled="true"])';

/**
 * A search field heading a menu that lists a whole catalog, whose rows scroll at the menu list
 * length. The field holds the keyboard: Enter runs the first row and the arrows move into the list.
 * Typing on a row returns to the field, and a touch does not raise the on-screen keyboard.
 */
export function MenuSearch({
	children,
	emptyLabel,
	label,
	onQueryChange,
	query,
}: MenuSearchProps): ReactNode {
	const field = useRef<HTMLInputElement>(null);
	const results = useRef<HTMLDivElement>(null);
	const rows = () => Array.from(results.current?.querySelectorAll<HTMLElement>(ROW) ?? []);
	useEffect(() => {
		if (window.matchMedia?.('(pointer: coarse)').matches) return undefined;
		// After the menu has placed its own focus on opening.
		const frame = requestAnimationFrame(() => field.current?.focus());
		return () => cancelAnimationFrame(frame);
	}, []);
	const onFieldKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
		if (!event.nativeEvent.isComposing) {
			if (event.key === 'Escape' || event.key === 'Tab') return;
			if (event.key === 'ArrowLeft' && query === '') return;
			if (event.key === 'Enter') {
				event.preventDefault();
				rows()[0]?.click();
			}
			if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
				event.preventDefault();
				const listed = rows();
				// The first row already shows as current, so Down moves past it.
				(event.key === 'ArrowDown' ? (listed[1] ?? listed[0]) : listed.at(-1))?.focus();
			}
		}
		// The menu's own type-ahead and arrows must not act on what the field is editing.
		event.stopPropagation();
	};
	const onResultsKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const backToField = event.key === 'ArrowUp' && event.target === rows()[0];
		const typing =
			(event.key === 'Backspace' || (event.key.length === 1 && event.key !== ' ')) &&
			!event.altKey &&
			!event.ctrlKey &&
			!event.metaKey;
		if (!backToField && !typing) return;
		if (backToField) event.preventDefault();
		event.stopPropagation();
		field.current?.focus();
	};
	return (
		<div className={styles['root']}>
			<input
				ref={field}
				aria-label={label}
				autoComplete='off'
				className={styles['field']}
				placeholder={label}
				spellCheck={false}
				value={query}
				onChange={(event) => onQueryChange(event.target.value)}
				onKeyDown={onFieldKeyDown}
			/>
			<div ref={results} className={styles['results']} onKeyDown={onResultsKeyDown}>
				{emptyLabel === undefined ? children : <p className={styles['empty']}>{emptyLabel}</p>}
			</div>
		</div>
	);
}
