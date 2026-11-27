import type { GitPrompt, GitRelayEvent } from '../../../file-session';
import { notifyEachListener } from '../../contained-client-fault';
import { FILES_RELAY_STREAM } from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';
import { ServerFileEvents } from './ServerFileEvents';

interface RelayFollower {
	readonly listener: (event: GitRelayEvent) => void;
	/** The follower has had every prompt open when it joined. */
	primed: boolean;
}

/**
 * A session's `relay.subscribe`, shared by every follower and open while any follows. The Server
 * routes a prompt to one subscribed connection and routes it again when that subscription ends, so
 * a stream that ends closes every prompt it delivered, while a gap keeps them until the
 * replacement snapshot says which are still open. A follower that joins later gets the open prompts
 * first.
 */
export class ServerGitRelayStream {
	readonly #events: ServerFileEvents;
	readonly #followers = new Set<RelayFollower>();
	#prompts = new Map<string, GitPrompt>();

	constructor(channel: ServerFileChannel, workspaceId: string) {
		this.#events = new ServerFileEvents(channel, {
			failure: (record) => FILES_RELAY_STREAM.decodeFailure(record),
			lose: (ended) => {
				if (ended) this.#replace([]);
			},
			notice: (kind, record) => {
				const notice = FILES_RELAY_STREAM.decodeNotice(kind, record);
				if (notice === null) return false;
				for (const follower of this.#followers) this.#prime(follower);
				if (notice.kind === 'snapshot') this.#replace(notice.snapshot);
				else if (notice.update.kind === 'prompt') this.#open(notice.update.prompt);
				else this.#close(notice.update.id);
				return true;
			},
			request: () => FILES_RELAY_STREAM.encode({ op: 'relay.subscribe', workspaceId }),
		});
	}

	subscribe(listener: (event: GitRelayEvent) => void): () => void {
		if (!this.#events.live) return () => undefined;
		const follower: RelayFollower = { listener, primed: false };
		this.#followers.add(follower);
		queueMicrotask(() => this.#prime(follower));
		this.#events.open();
		return () => {
			if (!this.#followers.delete(follower) || this.#followers.size > 0) return;
			this.#events.shut();
		};
	}

	served(): void {
		this.#events.served();
	}

	end(): void {
		this.#events.end();
	}

	close(): void {
		this.#followers.clear();
		this.#events.close();
	}

	/** Takes the open prompts a snapshot lists: the others close and the new ones open. */
	#replace(prompts: readonly GitPrompt[]): void {
		const next = new Map(prompts.map((prompt) => [prompt.id, prompt]));
		for (const id of this.#prompts.keys()) {
			if (!next.has(id)) this.#close(id);
		}
		for (const prompt of next.values()) this.#open(prompt);
	}

	#open(prompt: GitPrompt): void {
		if (this.#prompts.has(prompt.id)) return;
		this.#prompts.set(prompt.id, prompt);
		this.#send({ kind: 'prompt', prompt });
	}

	#close(id: string): void {
		if (!this.#prompts.delete(id)) return;
		this.#send({ id, kind: 'closed' });
	}

	#prime(follower: RelayFollower): void {
		if (follower.primed || !this.#followers.has(follower)) return;
		follower.primed = true;
		const prompts = [...this.#prompts.values()];
		notifyEachListener([follower], 'source-listener', (target) => {
			for (const prompt of prompts) target.listener({ kind: 'prompt', prompt });
		});
	}

	#send(event: GitRelayEvent): void {
		const primed = [...this.#followers].filter((follower) => follower.primed);
		notifyEachListener(primed, 'source-listener', (follower) => follower.listener(event));
	}
}
