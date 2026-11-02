import { useMemoizedFn } from '@orchbee/foundation/react';
import { useEffect, useRef, useState, type ReactNode } from 'react';

import styles from './GroupTitleInput.module.css';

interface GroupTitleInputProps {
	readonly label: string;
	readonly onClose: () => void;
	readonly onCommit: (title: string) => void;
	readonly placeholder: string;
	readonly title: string;
}

/**
 * A name is one change once the person has finished typing it, so an authority that answers later
 * neither hears every key nor overwrites the keys still arriving. Escape keeps what the group had.
 */
export function GroupTitleInput({
	label,
	onClose,
	onCommit,
	placeholder,
	title,
}: GroupTitleInputProps): ReactNode {
	const [draft, setDraft] = useState(title);
	const unsaid = useRef<string | null>(null);
	const commit = useMemoizedFn(() => {
		const name = unsaid.current;
		unsaid.current = null;
		if (name !== null && name !== title) onCommit(name);
	});
	// A menu can close around the input without moving focus out of it first.
	useEffect(() => commit, [commit]);
	return (
		<input
			className={styles['ct-group-editor__title']}
			aria-label={label}
			autoFocus
			value={draft}
			placeholder={placeholder}
			onBlur={commit}
			onChange={(event) => {
				unsaid.current = event.target.value;
				setDraft(event.target.value);
			}}
			onKeyDown={(event) => {
				event.stopPropagation();
				if (event.nativeEvent.isComposing) return;
				if (event.key !== 'Enter' && event.key !== 'Escape') return;
				event.preventDefault();
				if (event.key === 'Enter') commit();
				else unsaid.current = null;
				onClose();
			}}
			onKeyUp={(event) => event.stopPropagation()}
			onPointerDown={(event) => event.stopPropagation()}
			onClick={(event) => event.stopPropagation()}
		/>
	);
}
