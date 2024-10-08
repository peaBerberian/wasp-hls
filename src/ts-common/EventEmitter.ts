import logger from "./logger";

/**
 * Generic type for a non-emitting event emitter (can only be relied on for
 * listening).
 */
export interface EventEmitterListenOnly<T> {
  addEventListener<TEventName extends keyof T>(
    evt: TEventName,
    fn: EventListener<T, TEventName>,
  ): void;
  removeEventListener<TEventName extends keyof T>(
    evt: TEventName,
    fn: EventListener<T, TEventName>,
  ): void;
}

/** Type of the argument in the listener's callback */
export type EventPayload<
  TEventRecord,
  TEventName extends keyof TEventRecord,
> = TEventRecord[TEventName];

/** Type of the listener function. */
export type EventListener<
  TEventRecord,
  TEventName extends keyof TEventRecord,
> = (args: EventPayload<TEventRecord, TEventName>) => void;

/** Type for the object storing all current listener functions. */
type ListenersObject<TEventRecord> = {
  [P in keyof TEventRecord]?: Array<EventListener<TEventRecord, P>>;
};

/**
 * Simple but fully type-safe EventEmitter implementation.
 * @class EventEmitter
 */
export default class EventEmitter<T> implements EventEmitterListenOnly<T> {
  /**
   * @type {Object}
   * @private
   */
  private _listeners: ListenersObject<T>;

  constructor() {
    this._listeners = {};
  }

  /**
   * Register a new callback for an event.
   *
   * @param {string} evt - The event to register a callback to
   * @param {Function} fn - The callback to call as that event is triggered.
   * The callback will take as argument the eventual payload of the event
   * (single argument).
   */
  public addEventListener<TEventName extends keyof T>(
    evt: TEventName,
    fn: EventListener<T, TEventName>,
  ): void {
    const listeners = this._listeners[evt];
    if (!Array.isArray(listeners)) {
      this._listeners[evt] = [fn];
    } else {
      listeners.push(fn);
    }
  }

  /**
   * Unregister callbacks linked to events.
   * @param {string} [evt] - The event for which the callback[s] should be
   * unregistered. Set it to null or undefined to remove all callbacks
   * currently registered (for any event).
   * @param {Function} [fn] - The callback to unregister. If set to null
   * or undefined while the evt argument is set, all callbacks linked to that
   * event will be unregistered.
   */
  public removeEventListener<TEventName extends keyof T>(
    evt?: TEventName | undefined,
    fn?: EventListener<T, TEventName> | undefined,
  ): void {
    if (evt === undefined) {
      this._listeners = {};
      return;
    }

    const listeners = this._listeners[evt];
    if (!Array.isArray(listeners)) {
      return;
    }
    if (fn === undefined) {
      delete this._listeners[evt];
      return;
    }

    const index = listeners.indexOf(fn);
    if (index !== -1) {
      listeners.splice(index, 1);
    }

    if (listeners.length === 0) {
      delete this._listeners[evt];
    }
  }

  /**
   * Trigger every registered callbacks for a given event
   * @param {string} evt - The event to trigger
   * @param {*} arg - The eventual payload for that event. All triggered
   * callbacks will recieve this payload as argument.
   */
  protected trigger<TEventName extends keyof T>(
    evt: TEventName,
    arg: EventPayload<T, TEventName>,
  ): void {
    const listeners = this._listeners[evt];
    if (!Array.isArray(listeners)) {
      return;
    }

    listeners.slice().forEach((listener) => {
      try {
        listener(arg);
      } catch (e) {
        logger.error(
          "EventEmitter: listener error",
          e instanceof Error ? e : null,
        );
      }
    });
  }
}
