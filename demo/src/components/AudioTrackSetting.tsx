import * as React from "react";
import { AudioTrackInfo } from "../../../src/ts-main";

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
  isAuto: boolean;
  updateAudioTrack: (t: AudioTrackInfo) => void;
}): JSX.Element | null {
  const onSelectChange = React.useCallback(
    (evt: React.SyntheticEvent<HTMLSelectElement>) => {
      if (audioTrackList.length <= 1) {
        return;
      }
      const index = +(evt.target as HTMLSelectElement).value;
      const selected = audioTrackList[index];
      updateAudioTrack(selected);
    },
    [audioTrackList]
  );

  const selectedIndex =
    audioTrackList.length <= 1
      ? 0
      : audioTrackList.findIndex((t) => t.id === audioTrack?.id);

  const optionsEl = React.useMemo(() => {
    return audioTrackList.map((t, index) => {
      return (
        <option key={t.id} value={index}>
          {formatAudioTrack(t)}
        </option>
      );
    });
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
