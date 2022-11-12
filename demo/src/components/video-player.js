import { createContainerElt } from "../dom-utils";

export default function VideoPlayerComponent(videoElt) {
  return createContainerElt("div", {
    className: "video-container",
  }, [videoElt]);
}
