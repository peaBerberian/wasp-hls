import * as React from "react";
import { toMinutes, toHours } from "../utils/time";

/**
 * Text with the following structure:
 *   CURRENT_POSITION / DURATION
 * @param {Object} props
 * @returns {Object}
 */
export default React.memo(function PositionInfos({
  position,
  duration,
}: {
  position: number;
  duration: number;
}) {
  const convertTime = duration >= 60*60 ? toHours : toMinutes;
  if (isNaN(position) || isNaN(duration) || !isFinite(position) || !isFinite(duration)) {
    return null;
  }
  return (
    <div className="position-info">
      <span className="current-position">
        { convertTime(position) }
      </span>
      <span className="separator">
        {" / "}
      </span>
      <span className="duration">
        { convertTime(duration) }
      </span>
    </div>
  );
});
