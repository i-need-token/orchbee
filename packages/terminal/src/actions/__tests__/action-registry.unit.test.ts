import { describe, expect, it, vi } from "vitest";
import { TerminalActionRegistry } from "../action-registry";
import type {
  TerminalActionContext,
  TerminalActionDefinition,
  TerminalActionResult,
} from "../action-registry";

function context(source: TerminalActionContext["source"] = "api"): TerminalActionContext {
  return { source, signal: new AbortController().signal };
}

describe("TerminalActionRegistry", () => {
  it("registers, executes, and disposes actions", async () => {
    const registry = new TerminalActionRegistry();
    const run = vi.fn<TerminalActionDefinition["run"]>(async () => ({ kind: "consumed" }));
    const registration = registry.register({
      id: "terminal.test",
      title: "Test action",
      scope: "surface",
      run,
    });

    await expect(registry.execute("terminal.test", context("keyboard"))).resolves.toEqual({
      kind: "consumed",
    });
    expect(run).toHaveBeenCalledWith(expect.objectContaining({ source: "keyboard" }));
    registration.dispose();
    await expect(registry.execute("terminal.test", context())).resolves.toEqual({
      kind: "unsupported",
    });
    registry.dispose();
  });

  it("reports enabled and checked state and blocks disabled actions", async () => {
    const registry = new TerminalActionRegistry();
    const run = vi.fn<
      (context: TerminalActionContext) => TerminalActionResult | Promise<TerminalActionResult>
    >(() => ({ kind: "consumed" }));
    registry.register({
      id: "terminal.toggle",
      title: "Toggle",
      scope: "surface",
      isEnabled: () => false,
      isChecked: () => true,
      run,
    });

    expect(registry.getState("terminal.toggle", context())).toEqual({
      enabled: false,
      checked: true,
    });
    await expect(registry.execute("terminal.toggle", context())).resolves.toEqual({
      kind: "blocked",
      reason: "disabled",
    });
    expect(run).not.toHaveBeenCalled();
    registry.dispose();
  });

  it("rejects duplicate action identifiers", () => {
    const registry = new TerminalActionRegistry();
    const definition = {
      id: "terminal.duplicate",
      title: "Duplicate",
      scope: "surface" as const,
      run: () => ({ kind: "consumed" }) as const,
    };
    registry.register(definition);
    expect(() => registry.register(definition)).toThrow("already registered");
    registry.dispose();
  });

  it("blocks execution through an aborted context", async () => {
    const registry = new TerminalActionRegistry();
    const controller = new AbortController();
    controller.abort();
    registry.register({
      id: "terminal.abort",
      title: "Abort",
      scope: "surface",
      run: () => ({ kind: "consumed" }),
    });
    await expect(
      registry.execute("terminal.abort", { source: "api", signal: controller.signal }),
    ).resolves.toEqual({ kind: "blocked", reason: "aborted" });
    registry.dispose();
  });
});
