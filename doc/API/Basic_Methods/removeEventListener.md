# `removeEventListener` method

## Description

Removes an event listener previously registered through the
[`addEventListener`](./addEventListener.md) method.
This also free-up the corresponding ressources.

The callback given is optional: if not given, _every_ registered callback to
that event will be removed.

For example, to add then remove an event listener for the `"playerStateChange"`
event, you can write:

```js
const onPlayerStateChange = (state) => {
  console.log("new player state:", state);
};
player.addEventListener("playerStateChange", onPlayerStateChange);
player.removeEventListener("playerStateChange", onPlayerStateChange);
```

## Syntax

```js
// Remove all callbacks linked to event
player.removeEventListener(event);

// Remove specific listener
player.removeEventListener(event, callback);
```

- **arguments**:

  1.  _event_ `string`: The event name.

  2.  _callback_ (optional) `Function`|`undefined`: The callback given when
      calling the corresponding `addEventListener` API.
