import type { FileContent, FileReadOptions } from './file-content-port';
import type { FileError, FileResult } from './file-error';

/** A repository the Workspace's discovery found, as the SCM summary reports it. */
export interface GitRepositorySummary {
	/** A Git operation of this repository runs now. */
	readonly busy: boolean;
	readonly counts: {
		readonly index: number;
		readonly merge: number;
		readonly untracked: number;
		readonly workingTree: number;
	};
	readonly head: {
		readonly branch: string | null;
		readonly commit: string | null;
		readonly detached: boolean;
		readonly unborn: boolean;
	};
	/** How discovery reached it: the root itself, below it, above it, or through an opened file. */
	readonly kind: 'file' | 'nested' | 'parent' | 'root';
	readonly operation: 'cherry-pick' | 'merge' | 'none' | 'rebase' | 'revert';
	/** The canonical toplevel path, which names the repository in every Git request. */
	readonly repository: string;
	/** The status this summary counts; a status page names it and fails after a newer one. */
	readonly revision: number;
	/** The status is older than the working tree, while a refresh is due. */
	readonly stale: boolean;
	/** The status stopped at its bound, so the counts are lower bounds. */
	readonly truncated: boolean;
	readonly upstream: {
		readonly ahead: number;
		readonly behind: number;
		readonly name: string;
	} | null;
	/** A status of it was read, so its head, counts and status pages hold. */
	readonly warm: boolean;
}

/** The Workspace's whole Source Control state; every update carries all of it. */
export interface GitSummary {
	/** Parent repositories that wait for the person's consent. */
	readonly candidates: readonly string[];
	readonly git: { readonly state: 'missing' | 'ok' | 'too-old'; readonly version: string | null };
	readonly repositories: readonly GitRepositorySummary[];
	readonly repositoriesTruncated: boolean;
	/** Repositories Git refuses as unsafe until the person marks them safe. */
	readonly unsafe: readonly string[];
}

/** One record of the Workspace's Source Control stream. */
export type GitSummaryEvent =
	/** Records were lost; the next summary is whole, so nothing needs to be asked again. */
	| { readonly kind: 'gap' }
	| { readonly kind: 'summary'; readonly sequence: number; readonly summary: GitSummary };

/** One changed path in porcelain order: `x` is the index's status letter, `y` the working tree's. */
export interface GitStatusEntry {
	/** The source of a rename or copy. */
	readonly original: string | null;
	readonly path: string;
	readonly submodule: boolean;
	readonly x: string;
	readonly y: string;
}

/** One page of a repository's status at one revision. */
interface GitStatusPage {
	/** Present while more pages follow. */
	readonly cursor: string | null;
	readonly entries: readonly GitStatusEntry[];
}

/** A branch, remote branch or tag, by VS Code's `RefType`. */
export interface GitRef {
	readonly commit: string;
	readonly name: string;
	readonly type: 'head' | 'remote-head' | 'tag';
	readonly upstream: string | null;
}

export interface GitRemote {
	readonly fetchUrl: string | null;
	readonly name: string;
	readonly pushUrl: string | null;
}

interface GitRefsPage {
	readonly cursor: string | null;
	readonly refs: readonly GitRef[];
	readonly remotes: readonly GitRemote[];
}

/** New Git Output text since a cursor; `dropped` counts text the bounded log lost meanwhile. */
interface GitOutputPage {
	readonly cursor: string;
	readonly dropped: number;
	readonly text: string;
}

/** A Git change, which runs at most once per request ID. */
export type GitMutation =
	| {
			readonly op: 'discard' | 'stage' | 'unstage';
			readonly paths: readonly string[];
			readonly repository: string;
	  }
	| {
			readonly op: 'stage-content';
			readonly path: string;
			readonly repository: string;
			/** The content the index takes for the path, as from Unstage Selected Ranges. */
			readonly text: string;
	  }
	| {
			readonly mode: 'permanent' | 'trash';
			readonly op: 'clean';
			readonly paths: readonly string[];
			readonly repository: string;
	  }
	| {
			readonly all: 'all' | 'none' | 'tracked';
			readonly amend: boolean;
			/** The message, or `editor` to write it in a `COMMIT_EDITMSG` tab through the relay. */
			readonly message:
				| { readonly kind: 'editor' }
				| { readonly kind: 'text'; readonly text: string };
			readonly noVerify: boolean;
			readonly op: 'commit';
			readonly repository: string;
			readonly signoff: boolean;
	  }
	| {
			/** A background fetch never prompts. */
			readonly background: boolean;
			readonly op: 'fetch';
			readonly prune: boolean;
			readonly remote:
				| { readonly kind: 'all' }
				| { readonly kind: 'default' }
				| { readonly kind: 'named'; readonly name: string };
			readonly repository: string;
	  }
	| { readonly op: 'pull' | 'sync'; readonly rebase: boolean; readonly repository: string }
	| {
			readonly branch: string | null;
			readonly force: 'force' | 'lease' | 'none';
			readonly op: 'push';
			readonly remote: string | null;
			readonly repository: string;
			/** Publish: set the pushed branch's upstream. */
			readonly setUpstream: boolean;
			readonly tags: boolean;
	  }
	| { readonly op: 'init' | 'mark-safe'; readonly path: string };

/** What a Git change returned. */
export type GitMutationOutput =
	| {
			readonly items: readonly { readonly error: FileError | null; readonly path: string }[];
			readonly kind: 'cleaned';
	  }
	| { readonly commit: string; readonly kind: 'committed' }
	| { readonly kind: 'done' }
	| { readonly kind: 'initialized'; readonly repository: string };

/** What the backend knows of a request ID it was sent. */
export type GitRequestOutcome =
	| { readonly result: FileResult<GitMutationOutput>; readonly state: 'completed' }
	| { readonly state: 'running' }
	/** Never received, or no longer remembered: it is safe to send again with the same ID. */
	| { readonly state: 'unknown' };

/** A question Git asked through the relay, for the window viewing its Workspace. */
export interface GitPrompt {
	readonly id: string;
	readonly kind: 'editor' | 'host-key' | 'passphrase' | 'password' | 'username';
	/** The file an `editor` prompt names inside the repository's Git directory. */
	readonly path: string | null;
	readonly repository: string;
	/** Git's own question, shown verbatim. */
	readonly text: string;
}

/** One record of the Workspace's relay stream. */
export type GitRelayEvent =
	| { readonly id: string; readonly kind: 'closed' }
	| { readonly kind: 'prompt'; readonly prompt: GitPrompt };

export type GitPromptAnswer =
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'value'; readonly value: string };

/** Source Control for one Workspace: its summary, status, contents at a reference and changes. */
export interface GitPort {
	answer(promptId: string, answer: GitPromptAnswer, signal: AbortSignal): Promise<FileResult<null>>;
	checkIgnore(
		request: { readonly paths: readonly string[]; readonly repository: string },
		signal: AbortSignal,
	): Promise<FileResult<readonly string[]>>;
	/**
	 * Tells the backend whether the window has focus while it shows this Workspace: automatic status
	 * refreshes after file changes wait for a focused Client.
	 */
	focus(focused: boolean): void;
	/** Answers whether a change sent with this ID ran, so an uncertain one is never repeated. */
	outcome(requestId: string, signal: AbortSignal): Promise<FileResult<GitRequestOutcome>>;
	output(cursor: string | null, signal: AbortSignal): Promise<FileResult<GitOutputPage>>;
	refs(
		request: { readonly cursor?: string | undefined; readonly repository: string },
		signal: AbortSignal,
	): Promise<FileResult<GitRefsPage>>;
	/** Runs a change once: a repeated request ID returns the recorded outcome without running. */
	run(
		requestId: string,
		mutation: GitMutation,
		signal: AbortSignal,
	): Promise<FileResult<GitMutationOutput>>;
	/** Reads a path at `HEAD`, the index or a commit, as a file read does. */
	show(
		request: FileReadOptions & {
			readonly path: string;
			readonly ref: string;
			readonly repository: string;
		},
		signal: AbortSignal,
	): Promise<FileResult<FileContent>>;
	statusPage(
		request: {
			readonly cursor?: string | undefined;
			readonly repository: string;
			readonly revision: number;
		},
		signal: AbortSignal,
	): Promise<FileResult<GitStatusPage>>;
	/** Follows the Source Control stream: the current summary first, then every new one. */
	subscribe(listener: (event: GitSummaryEvent) => void): () => void;
	/** Follows the relay: Git's prompts for this Workspace and their closing. */
	subscribeRelay(listener: (event: GitRelayEvent) => void): () => void;
	userConfig(
		repository: string,
		signal: AbortSignal,
	): Promise<FileResult<{ readonly email: string | null; readonly name: string | null }>>;
}
