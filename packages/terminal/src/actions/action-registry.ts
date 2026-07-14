import { EventEmitter, type IDisposable, type IEvent } from "../event-emitter";
import type { TerminalInteractionContext } from "../host/terminal-host";

export type TerminalActionSource =
  | "keyboard"
  | "context-menu"
  | "command-palette"
  | "automation"
  | "pointer"
  | "api";

export interface TerminalActionContext {
  readonly source: TerminalActionSource;
  readonly signal: AbortSignal;
  readonly metadata?: Readonly<Record<string, unknown>>;
  readonly interaction?: TerminalInteractionContext | null;
}

export type TerminalActionResult =
  | { readonly kind: "consumed" }
  | { readonly kind: "passthrough" }
  | { readonly kind: "blocked"; readonly reason: string }
  | { readonly kind: "unsupported" };

export interface TerminalActionDefinition {
  readonly id: string;
  readonly title: string;
  readonly scope: "surface" | "application";
  isEnabled?(context: TerminalActionContext): boolean;
  isChecked?(context: TerminalActionContext): boolean;
  run(context: TerminalActionContext): TerminalActionResult | Promise<TerminalActionResult>;
}

export interface TerminalActionState {
  readonly enabled: boolean;
  readonly checked: boolean;
}

export class TerminalActionRegistry implements IDisposable {
  private readonly definitions = new Map<string, TerminalActionDefinition>();
  private readonly onDidChangeEmitter = new EventEmitter<string | null>();

  get onDidChange(): IEvent<string | null> {
    return this.onDidChangeEmitter.event;
  }

  register(definition: TerminalActionDefinition): IDisposable {
    if (this.definitions.has(definition.id)) {
      throw new Error(`Terminal action "${definition.id}" is already registered`);
    }
    this.definitions.set(definition.id, definition);
    this.onDidChangeEmitter.fire(definition.id);
    return {
      dispose: () => {
        if (this.definitions.get(definition.id) !== definition) return;
        this.definitions.delete(definition.id);
        this.onDidChangeEmitter.fire(definition.id);
      },
    };
  }

  get(id: string): TerminalActionDefinition | undefined {
    return this.definitions.get(id);
  }

  list(): readonly TerminalActionDefinition[] {
    return [...this.definitions.values()];
  }

  getState(id: string, context: TerminalActionContext): TerminalActionState | null {
    const definition = this.definitions.get(id);
    if (!definition) return null;
    return {
      enabled: definition.isEnabled?.(context) ?? true,
      checked: definition.isChecked?.(context) ?? false,
    };
  }

  async execute(id: string, context: TerminalActionContext): Promise<TerminalActionResult> {
    const definition = this.definitions.get(id);
    if (!definition) return { kind: "unsupported" };
    if (context.signal.aborted) return { kind: "blocked", reason: "aborted" };
    if (!(definition.isEnabled?.(context) ?? true)) {
      return { kind: "blocked", reason: "disabled" };
    }
    return await definition.run(context);
  }

  notifyStateChange(id?: string): void {
    this.onDidChangeEmitter.fire(id ?? null);
  }

  dispose(): void {
    if (this.definitions.size > 0) {
      this.definitions.clear();
      this.onDidChangeEmitter.fire(null);
    }
    this.onDidChangeEmitter.dispose();
  }
}
