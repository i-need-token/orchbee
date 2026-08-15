import { Button, Input, ProductIcon, Toggle } from '@orchbee/ui/components';
import type * as React from 'react';

import { useTerminalFindSurface } from './hooks/useTerminalFindSurface';
import styles from './TerminalFindSurface.module.css';
import type { TerminalFindController } from './model/TerminalFindController';

export interface TerminalFindSurfaceProps {
	readonly controller: TerminalFindController;
	readonly onNext: () => void;
	readonly onPrevious: () => void;
	readonly restoreFocus: () => void;
}

export function TerminalFindSurface(props: TerminalFindSurfaceProps): React.ReactNode {
	const view = useTerminalFindSurface(props);
	if (!view.snapshot.isOpen) return null;

	return (
		<section
			aria-label={view.findLabel}
			className={styles['root']}
			data-search-state={view.snapshot.searchState}
			onKeyDown={view.onSurfaceKeyDown}
			role='search'
		>
			<div className={styles['controls']}>
				<ProductIcon name='search' />
				<Input
					aria-describedby='projected-terminal-find-status'
					aria-invalid={view.snapshot.searchState === 'error'}
					aria-label={view.findLabel}
					className={styles['input']}
					onChange={view.onInputChange}
					onKeyDown={view.onInputKeyDown}
					placeholder={view.placeholder}
					ref={view.inputReference}
					value={view.snapshot.query}
				/>
				<Toggle
					aria-label={view.caseSensitiveLabel}
					aria-pressed={view.snapshot.caseSensitive}
					onPressedChange={view.onCaseSensitiveChange}
					size='sm'
					variant='outline'
				>
					Aa
				</Toggle>
				<Toggle
					aria-label={view.wholeWordLabel}
					aria-pressed={view.snapshot.wholeWord}
					onPressedChange={view.onWholeWordChange}
					size='sm'
					variant='outline'
				>
					W
				</Toggle>
				<Toggle
					aria-label={view.regexLabel}
					aria-pressed={view.snapshot.regex}
					onPressedChange={view.onRegexChange}
					size='sm'
					variant='outline'
				>
					.*
				</Toggle>
				<Button
					aria-label={view.previousLabel}
					disabled={view.snapshot.resultCount === 0}
					onClick={view.onPrevious}
					size='icon-sm'
					variant='ghost'
				>
					<ProductIcon name='arrow-left' />
				</Button>
				<Button
					aria-label={view.nextLabel}
					disabled={view.snapshot.resultCount === 0}
					onClick={view.onNext}
					size='icon-sm'
					variant='ghost'
				>
					<ProductIcon name='chevron-right' />
				</Button>
				<Button aria-label={view.closeLabel} onClick={view.onClose} size='icon-sm' variant='ghost'>
					<ProductIcon name='close' />
				</Button>
			</div>
			<p
				aria-live='polite'
				className={styles['status']}
				id='projected-terminal-find-status'
				role='status'
			>
				{view.statusText}
			</p>
		</section>
	);
}
