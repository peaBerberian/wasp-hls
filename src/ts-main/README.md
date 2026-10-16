# Main thread code

This directory is where we define the JavaScript API used by applications and
handle interactions with the page's media element. The entry point is
`WaspHlsPlayer` in `api.ts`, exported alongside public types and errors through
`index.ts`.

Its job is to:

- Create and initialize the worker, send commands to it and receive its messages.

- Keep the state exposed by the API: current content information, tracks,
  variants, errors, playback settings etc. Changes are also sent to applications
  through events.

- Act on the media element, for example to pause, resume, seek or attach a media
  source, and send playback observations back to the worker.

- Perform media buffer operations on the main thread when they cannot run in
  the worker. When they run in the worker, we still attach its media source to
  the media element here.

- Remove listeners, stop observations and free the resources created for a
  content or player when they are no longer needed.

`worker-message-handlers.ts` handles actions requested by the worker and updates
the content state. Messages are checked against the current content or media
source so that old results don't affect a new content.

`observePlayback.ts` reads media element properties on events and at regular
intervals. `types.ts` describes the state kept here, `default_config.ts` contains
default settings, and `errors/` defines the errors exposed to applications.
`postMessageToWorker.ts` sends messages to the worker.

## Where it stops

This code owns the worker connection, its content metadata and the listeners and
browser resources it creates. The media element is provided by the application.

It doesn't choose which segments to load, parse playlists or estimate bandwidth.
Those decisions happen in the worker. Some API methods act on the media element
directly, others send a message: asking for a change doesn't always mean it has
already been applied.
