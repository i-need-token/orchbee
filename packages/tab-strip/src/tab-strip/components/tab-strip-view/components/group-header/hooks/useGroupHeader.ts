import { useRef, type RefObject } from 'react';

interface GroupHeaderState {
	editorOpenedOnPress: RefObject<boolean>;
	headerRef: RefObject<HTMLDivElement | null>;
	lastPointerType: RefObject<string>;
}

export function useGroupHeader(): Readonly<GroupHeaderState> {
	const headerRef = useRef<HTMLDivElement>(null);
	const lastPointerType = useRef<string>('mouse');
	const editorOpenedOnPress = useRef(false);

	return { editorOpenedOnPress, headerRef, lastPointerType } as const;
}
