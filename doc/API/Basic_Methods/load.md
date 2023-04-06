# `load` method

## Description

Load a content see the [API documentation page on loading a
content](../Loading_a_content.md) for more information.

## Syntax

```js
// Without options
player.load(url);

// With options
player.load(url, {
  startingPosition: initialWantedPosition,
});
```

- **arguments**:

  1. _url_ `string`: Url to the Multivariant Playlist of the content you want to
     play.

  2. _options_ `Object`: Optional argument to configure how the content will be
     loaded.

     Can contain the following keys:

     - `startType` (`number`|`object`|`undefined`): indicate a preferred
       starting position. See [the documentation page on loading a
       content](../Loading_a_content.md) for more information
