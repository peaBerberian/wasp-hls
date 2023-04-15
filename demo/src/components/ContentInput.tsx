import * as React from "react";
import WaspHlsPlayer from "../../../src";

/**
 * Return input elements to load a new content.
 * @param {Object} props
 * @param {Object} props.player - The WaspHlsPlayer instance on which the
 * content will be loaded
 * @returns {Object}
 */
export default React.memo(function ContentInput({
  player,
}: {
  player: WaspHlsPlayer;
}): JSX.Element {
  const nameEltId = React.useId();
  const [url, setUrl] = React.useState<string>(
    "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8"
  );
  const [isContentListOpened, setIsContentListOpened] = React.useState(false);
  const loadContent = React.useCallback(() => {
    player.load(url);
  }, [player, url]);
  const onKeyDown = React.useCallback(
    (e: { key: string }) => {
      if (e.key === "Enter") {
        loadContent();
      }
    },
    [loadContent]
  );

  const onUrlChange = React.useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setUrl(e.target.value ?? "");
    },
    []
  );

  const onNewUrlChoice = React.useCallback((newUrl: string) => {
    setUrl(newUrl);
  }, []);

  const onDownArrowClick = React.useCallback(() => {
    setIsContentListOpened((prevVal) => {
      return !prevVal;
    });
  }, []);

  const downArrowClassName = isContentListOpened
    ? "content-list-arrow active"
    : "content-list-arrow";

  return (
    <>
      <div className="inputs-container">
        <label htmlFor={`url${nameEltId}`}>
          {"URL to HLS Multivariant (a.k.a. Master) Playlist:"}
        </label>
        <br />
        <span className="input-bar">
          <input
            onKeyDown={onKeyDown}
            className="input-url"
            type="text"
            name={`url${nameEltId}`}
            id={`url${nameEltId}`}
            onChange={onUrlChange}
            value={url}
          />
          <DownArrow
            onClick={onDownArrowClick}
            className={downArrowClassName}
          />
        </span>
        <button className="loading-button white-button" onClick={loadContent}>
          Load
        </button>
      </div>
      <ContentList
        currentUrl={url}
        onNewUrl={onNewUrlChoice}
        isOpened={isContentListOpened}
      />
    </>
  );
});

const DEFAULT_CONTENT_LIST = [
  /* eslint-disable max-len */
  {
    name: "Angel One (fmp4, multi-audio, multi-variants)",
    url: "https://storage.googleapis.com/shaka-demo-assets/angel-one-hls/hls.m3u8",
  },
  {
    name: "Big Buck Bunny (mpeg-ts, multi-variants, 10s segments)",
    url: "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8",
  },
  {
    name: "HLS Bitmovin (fmp4, multi-variants, 4s segments)",
    url: "https://bitdash-a.akamaihd.net/content/MI201109210084_1/m3u8s-fmp4/f08e80da-bf1d-4e3d-8899-f0f6155f6efa.m3u8",
  },
  /* eslint-enable max-len */
];

function ContentList({
  currentUrl,
  onNewUrl,
  isOpened,
}: {
  currentUrl: string;
  onNewUrl: (url: string) => void;
  isOpened: boolean;
}) {
  const contentChoice = React.useMemo(() => {
    return DEFAULT_CONTENT_LIST.map((content) => {
      function onClick() {
        onNewUrl(content.url);
      }
      const className =
        content.url === currentUrl
          ? "selected content-choice"
          : "content-choice";
      return (
        <div className={className} onClick={onClick} key={content.url}>
          {content.name}
        </div>
      );
    });
  }, [currentUrl]);
  const btnClassName = "btn-dropdown" + (isOpened ? " open" : "");
  return (
    <nav className="dropdown-wrapper">
      <div className={btnClassName}>
        <div className="dropdown-container">
          <div className="dropdown-inner">{contentChoice}</div>
        </div>
      </div>
    </nav>
  );
}

function DownArrow({
  onClick,
  className,
}: {
  onClick: () => void;
  className: string;
}): JSX.Element {
  /* eslint-disable max-len */
  return (
    <span className="down-arrow-wrapper">
      <svg
        className={className}
        onClick={onClick}
        viewBox="0 -0.5 21 21"
        version="1.1"
        xmlns="http://www.w3.org/2000/svg"
      >
        <g stroke="none" strokeWidth="1">
          <g transform="translate(-179.000000, -600.000000)">
            <g id="icons" transform="translate(56.000000, 160.000000)">
              <path d="M137.7,450 C137.7,450.552 137.2296,451 136.65,451 L134.55,451 L134.55,453 C134.55,453.552 134.0796,454 133.5,454 C132.9204,454 132.45,453.552 132.45,453 L132.45,451 L130.35,451 C129.7704,451 129.3,450.552 129.3,450 C129.3,449.448 129.7704,449 130.35,449 L132.45,449 L132.45,447 C132.45,446.448 132.9204,446 133.5,446 C134.0796,446 134.55,446.448 134.55,447 L134.55,449 L136.65,449 C137.2296,449 137.7,449.448 137.7,450 M133.5,458 C128.86845,458 125.1,454.411 125.1,450 C125.1,445.589 128.86845,442 133.5,442 C138.13155,442 141.9,445.589 141.9,450 C141.9,454.411 138.13155,458 133.5,458 M133.5,440 C127.70085,440 123,444.477 123,450 C123,455.523 127.70085,460 133.5,460 C139.29915,460 144,455.523 144,450 C144,444.477 139.29915,440 133.5,440"></path>
            </g>
          </g>
        </g>
      </svg>
    </span>
    /* eslint-enable max-len */
  );
}
