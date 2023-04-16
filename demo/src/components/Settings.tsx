import * as React from "react";
import WaspHlsPlayer from "../../../src";
import { WaspHlsPlayerConfig } from "../../../src/ts-common/types";

/**
 * @param {Object} props
 * @param {Object} props.player - The WaspHlsPlayer instance on which the
 * content will be loaded
 * @returns {Object}
 */
export default React.memo(function ContentBar({
  player,
}: {
  player: WaspHlsPlayer;
}): JSX.Element {
  return (
    <div className="settings">
      <SettingsTitle />
      <NumberSetting
        configKey="bufferGoal"
        player={player}
        description={
          <>The amount of buffer built ahead of the current position.</>
        }
      />
      <NumberSetting
        configKey="segmentMaxRetry"
        player={player}
        description={
          <>
            Maximum amount of retry if a segment request fails on a retry-able
            error (HTTP 5xx, 404, timeouts...).
            <br />
            An exponential backoff delay mechanism exists to avoid overloading
            the server.
            <br />
            To set to `-1` for no retry limitation.
          </>
        }
      />
      <NumberSetting
        configKey="segmentBackoffBase"
        player={player}
        description={<>(see configuration documentation)</>}
      />
      <NumberSetting
        configKey="segmentBackoffMax"
        player={player}
        description={<>(see configuration documentation)</>}
      />
      <NumberSetting
        configKey="multiVariantPlaylistMaxRetry"
        player={player}
        description={
          <>
            Maximum amount of retry if the Multivariant Playlist request fails
            on a retry-able error (HTTP 5xx, 404, timeouts...).
            <br />
            An exponential backoff delay mechanism exists to avoid overloading
            the server.
            <br />
            To set to `-1` for no retry limitation.
          </>
        }
      />
      <NumberSetting
        configKey="multiVariantPlaylistBackoffBase"
        player={player}
        description={<>(see configuration documentation)</>}
      />
      <NumberSetting
        configKey="multiVariantPlaylistBackoffMax"
        player={player}
        description={<>(see configuration documentation)</>}
      />
      <NumberSetting
        configKey="mediaPlaylistMaxRetry"
        player={player}
        description={
          <>
            Maximum amount of retry if a Media Playlist request fails on a
            retry-able error (HTTP 5xx, 404, timeouts...).
            <br />
            An exponential backoff delay mechanism exists to avoid overloading
            the server.
            <br />
            To set to `-1` for no retry limitation.
          </>
        }
      />
      <NumberSetting
        configKey="mediaPlaylistBackoffBase"
        player={player}
        description={<>(see configuration documentation)</>}
      />
      <NumberSetting
        configKey="mediaPlaylistBackoffMax"
        player={player}
        description={<>(see configuration documentation)</>}
      />
    </div>
  );
});

function NumberSetting({
  configKey,
  player,
  description,
}: {
  configKey: keyof WaspHlsPlayerConfig;
  player: WaspHlsPlayer;
  description: JSX.Element;
}): JSX.Element {
  const [canApply, setCanApply] = React.useState<boolean>(false);
  const inputRef = React.useRef<HTMLInputElement>(null);
  const onApply = React.useCallback(() => {
    if (inputRef.current === null) {
      return;
    }
    const input = parseFloat(inputRef.current.value);
    if (isNaN(input)) {
      return;
    }
    player.updateConfig({ [configKey]: input });
    setCanApply(input !== player.getConfig()[configKey]);
  }, []);

  const onInputChange = React.useCallback(() => {
    if (inputRef.current === null) {
      setCanApply(false);
      return;
    }
    const input = parseFloat(inputRef.current.value);
    if (isNaN(input)) {
      setCanApply(false);
      return;
    }
    setCanApply(input !== player.getConfig()[configKey]);
  }, []);

  const config = player.getConfig();
  return (
    <div className="setting-elt">
      <div className="setting-elt-title">{configKey}</div>
      <input
        type="number"
        ref={inputRef}
        defaultValue={config[configKey]}
        onChange={onInputChange}
      />
      <button
        disabled={!canApply}
        className={"apply-button white-button" + (canApply ? "" : " disabled")}
        onClick={onApply}
      >
        Apply
      </button>
      <div className="setting-elt-desc">{description}</div>
    </div>
  );
}

function SettingsTitle(): JSX.Element {
  return (
    <div className="settings-title">
      <div className="settings-title-text">Config</div>
      <div className="settings-title-desc">
        Some selected values for the WaspHlsPlayer's{" "}
        <a href="./doc/API/Configuration_Object.html">configuration object</a>.
        <br />
        Apply even to an already-loaded content.
      </div>
    </div>
  );
}
