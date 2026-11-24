import type { DesktopMachineSessionRefusal } from '@orchbee/desktop-client-protocol';

import { ServerConnection, ServerWorkbenchSession } from '../../workbench-client';
import { MessagePortServerCarrier } from './MessagePortServerCarrier';
import type { MachineSessionAnswer } from './requestMachineSession';

/** How long a connection that failed for a passing reason waits before it is asked for again. */
const RETRY_MILLISECONDS = 5000;

/** Refusals that pass on their own; any other holds until the machine is read online anew. */
const PASSING: ReadonlySet<DesktopMachineSessionRefusal> = new Set([
	'hub-unavailable',
	'link-lost',
	'protocol',
	'timeout',
	'transient',
]);

/** One machine's Session, reached through its relay. */
export interface DesktopRemoteSession {
	/** Every capability the Session's handshake answer listed. */
	readonly capabilities: readonly string[];
	readonly connection: ServerConnection;
	readonly session: ServerWorkbenchSession;
}

type Held =
	| { readonly kind: 'opening' }
	| { readonly kind: 'held' }
	| { readonly kind: 'waiting'; readonly timer: ReturnType<typeof setTimeout> }
	| { readonly kind: 'open'; readonly remote: DesktopRemoteSession; readonly stop: () => void };

/**
 * The window's connections to other machines' Sessions: one for every machine it is told to follow,
 * all at once and as many as there are, so moving to another machine's Workspace never waits for a
 * connection. A connection that ends, or fails for a passing reason, is asked for again while its
 * machine is followed. Ending one never stops the remote Session.
 */
export class DesktopRemoteSessions {
	readonly #held = new Map<string, Held>();
	readonly #listeners = new Set<() => void>();
	readonly #request: (machine: string) => Promise<MachineSessionAnswer>;
	#followed: ReadonlySet<string> = new Set();
	#disposed = false;

	constructor(request: (machine: string) => Promise<MachineSessionAnswer>) {
		this.#request = request;
	}

	/** Holds a connection for exactly `machines`: the ones the machines' hub reads online. */
	follow(machines: ReadonlySet<string>): void {
		if (this.#disposed) return;
		this.#followed = machines;
		for (const [machine, held] of this.#held) {
			if (!machines.has(machine)) this.#drop(machine, held);
		}
		for (const machine of machines) {
			if (!this.#held.has(machine)) this.#open(machine);
		}
	}

	get(machine: string): DesktopRemoteSession | undefined {
		const held = this.#held.get(machine);
		return held?.kind === 'open' ? held.remote : undefined;
	}

	/** Runs `listener` when a connection opens or ends, or a Session publishes. */
	subscribe(listener: () => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	dispose(): void {
		this.#disposed = true;
		for (const [machine, held] of this.#held) this.#drop(machine, held);
		this.#listeners.clear();
	}

	#open(machine: string): void {
		const opening: Held = { kind: 'opening' };
		this.#held.set(machine, opening);
		void this.#connect(machine).then((outcome) => {
			if (this.#held.get(machine) !== opening) {
				if (outcome.kind === 'open') close(outcome.remote);
				return;
			}
			if (outcome.kind === 'failed') {
				if (outcome.passing) this.#wait(machine);
				else this.#held.set(machine, { kind: 'held' });
				return;
			}
			const { remote } = outcome;
			const stopPublication = remote.session.subscribe(() => this.#notify());
			const stopEnd = remote.session.onEnded(() => {
				const held = this.#held.get(machine);
				if (held?.kind !== 'open' || held.remote !== remote) return;
				this.#drop(machine, held);
				if (this.#followed.has(machine)) this.#wait(machine);
			});
			this.#held.set(machine, {
				kind: 'open',
				remote,
				stop: () => {
					stopPublication();
					stopEnd();
				},
			});
			this.#notify();
		});
	}

	async #connect(
		machine: string,
	): Promise<
		| { readonly kind: 'failed'; readonly passing: boolean }
		| { readonly kind: 'open'; readonly remote: DesktopRemoteSession }
	> {
		const answer = await this.#request(machine);
		if (answer.kind === 'refused') return { kind: 'failed', passing: PASSING.has(answer.reason) };
		const connection = new ServerConnection(new MessagePortServerCarrier(answer.port));
		const session = await ServerWorkbenchSession.open(connection).catch(() => null);
		if (session === null) {
			connection.close();
			return { kind: 'failed', passing: true };
		}
		return { kind: 'open', remote: { capabilities: answer.capabilities, connection, session } };
	}

	#wait(machine: string): void {
		const timer = setTimeout(() => {
			if (this.#held.get(machine)?.kind !== 'waiting') return;
			this.#held.delete(machine);
			if (this.#followed.has(machine) && !this.#disposed) this.#open(machine);
		}, RETRY_MILLISECONDS);
		this.#held.set(machine, { kind: 'waiting', timer });
	}

	#drop(machine: string, held: Held): void {
		this.#held.delete(machine);
		if (held.kind === 'waiting') clearTimeout(held.timer);
		if (held.kind !== 'open') return;
		held.stop();
		close(held.remote);
		this.#notify();
	}

	#notify(): void {
		for (const listener of this.#listeners) listener();
	}
}

function close(remote: DesktopRemoteSession): void {
	remote.session.dispose();
	remote.connection.close();
}
