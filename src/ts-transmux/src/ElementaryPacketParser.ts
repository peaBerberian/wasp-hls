import {
  ADTS_STREAM_TYPE,
  H264_STREAM_TYPE,
  METADATA_STREAM_TYPE,
} from "./constants";
import {
  ParsedPacket,
  PesPacket,
  ProgramMapTable,
} from "./TransportPacketParser";

export interface MetadataElementaryPacket {
  type: "metadata";
  tracks: MetadataElementaryPacketTrack[];
}

export type MediaElementaryPacket =
  | AudioElementaryPacket
  | VideoElementaryPacket
  | TimedMetadataElementaryPacket;

export interface AudioElementaryPacket extends ElementaryPacketBase {
  type: "audio";
}

export interface VideoElementaryPacket extends ElementaryPacketBase {
  type: "video";
}

export interface TimedMetadataElementaryPacket extends ElementaryPacketBase {
  type: "timed-metadata";
}

interface ElementaryPacketBase {
  trackId: number;
  data: Uint8Array;
  packetLength?: number;
  dataAlignmentIndicator?: boolean;
  pts?: number;
  dts?: number;
}

export interface MetadataElementaryPacketTrack {
  timelineStartInfo: {
    baseMediaDecodeTime: number;
  };
  id: number;
  codec: string;
  type: ElementaryPacketMediaType;
}

export type ElementaryPacketMediaType = "video" | "audio" | "timed-metadata";

export type ElementaryPacket = MetadataElementaryPacket | MediaElementaryPacket;

interface ElementaryTrackInfo {
  data: PesPacket[];
  size: number;
}

export default class ElementaryPacketParser {
  private _segmentHadPmt: boolean;
  private _audio: ElementaryTrackInfo;
  private _video: ElementaryTrackInfo;
  private _timedMetadata: ElementaryTrackInfo;
  private _programMapTable: ProgramMapTable | null;

  constructor() {
    this._segmentHadPmt = false;
    // PES packet fragments
    this._video = {
      data: [],
      size: 0,
    };
    this._audio = {
      data: [],
      size: 0,
    };
    this._timedMetadata = {
      data: [],
      size: 0,
    };
    this._programMapTable = null;
  }

  /**
   * Identifies M2TS packet types and parses PES packets using metadata
   * parsed from the PMT
   **/
  public readNextPacket(data: ParsedPacket): ElementaryPacket | null {
    switch (data.type) {
      case "pat":
        // we have to wait for the PMT to arrive as well before we
        // have any meaningful metadata
        break;
      case "pes": {
        let stream: { data: PesPacket[]; size: number };
        let streamType: ElementaryPacketMediaType;
        switch (data.streamType) {
          case H264_STREAM_TYPE:
            stream = this._video;
            streamType = "video";
            break;
          case ADTS_STREAM_TYPE:
            stream = this._audio;
            streamType = "audio";
            break;
          case METADATA_STREAM_TYPE:
            stream = this._timedMetadata;
            streamType = "timed-metadata";
            break;
          default:
            // ignore unknown stream types
            return null;
        }

        let res: ElementaryPacket | null = null;
        // if a new packet is starting, we can flush the completed
        // packet
        if (data.payloadUnitStartIndicator) {
          res = this._flushStream(stream, streamType, true);
        }

        // buffer this fragment until we are sure we've received the
        // complete payload
        stream.data.push(data);
        stream.size += data.data.byteLength;
        return res;
      }
      case "pmt": {
        const packet: MetadataElementaryPacket = {
          type: "metadata",
          tracks: [],
        };

        const programMapTable = data.programMapTable;
        this._programMapTable = programMapTable;

        // translate audio and video streams to tracks
        if (programMapTable.video !== null) {
          packet.tracks.push({
            timelineStartInfo: {
              baseMediaDecodeTime: 0,
            },
            id: +programMapTable.video,
            codec: "avc",
            type: "video",
          });
        }
        if (programMapTable.audio !== null) {
          packet.tracks.push({
            timelineStartInfo: {
              baseMediaDecodeTime: 0,
            },
            id: +programMapTable.audio,
            codec: "adts",
            type: "audio",
          });
        }

        this._segmentHadPmt = true;

        return packet;
      }
    }
    return null;
  }

  public reset() {
    this._video.size = 0;
    this._video.data.length = 0;
    this._audio.size = 0;
    this._audio.data.length = 0;
  }

  private _parsePes(payload: Uint8Array, pes: MediaElementaryPacket): void {
    const startPrefix = (payload[0] << 16) | (payload[1] << 8) | payload[2];
    // In certain live streams, the start of a TS fragment has ts packets
    // that are frame data that is continuing from the previous fragment. This
    // is to check that the pes data is the start of a new pes payload
    if (startPrefix !== 1) {
      return;
    }
    // get the packet length, this will be 0 for video
    pes.packetLength = 6 + ((payload[4] << 8) | payload[5]);

    // find out if this packets starts a new keyframe
    pes.dataAlignmentIndicator = (payload[6] & 0x04) !== 0;
    // PES packets may be annotated with a PTS value, or a PTS value
    // and a DTS value. Determine what combination of values is
    // available to work with.
    const ptsDtsFlags = payload[7];

    // PTS and DTS are normally stored as a 33-bit number.  Javascript
    // performs all bitwise operations on 32-bit integers but javascript
    // supports a much greater range (52-bits) of integer using standard
    // mathematical operations.
    // We construct a 31-bit value using bitwise operators over the 31
    // most significant bits and then multiply by 4 (equal to a left-shift
    // of 2) before we add the final 2 least significant bits of the
    // timestamp (equal to an OR.)
    if (ptsDtsFlags & 0xc0) {
      // the PTS and DTS are not written out directly. For information
      // on how they are encoded, see
      // http://dvd.sourceforge.net/dvdinfo/pes-hdr.html
      pes.pts =
        ((payload[9] & 0x0e) << 27) |
        ((payload[10] & 0xff) << 20) |
        ((payload[11] & 0xfe) << 12) |
        ((payload[12] & 0xff) << 5) |
        ((payload[13] & 0xfe) >>> 3);
      pes.pts *= 4; // Left shift by 2
      pes.pts += (payload[13] & 0x06) >>> 1; // OR by the two LSBs
      pes.dts = pes.pts;
      if (ptsDtsFlags & 0x40) {
        pes.dts =
          ((payload[14] & 0x0e) << 27) |
          ((payload[15] & 0xff) << 20) |
          ((payload[16] & 0xfe) << 12) |
          ((payload[17] & 0xff) << 5) |
          ((payload[18] & 0xfe) >>> 3);
        pes.dts *= 4; // Left shift by 2
        pes.dts += (payload[18] & 0x06) >>> 1; // OR by the two LSBs
      }
    }
    // the data section starts immediately after the PES header.
    // pes_header_data_length specifies the number of header bytes
    // that follow the last byte of the field.
    pes.data = payload.subarray(9 + payload[8]);
  }
  /**
   * Pass completely parsed PES packets.
   **/
  private _flushStream(
    stream: ElementaryTrackInfo,
    type: "video" | "audio" | "timed-metadata",
    forceFlush?: boolean
  ): ElementaryPacket | null {
    let offset = 0;
    let packetFlushable = false;

    // do nothing if there is not enough buffered data for a complete
    // PES header
    if (stream.data.length === 0 || stream.size < 9) {
      return null;
    }
    const packet: MediaElementaryPacket = {
      type,
      data: new Uint8Array(),
      trackId: stream.data[0].pid,
    };

    // reassemble the packet
    const packetData = new Uint8Array(stream.size);
    stream.data.forEach((fragment) => {
      packetData.set(fragment.data, offset);
      offset += fragment.data.byteLength;
    });

    // parse assembled packet's PES header
    this._parsePes(packetData, packet);

    // non-video PES packets MUST have a non-zero PES_packet_length
    // check that there is enough stream data to fill the packet
    packetFlushable =
      type === "video" ||
      (packet.packetLength !== undefined && packet.packetLength <= stream.size);

    // flush pending packets if the conditions are right
    if (forceFlush === true || packetFlushable) {
      stream.size = 0;
      stream.data.length = 0;
    }

    // only emit packets that are complete. this is to avoid assembling
    // incomplete PES packets due to poor segmentation
    if (packetFlushable) {
      return packet;
    }
    return null;
  }

  /**
   * Flush any remaining input. Video PES packets may be of variable
   * length. Normally, the start of a new video packet can trigger the
   * finalization of the previous packet. That is not possible if no
   * more video is forthcoming, however. In that case, some other
   * mechanism (like the end of the file) has to be employed. When it is
   * clear that no additional data is forthcoming, calling this method
   * will flush the buffered packets.
   */
  public _flushStreams(): ElementaryPacket[] {
    // !!THIS ORDER IS IMPORTANT!!
    // video first then audio
    const res: ElementaryPacket[] = [];
    const videoRes = this._flushStream(this._video, "video");
    if (videoRes !== null) {
      res.push(videoRes);
    }
    const audioRes = this._flushStream(this._audio, "audio");
    if (audioRes !== null) {
      res.push(audioRes);
    }
    const timedRes = this._flushStream(this._timedMetadata, "timed-metadata");
    if (timedRes !== null) {
      res.push(timedRes);
    }
    return res;
  }

  public flush(): ElementaryPacket[] {
    let pmtPacket: MetadataElementaryPacket | null = null;
    // if on flush we haven't had a pmt emitted and we have a
    // pmt to emit. emit the pmt so that we trigger a trackinfo downstream.
    if (!this._segmentHadPmt && this._programMapTable !== null) {
      const programMapTable = this._programMapTable;
      const pmt: MetadataElementaryPacket = {
        type: "metadata",
        tracks: [],
      };
      // translate audio and video streams to tracks
      if (programMapTable.video !== null) {
        pmt.tracks.push({
          timelineStartInfo: {
            baseMediaDecodeTime: 0,
          },
          id: +programMapTable.video,
          codec: "avc",
          type: "video",
        });
      }

      if (programMapTable.audio !== null) {
        pmt.tracks.push({
          timelineStartInfo: {
            baseMediaDecodeTime: 0,
          },
          id: +programMapTable.audio,
          codec: "adts",
          type: "audio",
        });
      }

      pmtPacket = pmt;
    }

    this._segmentHadPmt = false;
    return pmtPacket !== null
      ? [pmtPacket, ...this._flushStreams()]
      : this._flushStreams();
  }
}
