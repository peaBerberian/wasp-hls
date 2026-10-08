import * as React from "react";
import type { AudioTrackInfo } from "../../../src/ts-main";

/**
 * @param {Object} props
 * @returns {Object}
 */
function AudioTrackSetting({
  audioTrack,
  audioTrackList,
  updateAudioTrack,
}: {
  audioTrack: AudioTrackInfo | undefined;
  audioTrackList: AudioTrackInfo[];
  updateAudioTrack: (t: AudioTrackInfo | undefined) => void;
}): React.JSX.Element | null {
  const onSelectChange = React.useCallback(
    (evt: React.SyntheticEvent<HTMLSelectElement>) => {
      if (audioTrackList.length < 2) {
        return;
      }
      const index = +(evt.target as HTMLSelectElement).value;
      updateAudioTrack(index === 0 ? undefined : audioTrackList[index - 1]);
    },
    [audioTrackList],
  );

  const selectedIndex =
    audioTrack === undefined
      ? 0
      : audioTrackList.findIndex((t) => t.id === audioTrack.id) + 1;

  const optionsEl = React.useMemo(() => {
    return [
      <option key="default" value={0}>
        {"default"}
      </option>,
      ...audioTrackList.map((t, index) => {
        return (
          <option key={t.id} value={index + 1}>
            {formatAudioTrack(t)}
          </option>
        );
      }),
    ];
  }, [audioTrack, audioTrackList]);

  if (audioTrackList.length === 0) {
    return null;
  }

  return (
    <div className="video-setting audio-track-setting">
      <span className="setting-name">{"Audio"}</span>
      <select
        disabled={audioTrackList.length < 2}
        aria-label="Update the current audio track"
        className="setting-value"
        onChange={onSelectChange}
        value={selectedIndex || 0}
      >
        {optionsEl}
      </select>
    </div>
  );
}

export default React.memo(AudioTrackSetting);

function formatAudioTrack(t: AudioTrackInfo): string {
  // Some crazy work-around because the main test stream I add did not quite
  // respect the idea of having NAME in a human-readable format
  if (t.name.startsWith("stream_")) {
    return t.language ?? t.name;
  }
  return t.name;
}
