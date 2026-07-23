import { useState, type Dispatch, type RefObject, type SetStateAction } from 'react';
import { useElementSize } from '../../../../../hooks/useElementSize';

interface TabButtonState {
	focused: boolean;
	hovered: boolean;
	measuredTabSize: Readonly<{ width: number; height: number }>;
	setFocused: Dispatch<SetStateAction<boolean>>;
	setHovered: Dispatch<SetStateAction<boolean>>;
	tabViewRef: RefObject<HTMLDivElement | null>;
}

export function useTabButton(): Readonly<TabButtonState> {
	const [tabViewRef, measuredTabSize] = useElementSize<HTMLDivElement>();
	const [hovered, setHovered] = useState(false);
	const [focused, setFocused] = useState(false);

	return { focused, hovered, measuredTabSize, setFocused, setHovered, tabViewRef } as const;
}
