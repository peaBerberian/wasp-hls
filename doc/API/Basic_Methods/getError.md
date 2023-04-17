# `getError` method

## description

If the player is currently in the `"Error"` [state](./getPlayerState.md),
returns the corresponding [error object](../Player_Errors.md).

In any other case, return `null`.

## Syntax

```js
const error = player.getError();
```

- **return value**:

`Object|null`: Returns the current error object if in the `"Error"` state or
`null` in any other case.
