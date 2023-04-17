import * as React from "react";
import WaspHlsPlayer from "../../../src";

/**
 * @param {Object} props
 * @param {Object} props.player - The WaspHlsPlayer instance on which the
 * content will be loaded
 * @returns {Object}
 */
export default React.memo(function ContentBar({
  player,
  onSettingsClick,
  isSettingsOpened,
}: {
  player: WaspHlsPlayer;
  onSettingsClick: () => void;
  isSettingsOpened: boolean;
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
            isOpened={isContentListOpened}
          />
          <SettingsButton
            onClick={onSettingsClick}
            isOpened={isSettingsOpened}
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
  isOpened,
}: {
  onClick: () => void;
  isOpened: boolean;
}): JSX.Element {
  const className = isOpened
    ? "content-list-arrow active"
    : "content-list-arrow";
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

function SettingsButton({
  onClick,
  isOpened,
}: {
  onClick: () => void;
  isOpened: boolean;
}): JSX.Element {
  const className = isOpened ? "settings-button active" : "settings-button";
  /* eslint-disable max-len */
  return (
    <span className="settings-btn-wrapper">
      <svg className={className} onClick={onClick} viewBox="0 -0.5 21 21">
        <g stroke="none" strokeWidth="1">
          <g transform="translate(-419.000000, -320.000000)">
            <g id="icons" transform="translate(56.000000, 160.000000)">
              <path d="M374.55,170 C374.55,170.552 374.0796,171 373.5,171 C372.9204,171 372.45,170.552 372.45,170 C372.45,169.448 372.9204,169 373.5,169 C374.0796,169 374.55,169.448 374.55,170 M378.561,171.358 C378.09585,173.027 376.67835,174.377 374.9259,174.82 C370.9359,175.828 367.3806,172.442 368.439,168.642 C368.90415,166.973 370.32165,165.623 372.0741,165.18 C376.0641,164.172 379.6194,167.558 378.561,171.358 M382.95,169 L381.2112,169 C380.95815,169 380.6106,168.984 380.6127,168.743 C380.61795,167.854 380.3124,166.59 379.6383,165.898 C379.4661,165.721 379.5165,165.559 379.695,165.389 L380.92455,164.281 C381.3351,163.89 381.3351,163.288 380.92455,162.898 C380.51505,162.507 379.84935,162.523 379.43985,162.913 L378.2103,164.092 C378.0318,164.262 377.75565,164.283 377.5446,164.151 C376.7781,163.669 375.91185,163.322 374.9805,163.141 C374.7327,163.092 374.55,162.897 374.55,162.656 L374.55,161 C374.55,160.448 374.0796,160 373.5,160 C372.9204,160 372.45,160.448 372.45,161 L372.45,162.656 C372.45,162.897 372.2673,163.094 372.0195,163.143 C371.08815,163.324 370.2219,163.672 369.4554,164.154 C369.24435,164.287 368.9682,164.27 368.7897,164.1 L367.56015,162.929 C367.15065,162.538 366.48495,162.538 366.07545,162.929 C365.6649,163.319 365.6649,163.953 366.07545,164.343 L367.305,165.514 C367.4835,165.684 367.5108,165.953 367.3617,166.148 C366.843,166.831 366.5112,167.562 366.3621,168.84 C366.33375,169.079 366.04185,169 365.7888,169 L364.05,169 C363.4704,169 363,169.448 363,170 C363,170.552 363.4704,171 364.05,171 L365.7888,171 C366.04185,171 366.34845,171.088 366.39885,171.323 C366.5889,172.21 366.85665,172.872 367.3617,173.602 C367.50135,173.803 367.4835,174.191 367.305,174.361 L366.07545,175.594 C365.6649,175.985 365.6649,176.649 366.07545,177.04 C366.48495,177.43 367.15065,177.446 367.56015,177.055 L368.7897,175.892 C368.9682,175.722 369.24435,175.709 369.4554,175.842 C370.2219,176.323 371.08815,176.674 372.0195,176.855 C372.2673,176.904 372.45,177.103 372.45,177.344 L372.45,179 C372.45,179.552 372.9204,180 373.5,180 C374.0796,180 374.55,179.552 374.55,179 L374.55,177.344 C374.55,177.103 374.7327,176.906 374.9805,176.857 C375.91185,176.676 376.7781,176.327 377.5446,175.846 C377.75565,175.713 378.0318,175.73 378.2103,175.9 L379.43985,177.071 C379.84935,177.462 380.51505,177.462 380.92455,177.071 C381.3351,176.681 381.3351,176.047 380.92455,175.657 L379.695,174.486 C379.5165,174.316 379.49865,174.053 379.6383,173.852 C380.14335,173.122 380.4174,172.714 380.69985,171.91 C380.7807,171.682 380.95815,171 381.2112,171 L382.95,171 C383.5296,171 384,170.552 384,170 C384,169.448 383.5296,169 382.95,169"></path>
            </g>
          </g>
        </g>
      </svg>
    </span>
  );
  /* eslint-enable max-len */
}
