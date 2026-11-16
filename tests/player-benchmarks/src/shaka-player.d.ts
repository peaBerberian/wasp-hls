/// <reference path="../node_modules/shaka-player/dist/shaka-player.compiled.d.ts" />

declare module "shaka-player" {
  const api: typeof shaka;
  export default api;
}
