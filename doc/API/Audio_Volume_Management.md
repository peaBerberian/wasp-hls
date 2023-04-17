# Audio Volume Management

## Overview

Updating and getting the audio volume is done differently than most other API of
the `WaspHlsPlayer` in that we encourage you to use the browser's native means
to do so by directly updating property from the `<video>` element you gave to
the `WaspHlsPlayer` on [instantiation](./Instantiation.md).

For example, to update the audio volume, you can update the video element's
[volume](https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/volume)
property by setting it to a value between `0` - indicating the quietest sound, to
`1` indicating the loudest sound:

```js
const videoElement = document.getElementsByTagName("video")[0];

// Full volume
videoElement.volume = 1;
```

To explicitely mute or unmute the audio volume, it is also possible to use the
[muted](https://developer.mozilla.org/en-US/docs/Web/API/HTMLMediaElement/muted)
property, also found on the `<video>` element. Using that property has the
advantage of keeping the `volume` property to its last set value (for cases
where you want to un-mute to the previous value).
