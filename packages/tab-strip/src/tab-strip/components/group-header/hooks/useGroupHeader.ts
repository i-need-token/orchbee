import { useRef, type RefObject } from 'react';
import { useElementSize } from '../../../hooks/useElementSize';

interface GroupHeaderState {
	editorOpenedOnPress: RefObject<boolean>;
	headerRef: RefObject<HTMLDivElement | null>;
	lastPointerType: RefObject<string>;
	measuredHeaderSize: Readonly<{ width: number; height: number }>;
}

export function useGroupHeader(): Readonly<GroupHeaderState> {
	const [headerRef, measuredHeaderSize] = useElementSize<HTMLDivElement>();
	const lastPointerType = useRef<string>('mouse');
	const editorOpenedOnPress = useRef(false);

	return { editorOpenedOnPress, headerRef, lastPointerType, measuredHeaderSize } as const;
}
