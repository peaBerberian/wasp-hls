import * as Bindings from "./bindings";
import MessageReceiver from "./MessageReceiver";

// Declare ts-bindings as globals, capitalized and prefixed with "js".
// We don't care, we are the only one to control the Worker.
Object.keys(Bindings).forEach((binding) => {
  /* eslint-disable */
  const global = self as any;
  if (binding === "doFetch") {
    global.jsFetch = Bindings[binding];
  }
  const capitalized = binding.charAt(0).toUpperCase() + binding.slice(1);
  global[`js${capitalized}`] = Bindings[binding as keyof typeof Bindings];
  /* eslint-enable */
});

MessageReceiver();
