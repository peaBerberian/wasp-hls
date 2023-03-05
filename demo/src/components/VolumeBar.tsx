import * as React from "react";

/**
 * Horizontal (left-to-right) volume indication component which:
 *
 *   - represents the current volume relatively to the max and min.
 *
 *   - triggers a onVolumeChange function with the clicked volume percentage
 *      on click
 * @param {Object} props
 * @returns {Object}
 */
export default React.memo(function VolumeBar({
  volume,
  onVolumeChange,
}: {
  volume: number;
  onVolumeChange: (volume: number) => void;
}): JSX.Element {
  const element = React.useRef<HTMLDivElement>(null);

  const getMouseVolume = React.useCallback((event: { clientX : number }) => {
    if (element.current === null) {
      return;
    }
    const rect = element.current.getBoundingClientRect();
    const point0 = rect.left;
    const clickPosPx = Math.max(event.clientX - point0, 0);
    const endPointPx = Math.max(rect.right - point0, 0);
    if (!endPointPx) {
      return 0;
    }
    return Math.min(clickPosPx / endPointPx, 1);
  }, []);

  return (
    <div
      className="volume-bar-wrapper"
      ref={element}
      onClick={(evt) => {
        const newVolume = getMouseVolume(evt);
        if (newVolume !== undefined) {
          onVolumeChange(newVolume);
        }
      }}>
      <div
        className="volume-bar-current"
        style={{ width: String(volume * 100) + "%" }}
      />
    </div>
  );
});
