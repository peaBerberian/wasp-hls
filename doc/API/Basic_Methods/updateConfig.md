# `updateConfig` method

## Description

Update some or all of the [Configuration Object](../Configuration_Object.md)
properties.

That method accepts an object which can be a redefinition of the whole
configuration object or only a subset by only declaring some of its properties.

For example, to only set the `segmentMaxRetry` configuration to `3`, you can
write:

```js
player.updateConfig({
  segmentMaxRetry: 3,
});
```

Note that to ignore some properties, you can also set them to `undefined`. As
such, the following code will also **JUST** update the `segmentMaxRetry`
configuration to `3`.

```js
player.updateConfig({
  bufferGoal: undefined,
  segmentMaxRetry: 3,
});
```

This is possible because no properties of the Configuration Object has
`undefined` as a valid value.

Also note that because most of that configuration's values are actually relied
on by code running in a worker, there's a necessary delay before it actually
becomes considered (it doesn't happen synchronously).

## Syntax

```js
player.updateConfig(configUpdate);
```

- **argument**:

  1. _configUpdate_ `Object`: Updates to combine with the previous configuration
     object. All properties unset or set to `undefined` will be let as they
     were before.
