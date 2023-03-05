import WaspHlsPlayer, {
  PlayerState,
} from "./ts-main";

// TODO only debug mode?
/* eslint-disable */
(window as any).WaspHlsPlayer = WaspHlsPlayer;
/* eslint-enable */

export { PlayerState };
export default WaspHlsPlayer;
