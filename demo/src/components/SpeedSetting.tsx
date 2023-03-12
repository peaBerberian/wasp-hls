import * as React from "react";

const AVAILABLE_RATES = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.5, 3];
const OPTIONS = AVAILABLE_RATES.map(String);

/**
 * Input for playback rate selection on a media content.
 * @param {Object} props
 * @returns {Object}
 */
function SpeedSetting({
  speed,
  updateSpeed,
}: {
  speed: number;
  updateSpeed: (newSpeed: number) => void;
}): JSX.Element {

  const onSelectChange = React.useCallback((
    evt: React.SyntheticEvent<HTMLSelectElement>
  ) => {
    const index = +(evt.target as HTMLSelectElement).value;
    if (index > -1) {
      const rate = AVAILABLE_RATES[index];
      if (rate !== undefined) {
        updateSpeed(rate);
      } else {
        /* eslint-disable-next-line no-console */
        console.error("Error: playback rate not found");
      }
    }
  }, []);

  const selectedIndex = React.useMemo(() => {
    return OPTIONS.findIndex(o => o === String(speed));
  }, [speed]);

  const optionsEl = React.useMemo(() => {
    return OPTIONS.map((optName, index) => {
      return <option key={index} value={index}>
        {optName}
      </option>;
    });
  }, []);

  return (
    <div className="video-setting speed-setting">
      <span className="setting-name" >
        {"Speed"}
      </span>
      <select
        aria-label="Update the current playback speed"
        className="setting-value"
        onChange={onSelectChange}
        value={selectedIndex || 0}
      >
        {optionsEl}
      </select>
    </div>
  );
}

export default React.memo(SpeedSetting);
