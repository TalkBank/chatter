import type { ValidationEvent } from "../../protocol/validation";

/** A delivery owns at least one event and retains their original order. */
export type ValidationEventBatch = readonly [ValidationEvent, ...ValidationEvent[]];

/** Schedule a later frame and return its cancellation action; never call inline. */
export type FrameScheduler = (flush: () => void) => () => void;

export const animationFrame: FrameScheduler = (flush) => {
  const frame = requestAnimationFrame(flush);
  return () => cancelAnimationFrame(frame);
};

type BatchState =
  | { kind: "idle" }
  | { kind: "closed" }
  | {
      kind: "queued";
      events: [ValidationEvent, ...ValidationEvent[]];
      completed: number;
      cancelFrame: () => void;
    };

/** Run-owned buffering: frame/50-file flushes, immediate lifecycle/ending delivery. */
export class ValidationBatcher {
  private state: BatchState = { kind: "idle" };

  constructor(
    private readonly deliver: (events: ValidationEventBatch) => void,
    private readonly schedule: FrameScheduler = animationFrame,
  ) {}

  push(event: ValidationEvent): void {
    if (this.state.kind === "closed") return;
    switch (event.type) {
      case "errors":
      case "fileComplete": {
        if (this.state.kind === "idle") {
          const events: [ValidationEvent, ...ValidationEvent[]] = [event];
          this.state = {
            kind: "queued", events, completed: event.type === "fileComplete" ? 1 : 0,
            cancelFrame: this.schedule(() => this.flush(events)),
          };
        } else {
          this.state.events.push(event);
          if (event.type === "fileComplete") this.state.completed++;
        }
        if (this.state.completed >= 50) this.flush(this.state.events);
        break;
      }
      case "cacheUnavailable":
      case "discovering":
      case "started":
        this.deliverThrough(event, { kind: "idle" });
        break;
      case "finished":
      case "finishedIncomplete":
      case "stopped":
      case "aborted":
      case "nothingFound":
        this.deliverThrough(event, { kind: "closed" });
        break;
      default: {
        const unknown: never = event;
        throw new Error(`Unknown validation event: ${JSON.stringify(unknown)}`);
      }
    }
  }

  private flush(expected: ValidationEventBatch): void {
    if (this.state.kind !== "queued" || this.state.events !== expected) return;
    const queued = this.state;
    this.state = { kind: "idle" };
    queued.cancelFrame();
    this.deliver(queued.events);
  }

  private deliverThrough(event: ValidationEvent, next: { kind: "idle" } | { kind: "closed" }): void {
    const queued = this.state;
    this.state = next;
    if (queued.kind === "queued") {
      queued.cancelFrame();
      queued.events.push(event);
      this.deliver(queued.events);
    } else {
      this.deliver([event]);
    }
  }

  dispose(): void {
    const previous = this.state;
    this.state = { kind: "closed" };
    if (previous.kind === "queued") previous.cancelFrame();
  }
}
