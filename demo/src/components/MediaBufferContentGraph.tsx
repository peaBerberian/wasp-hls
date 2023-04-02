import * as React from "react";
import WaspHlsPlayer, { PlayerState } from "../../../src";

const { useEffect, useMemo, useRef, useState } = React;

const TIME_CHECK_INTERVAL = 200;
const CANVAS_WIDTH = 1000;
const CANVAS_HEIGHT = 1;
const COLOR_CURRENT_POSITION = "#008bd3";
const COLOR_BUFFER_FILLING = "#eee800";

/**
 * Clear the whole canvas.
 * @param {Object} canvasContext
 */
function clearCanvas(canvasContext: CanvasRenderingContext2D): void {
  canvasContext.clearRect(0, 0, CANVAS_WIDTH, CANVAS_HEIGHT);
}

/**
 * Represent the current position in the canvas.
 * @param {number|undefined} position - The current position
 * @param {number} minimumPosition - minimum possible position represented in
 * the canvas.
 * @param {number} maximumPosition - maximum possible position represented in
 * the canvas.
 * @param {Object} canvasCtx - The canvas' 2D context
 */
function paintCurrentPosition(
  position: number | undefined,
  minimumPosition: number,
  maximumPosition: number,
  canvasCtx: CanvasRenderingContext2D
): void {
  if (
    typeof position === "number" &&
    position >= minimumPosition &&
    position < maximumPosition
  ) {
    const lengthCanvas = maximumPosition - minimumPosition;
    canvasCtx.fillStyle = COLOR_CURRENT_POSITION;
    canvasCtx.fillRect(
      Math.ceil(((position - minimumPosition) / lengthCanvas) * CANVAS_WIDTH) -
        1,
      0,
      3,
      CANVAS_HEIGHT
    );
  }
}

interface ScaledBufferedRange {
  scaledStart: number;
  scaledEnd: number;
}

/**
 * Scale given `bufferedData` in terms of percentage between the minimum and
 * maximum position. Filter out ranges which are not part of it.
 * @param {Array.<Object>} bufferedData
 * @param {number} minimumPosition
 * @param {number} maximumPosition
 * @returns {Array.<Object>}
 */
function scaleRanges(
  bufferedData: TimeRanges,
  minimumPosition: number,
  maximumPosition: number,
  mediaOffset: number
): ScaledBufferedRange[] {
  const scaledRanges = [];
  const wholeDuration = maximumPosition - minimumPosition;
  for (let i = 0; i < bufferedData.length; i++) {
    const start = bufferedData.start(i) - mediaOffset;
    const end = bufferedData.end(i) - mediaOffset;
    if (end > minimumPosition && start < maximumPosition) {
      const startPoint = Math.max(start - minimumPosition, 0);
      const endPoint = Math.min(end - minimumPosition, maximumPosition);
      const scaledStart = startPoint / wholeDuration;
      const scaledEnd = endPoint / wholeDuration;
      scaledRanges.push({ scaledStart, scaledEnd });
    }
  }
  return scaledRanges;
}

/**
 * Display a graph representing what has been buffered according to the data
 * given.
 * Allow to seek on click, display the current time, and display a tooltip
 * describing the buffered data when hovering represented data.
 * @param {Object}
 */
export default function BufferContentGraph({
  videoElement,
  player,
}: {
  videoElement: HTMLMediaElement;
  player: WaspHlsPlayer;
}): JSX.Element {
  const [position, setPosition] = useState<number | undefined>(undefined);
  const [minimumPosition, setMinimumPosition] = useState<number | undefined>(
    undefined
  );
  const [maximumPosition, setMaximumPosition] = useState<number | undefined>(
    undefined
  );
  const [bufferedData, setBufferedData] = useState<TimeRanges | undefined>(
    undefined
  );

  const canvasEl = useRef<HTMLCanvasElement>(null);
  const usedMaximum = maximumPosition ?? 300;
  const usedMinimum = minimumPosition ?? 0;
  const duration = Math.max(usedMaximum - usedMinimum, 0);

  /**
   * Paint a given range in the canvas.
   * @param {Object} scaledRange - Buffered range information with
   * "scaled" time information to know where it fits in the canvas.
   * @param {Object} canvasCtx - The canvas' 2D context
   */
  const paintRange = React.useCallback(
    (
      scaledRange: ScaledBufferedRange,
      canvasCtx: CanvasRenderingContext2D
    ): void => {
      const startX = scaledRange.scaledStart * CANVAS_WIDTH;
      const endX = scaledRange.scaledEnd * CANVAS_WIDTH;
      canvasCtx.fillStyle = COLOR_BUFFER_FILLING;
      canvasCtx.fillRect(
        Math.ceil(startX),
        0,
        Math.ceil(endX - startX),
        CANVAS_HEIGHT
      );
    },
    []
  );

  const currentRangesScaled = useMemo<ScaledBufferedRange[] | null>(() => {
    if (bufferedData === undefined) {
      return null;
    }
    const mediaOffset = player.getMediaOffset() ?? 0;
    return scaleRanges(bufferedData, usedMinimum, usedMaximum, mediaOffset);
  }, [bufferedData, usedMinimum, usedMaximum]);

  useEffect(() => {
    if (canvasEl.current === null) {
      return;
    }
    const ctx = canvasEl.current.getContext("2d");
    if (ctx === null) {
      return;
    }
    canvasEl.current.width = CANVAS_WIDTH;
    canvasEl.current.height = CANVAS_HEIGHT;
    clearCanvas(ctx);

    if (
      currentRangesScaled === null ||
      usedMinimum === undefined ||
      usedMaximum === undefined ||
      usedMinimum >= usedMaximum
    ) {
      return;
    }
    for (let i = 0; i < currentRangesScaled.length; i++) {
      paintRange(currentRangesScaled[i], ctx);
    }
    paintCurrentPosition(position, usedMinimum, usedMaximum, ctx);
  }, [usedMinimum, usedMaximum, currentRangesScaled]);

  const getMousePositionInPercentage = React.useCallback(
    (event: React.MouseEvent) => {
      if (canvasEl.current === null) {
        return;
      }
      const rect = canvasEl.current.getBoundingClientRect();
      const point0 = rect.left;
      const clickPosPx = Math.max(event.clientX - point0, 0);
      const endPointPx = Math.max(rect.right - point0, 0);
      if (!endPointPx) {
        return 0;
      }
      return clickPosPx / endPointPx;
    },
    []
  );

  const getMousePosition = React.useCallback(
    (event: React.MouseEvent) => {
      const mousePercent = getMousePositionInPercentage(event);
      return mousePercent === undefined
        ? undefined
        : mousePercent * duration + usedMinimum;
    },
    [getMousePositionInPercentage, duration, usedMinimum]
  );

  const seek = React.useCallback(
    (wantedPos: number) => {
      player.seek(wantedPos);
    },
    [player]
  );

  const onCanvasClick = React.useCallback(
    (event: React.MouseEvent) => {
      const mousePosition = getMousePosition(event);
      if (mousePosition !== undefined) {
        seek(mousePosition);
      }
    },
    [getMousePosition, seek]
  );

  useEffect(() => {
    let positionRefreshIntervalId: number | undefined;
    if (player.getPlayerState() === PlayerState.Loaded) {
      onPositionUpdateInterval();
      positionRefreshIntervalId = setInterval(
        onPositionUpdateInterval,
        TIME_CHECK_INTERVAL
      );
    }
    player.addEventListener("playerStateChange", onPlayerStateChange);

    return () => {
      resetTimeInfo();
      player.removeEventListener("playerStateChange", onPlayerStateChange);
      clearPositionUpdateInterval();
    };

    function onPlayerStateChange(playerState: PlayerState): void {
      switch (playerState) {
        case PlayerState.Loading:
        case PlayerState.Stopped:
        case PlayerState.Error:
          resetTimeInfo();
          clearPositionUpdateInterval();
          break;
        case PlayerState.Loaded:
          clearPositionUpdateInterval();
          positionRefreshIntervalId = setInterval(
            onPositionUpdateInterval,
            TIME_CHECK_INTERVAL
          );
          break;
      }
    }

    function resetTimeInfo() {
      setPosition(undefined);
      setMinimumPosition(0);
      setMaximumPosition(Infinity);
      setBufferedData(undefined);
    }

    function onPositionUpdateInterval() {
      const pos = player.getPosition();
      setPosition(pos);
      const minPos = player.getMinimumPosition();
      setMinimumPosition(minPos ?? 0);
      const maxPos = player.getMaximumPosition();
      setMaximumPosition(maxPos ?? Infinity);
      setBufferedData(videoElement.buffered);
    }
    function clearPositionUpdateInterval() {
      if (positionRefreshIntervalId !== undefined) {
        clearInterval(positionRefreshIntervalId);
        positionRefreshIntervalId = undefined;
      }
    }
  }, [player]);

  return (
    <div className="container-buffer-graph">
      {bufferedData === undefined ? null : (
        <div className="canvas-buffer-graph-container">
          <canvas
            onClick={onCanvasClick}
            height={String(CANVAS_HEIGHT)}
            width={String(CANVAS_WIDTH)}
            className="canvas-buffer-graph"
            ref={canvasEl}
          />
        </div>
      )}
    </div>
  );
}
