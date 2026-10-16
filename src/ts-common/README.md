# Shared TypeScript code

This directory regroups types and helpers that can be used both on the main
thread and in a Web Worker.

`types.ts` defines the messages exchanged between threads, alongside shared
configuration, content information and error descriptions. It describes what
messages contain; it doesn't send or handle them.

`QueuedSourceBuffer.ts` wraps a browser SourceBuffer so append and remove
operations happen one after another, with a Promise for each result. It keeps
the pending operations and the listeners needed to follow them. There should
only be one wrapper per SourceBuffer. When an operation fails, queued operations
are cancelled; `dispose` removes listeners and rejects unfinished work.

The other files provide small helpers: an event emitter, logging, identifier
generation, a monotonic clock, time range conversion etc.
`generatedWasmEnums.ts` contains shared numeric codes and is automatically
generated. It should not be modified by hand; use `npm run generate` after
changing the ABI definitions.

## Where it stops

The objects here keep their own data, such as listeners or queued operations.
They don't own the current content or decide what to request and buffer.

Shared code doesn't mean shared memory: the main thread and worker have their
own instances. Also, being usable in either thread doesn't mean every helper
works everywhere. `QueuedSourceBuffer` needs an actual browser SourceBuffer in
the thread where it is used.
