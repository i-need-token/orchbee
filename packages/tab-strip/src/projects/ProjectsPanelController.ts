import { requireDefined } from '../common';
import type { TabGroupColor } from '../model';
export interface SavedProjectTabGroup {
	id: string;
	title: string;
	color: TabGroupColor;
	tabCount?: number;
	pinned?: boolean;
	position?: number;
	localGroupId?: string | null;
}
export type ProjectThreadType = 'ai-mode' | 'gemini';
export interface ProjectThread {
	taskId: string;
	serverId: string;
	title: string;
	lastTurnTime: number;
	type: ProjectThreadType;
}
export interface ProjectsPanelSnapshot {
	revision: number;
	tabGroups: readonly Readonly<SavedProjectTabGroup>[];
	threads: readonly Readonly<ProjectThread>[];
}
export interface ProjectsPanelControllerOptions {
	canShowAiModeThreads?: () => boolean;
	canShowGeminiThreads?: () => boolean;
	onOpenTabGroup?: (groupId: string) => void;
	onReorderGroupBefore?: (groupId: string, beforeGroupId: string) => void;
	onReorderGroupAfter?: (groupId: string, afterGroupId: string) => void;
	onOpenThread?: (thread: Readonly<ProjectThread>) => void;
}
export interface ProjectsPanelObserver {
	onTabGroupsInitialized?: (groups: readonly Readonly<SavedProjectTabGroup>[]) => void;
	onTabGroupAdded?: (group: Readonly<SavedProjectTabGroup>, index: number) => void;
	onTabGroupUpdated?: (group: Readonly<SavedProjectTabGroup>) => void;
	onTabGroupRemoved?: (groupId: string, oldIndex: number) => void;
	onTabGroupsReordered?: (groups: readonly Readonly<SavedProjectTabGroup>[]) => void;
	onThreadsInitialized?: (threads: readonly Readonly<ProjectThread>[]) => void;
}
type Listener = () => void;

/** Browser-independent port of ProjectsPanelController and its observers. */
export class ProjectsPanelController {
	private readonly options: ProjectsPanelControllerOptions;
	private groups: SavedProjectTabGroup[] = [];
	private threads: ProjectThread[] = [];
	private revision = 0;
	private snapshot: ProjectsPanelSnapshot = Object.freeze({
		revision: 0,
		tabGroups: [],
		threads: [],
	});
	private readonly listeners = new Set<Listener>();
	private readonly observers = new Set<ProjectsPanelObserver>();
	public constructor(options: ProjectsPanelControllerOptions = {}) {
		this.options = options;
	}
	public getSnapshot = (): ProjectsPanelSnapshot => this.snapshot;
	public subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};
	public addObserver(observer: ProjectsPanelObserver): () => void {
		this.observers.add(observer);
		return () => this.observers.delete(observer);
	}
	public initializeTabGroups(groups: readonly SavedProjectTabGroup[]): void {
		this.groups = groups.map((group) => ({
			...group,
		}));
		this.commit();
		this.observers.forEach((observer) =>
			observer.onTabGroupsInitialized?.(this.snapshot.tabGroups),
		);
	}
	public addTabGroup(group: SavedProjectTabGroup): void {
		const index = Math.min(this.groups.length, Math.max(0, group.position ?? 0));
		this.groups.splice(index, 0, {
			...group,
		});
		this.commit();
		this.observers.forEach((observer) => observer.onTabGroupAdded?.(group, index));
	}
	public updateTabGroup(group: SavedProjectTabGroup): void {
		const index = this.groups.findIndex((candidate) => candidate.id === group.id);
		if (index === -1) return;
		this.groups[index] = {
			...group,
		};
		this.commit();
		this.observers.forEach((observer) => observer.onTabGroupUpdated?.(group));
	}
	public updateTabGroupLocalId(groupId: string, localGroupId: string | null): void {
		const group = this.groups.find((candidate) => candidate.id === groupId);
		if (!group) return;
		group.localGroupId = localGroupId;
		this.commit();
		this.observers.forEach((observer) =>
			observer.onTabGroupUpdated?.(
				Object.freeze({
					...group,
				}),
			),
		);
	}
	public removeTabGroup(groupId: string): void {
		const index = this.groups.findIndex((group) => group.id === groupId);
		if (index === -1) return;
		this.groups.splice(index, 1);
		this.commit();
		this.observers.forEach((observer) => observer.onTabGroupRemoved?.(groupId, index));
	}
	public reorderTabGroups(groups: readonly SavedProjectTabGroup[]): void {
		this.groups = groups.map((group) => ({
			...group,
		}));
		this.commit();
		this.observers.forEach((observer) => observer.onTabGroupsReordered?.(this.snapshot.tabGroups));
	}
	public openTabGroup(groupId: string): void {
		if (this.groups.some((group) => group.id === groupId)) this.options.onOpenTabGroup?.(groupId);
	}

	/** Mirrors ReorderGroupBefore/After without mutating before the service event. */
	public moveTabGroup(groupId: string, newIndex: number): void {
		if (newIndex < 0 || newIndex >= this.groups.length) return;
		const oldIndex = this.groups.findIndex((group) => group.id === groupId);
		if (oldIndex === -1 || oldIndex === newIndex) return;
		const targetId = requireDefined(this.groups[newIndex]).id;
		if (newIndex < oldIndex) this.options.onReorderGroupBefore?.(groupId, targetId);
		else this.options.onReorderGroupAfter?.(groupId, targetId);
	}
	public initializeThreads(threads: readonly ProjectThread[]): void {
		this.threads = threads.map((thread) => ({
			...thread,
		}));
		this.sortThreads();
		this.commit();
		this.observers.forEach((observer) => observer.onThreadsInitialized?.(this.snapshot.threads));
	}
	public addThread(thread: ProjectThread): void {
		this.threads.unshift({
			...thread,
		});
		this.sortThreads();
		this.commit();
	}
	public updateThread(thread: ProjectThread): void {
		const index = this.threads.findIndex((candidate) => candidate.serverId === thread.serverId);
		if (index === -1) {
			this.addThread(thread);
			return;
		}
		this.threads[index] = {
			...thread,
		};
		this.sortThreads();
		this.commit();
	}
	public removeThreadByTaskId(taskId: string): void {
		const next = this.threads.filter((thread) => thread.taskId !== taskId);
		if (next.length === this.threads.length) return;
		this.threads = next;
		this.commit();
	}
	public openThread(serverId: string): void {
		const thread = this.getEligibleThreads().find((candidate) => candidate.serverId === serverId);
		if (thread) this.options.onOpenThread?.(thread);
	}
	private getEligibleThreads(): ProjectThread[] {
		return this.threads.filter((thread) =>
			thread.type === 'ai-mode'
				? (this.options.canShowAiModeThreads?.() ?? true)
				: (this.options.canShowGeminiThreads?.() ?? true),
		);
	}
	private sortThreads(): void {
		this.threads.sort((left, right) => right.lastTurnTime - left.lastTurnTime);
	}
	private commit(): void {
		this.revision += 1;
		this.snapshot = Object.freeze({
			revision: this.revision,
			tabGroups: Object.freeze(
				this.groups.map((group) =>
					Object.freeze({
						...group,
					}),
				),
			),
			threads: Object.freeze(
				this.getEligibleThreads().map((thread) =>
					Object.freeze({
						...thread,
					}),
				),
			),
		});
		this.listeners.forEach((listener) => listener());
	}
}
