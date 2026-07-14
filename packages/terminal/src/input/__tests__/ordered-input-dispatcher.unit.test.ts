import { describe, expect, it, vi } from "vitest";
import { OrderedInputDispatcher } from "../ordered-input-dispatcher.js";

describe("OrderedInputDispatcher", () => {
  it("holds synchronous input behind an active asynchronous decision", async () => {
    const order: string[] = [];
    let release: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const dispatcher = new OrderedInputDispatcher();
    const first = dispatcher.dispatch(async () => {
      order.push("paste-start");
      await gate;
      order.push("paste-end");
    });
    dispatcher.dispatchSync(() => order.push("typing"));

    await Promise.resolve();
    expect(order).toEqual(["paste-start"]);
    release?.();
    await first;
    await dispatcher.dispatch(() => order.push("barrier"));
    expect(order).toEqual(["paste-start", "paste-end", "typing", "barrier"]);
  });

  it("reports a deferred synchronous failure and continues the queue", async () => {
    const error = new Error("listener failed");
    const onError = vi.fn<(value: unknown) => void>();
    const dispatcher = new OrderedInputDispatcher(onError);
    let release: (() => void) | undefined;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const first = dispatcher.dispatch(() => gate);
    dispatcher.dispatchSync(() => {
      throw error;
    });
    release?.();
    await first;
    await dispatcher.dispatch(() => undefined);

    expect(onError).toHaveBeenCalledWith(error);
    expect(dispatcher.isPending).toBe(false);
  });
});
