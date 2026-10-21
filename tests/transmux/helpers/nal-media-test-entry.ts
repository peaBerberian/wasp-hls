export { default as Finder } from "../../../src/ts-transmux/src/NalUnitFinder.ts";
export { default as Transmuxer } from "../../../src/ts-transmux/index.ts";
import TransportStreamSplitter from "../../../src/ts-transmux/src/TransportStreamSplitter.ts";
import TransportPacketParser from "../../../src/ts-transmux/src/TransportPacketParser.ts";
import ElementaryPacketParser from "../../../src/ts-transmux/src/ElementaryPacketParser.ts";

/** Keep the actual PES boundaries when testing the scanner used by playback. */
export function videoPes(input: Uint8Array): Uint8Array[] {
  const splitter = new TransportStreamSplitter();
  const packets = new TransportPacketParser();
  const elementary = new ElementaryPacketParser();
  const video: Uint8Array[] = [];
  splitter.feed(input);
  while (true) {
    const [packet, ended] = splitter.readNextPacket();
    if (packet !== null)
      for (const parsed of packets.parse(packet)) {
        const pes = elementary.readNextPacket(parsed);
        if (pes?.type === "video") video.push(pes.data);
      }
    if (ended) break;
  }
  for (const pes of elementary.flush())
    if (pes.type === "video") video.push(pes.data);
  return video;
}
