import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { ScmSectionsState } from '../model/scm-view-state';
import {
	readSourceControlSections,
	writeSourceControlSections,
	type SourceControlViewStateStorage,
} from '../model/source-control-view-state-storage';

interface ScmSections {
	/** The Repositories list's height while the person drags the sash; null otherwise. */
	readonly dragging: number | null;
	readonly sections: ScmSectionsState;
	/** Starts, moves and ends a drag of the sash from the height the list showed when it began. */
	readonly drag: {
		readonly end: () => void;
		readonly move: (height: number) => void;
		readonly start: () => void;
	};
	readonly toggleChanges: () => void;
	readonly toggleRepositories: () => void;
}

/**
 * The two sections' collapsed states and the Repositories height, which every Workspace shares and
 * the device remembers once the person changes them; a drag is saved when it ends.
 */
export function useScmSections(storage: SourceControlViewStateStorage | undefined): ScmSections {
	const [sections, setSections] = React.useState(() => readSourceControlSections(storage));
	const [dragging, setDragging] = React.useState<number | null>(null);
	const commit = useMemoizedFn((next: ScmSectionsState) => {
		setSections(next);
		writeSourceControlSections(storage, next);
	});
	const end = useMemoizedFn(() => {
		if (dragging !== null) commit({ ...sections, repositoriesSize: dragging });
		setDragging(null);
	});
	const move = useMemoizedFn((height: number) => setDragging(height));
	const start = useMemoizedFn(() => setDragging(null));
	const toggleChanges = useMemoizedFn(() =>
		commit({ ...sections, changesCollapsed: !sections.changesCollapsed }),
	);
	const toggleRepositories = useMemoizedFn(() =>
		commit({ ...sections, repositoriesCollapsed: !sections.repositoriesCollapsed }),
	);
	const drag = React.useMemo(() => ({ end, move, start }), [end, move, start]);
	return { drag, dragging, sections, toggleChanges, toggleRepositories };
}
