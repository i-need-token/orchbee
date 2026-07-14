import { describe, expect, it } from "vitest";
import { EventEmitter } from "../event-emitter";

describe("EventEmitter", () => {
  it("fires listeners synchronously and disposes subscriptions", () => {
    const emitter = new EventEmitter<number>();
    const values: number[] = [];
    const first = emitter.event((value) => values.push(value));
    emitter.event((value) => values.push(value * 10));
    emitter.fire(2);
    first.dispose();
    emitter.fire(3);
    expect(values).toEqual([2, 20, 30]);
  });

  it("removes every listener when disposed", () => {
    const emitter = new EventEmitter<string>();
    const values: string[] = [];
    emitter.event((value) => values.push(value));
    emitter.dispose();
    emitter.fire("ignored");
    expect(values).toEqual([]);
  });
});
