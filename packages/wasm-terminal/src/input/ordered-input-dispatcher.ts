/** Serializes async input policy decisions without delaying the idle synchronous path. */
export class OrderedInputDispatcher {
  private tail = Promise.resolve();
  private pending = 0;
  private readonly onDeferredError: (error: unknown) => void;

  constructor(onDeferredError: (error: unknown) => void = () => {}) {
    this.onDeferredError = onDeferredError;
  }

  get isPending(): boolean {
    return this.pending > 0;
  }

  dispatch<Result>(operation: () => Result | Promise<Result>): Promise<Result> {
    this.pending++;
    const result = this.tail.then(operation);
    this.tail = result.then(
      () => {
        this.pending--;
      },
      () => {
        this.pending--;
      },
    );
    return result;
  }

  dispatchSync(operation: () => void): void {
    if (!this.isPending) {
      operation();
      return;
    }
    void this.dispatch(operation).catch(this.onDeferredError);
  }
}
