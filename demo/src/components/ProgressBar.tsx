import * as React from "react";

/**
 * Horizontal (left-to-right) progress bar component which:
 *
 *   - represents the current position and the buffer relatively to the
 *     minimum / maximum position.
 *
 *   - triggers a seek function with the clicked position on click
 * @param {Object} props
 * @returns {Object}
 */
export default React.memo(function ProgressBar({
  seek,
  position,
  bufferGap,
  minimumPosition,
  maximumPosition,
}: {
  seek: (pos: number) => void;
  position: number;
  bufferGap: number;
  minimumPosition: number;
  maximumPosition: number;
}): JSX.Element {
  const wrapperRef = React.useRef<HTMLDivElement>(null);

  const duration = Math.max(maximumPosition - minimumPosition, 0);

  const getMousePosition = React.useCallback(
    (event: { clientX: number }) => {
      if (wrapperRef.current === null) {
        return;
      }
      const rect = wrapperRef.current.getBoundingClientRect();
      const point0 = rect.left;
      const clickPosPx = Math.max(event.clientX - point0, 0);
      const endPointPx = Math.max(rect.right - point0, 0);
      if (!endPointPx) {
        return 0;
      }
      return (clickPosPx / endPointPx) * duration + minimumPosition;
    },
    [minimumPosition, maximumPosition],
  );

  const relativePosition = Math.max(position - minimumPosition, 0);
  const percentBuffered =
    Math.min((bufferGap + relativePosition) / duration, 1) * 100;

  const percentPosition = Math.min(relativePosition / duration, 1) * 100;

  return (
    <div
      className="progress-bar-wrapper"
      ref={wrapperRef}
      onClick={(event) => {
        const pos = getMousePosition(event);
        if (pos !== undefined) {
          seek(pos);
        }
      }}
    >
      <div
        className="progress-bar-current"
        style={{
          width: String(percentPosition) + "%",
        }}
      />
      <div
        className="progress-bar-buffered"
        style={{
          width: String(percentBuffered) + "%",
        }}
      />
    </div>
  );
});
