export const PLAYER_LOAD_TIMEOUT_MS = 90_000;
export const PLAYER_EVENT_TIMEOUT_MS = 5 * 60_000;

// Leave room for setup, loading and several sequential event waits on slow CI machines.
export const TEST_TIMEOUT_MS = 10 * 60_000;
