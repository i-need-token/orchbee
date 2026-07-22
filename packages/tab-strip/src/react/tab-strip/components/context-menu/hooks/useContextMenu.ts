import {
	useEffect,
	useRef,
	useState,
	type Dispatch,
	type RefObject,
	type SetStateAction,
} from 'react';
import type { MenuState } from '../../../utils/tab-strip-internals';

interface ContextMenuState {
	enterTabGroupSubmenuFromKeyboard: RefObject<boolean>;
	setTabGroupSubmenuOpen: Dispatch<SetStateAction<boolean>>;
	tabGroupSubmenuOpen: boolean;
	tabGroupSubmenuRef: RefObject<HTMLDivElement | null>;
	tabGroupTriggerRef: RefObject<HTMLButtonElement | null>;
	tabMenuRef: RefObject<HTMLDivElement | null>;
}

export function useContextMenu({
	state,
	onClose,
}: {
	state: MenuState;
	onClose: () => void;
}): Readonly<ContextMenuState> {
	const [tabGroupSubmenuOpen, setTabGroupSubmenuOpen] = useState(false);
	const tabMenuRef = useRef<HTMLDivElement>(null);
	const tabGroupTriggerRef = useRef<HTMLButtonElement>(null);
	const tabGroupSubmenuRef = useRef<HTMLDivElement>(null);
	const enterTabGroupSubmenuFromKeyboard = useRef(false);

	useEffect(() => {
		if (state.kind === 'tab') tabMenuRef.current?.focus();
	}, [state]);

	useEffect(() => {
		if (!tabGroupSubmenuOpen || !enterTabGroupSubmenuFromKeyboard.current) return;
		enterTabGroupSubmenuFromKeyboard.current = false;
		tabGroupSubmenuRef.current?.querySelector<HTMLButtonElement>('button:not(:disabled)')?.focus();
	}, [tabGroupSubmenuOpen]);
	useEffect(() => {
		const close = () => onClose();
		const closeForEscape = (event: KeyboardEvent) => {
			if (event.key !== 'Escape') return;
			event.preventDefault();
			close();
		};
		window.addEventListener('pointerdown', close);
		window.addEventListener('blur', close);
		window.addEventListener('keydown', closeForEscape);
		return () => {
			window.removeEventListener('pointerdown', close);
			window.removeEventListener('blur', close);
			window.removeEventListener('keydown', closeForEscape);
		};
	}, [onClose]);

	return {
		enterTabGroupSubmenuFromKeyboard,
		setTabGroupSubmenuOpen,
		tabGroupSubmenuOpen,
		tabGroupSubmenuRef,
		tabGroupTriggerRef,
		tabMenuRef,
	} as const;
}
