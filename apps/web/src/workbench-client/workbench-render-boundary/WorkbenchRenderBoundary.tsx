import { Component, type ErrorInfo, type ReactNode } from 'react';

import {
	ContainedClientFault,
	type ContainedClientFaultSite,
	reportContainedClientFault,
} from '../contained-client-fault';

interface WorkbenchRenderBoundaryProps {
	readonly children: ReactNode;
	/** What takes the failed subtree's place; `retry` mounts the subtree again. */
	readonly fallback: (retry: () => void) => ReactNode;
	readonly site: ContainedClientFaultSite;
}

interface WorkbenchRenderBoundaryState {
	readonly failed: boolean;
}

/**
 * Keeps a render failure inside the subtree that threw, which React would otherwise answer by
 * unmounting the whole window. Only rendering and lifecycle failures reach it; event handlers and
 * asynchronous work contain their own.
 */
export class WorkbenchRenderBoundary extends Component<
	WorkbenchRenderBoundaryProps,
	WorkbenchRenderBoundaryState
> {
	override state: WorkbenchRenderBoundaryState = { failed: false };

	static getDerivedStateFromError(): WorkbenchRenderBoundaryState {
		return { failed: true };
	}

	override componentDidCatch(error: unknown, _info: ErrorInfo): void {
		reportContainedClientFault(new ContainedClientFault(this.props.site, error));
	}

	readonly #retry = (): void => {
		this.setState({ failed: false });
	};

	override render(): ReactNode {
		return this.state.failed ? this.props.fallback(this.#retry) : this.props.children;
	}
}
