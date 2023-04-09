/* eslint-disable max-classes-per-file */
import EventEmitter from "../../ts-common/EventEmitter";
import CaptionStream, { Cea608Stream, Cea708Stream } from "./caption-stream";
import MetadataStream from "./metadata-stream";
import TimestampRolloverStream from "./timestamp-rollover-stream";

const PAT_PID = 0x0000;
const H264_STREAM_TYPE = 0x1b;
const ADTS_STREAM_TYPE = 0x0f;
const METADATA_STREAM_TYPE = 0x15;
const MP2T_PACKET_LENGTH = 188;
const SYNC_BYTE = 0x47;

export interface TransportPacketStreamEvents {
  data: Uint8Array;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * Splits an incoming stream of binary data into single MPEG-2 Transport
 * Stream packets.
 * @class TransportPacketStream
 */
export class TransportPacketParser {
  private _input: Uint8Array | null;
  private _mp2tPacketBuffer: Uint8Array;
  private _bytesInBuffer: number;
  private _startIndex: number;
  private _endIndex: number;

  constructor() {
    this._input = null;
    this._mp2tPacketBuffer = new Uint8Array(MP2T_PACKET_LENGTH);
    this._bytesInBuffer = 0;
    this._startIndex = 0;
    this._endIndex = MP2T_PACKET_LENGTH;
  }

  public feed(bytes: Uint8Array): void {
    // If there are bytes remaining from the last segment, prepend them to the
    // bytes that were pushed in
    if (this._bytesInBuffer > 0 || this._input !== null) {
      const prevInputLength = this._input !== null
        ? this._input.byteLength - this._startIndex
        : 0;
      const newInput = new Uint8Array(
        bytes.byteLength + this._bytesInBuffer + prevInputLength
      );
      newInput.set(this._mp2tPacketBuffer.subarray(0, this._bytesInBuffer));
      newInput.set(bytes, this._bytesInBuffer);
      if (this._input !== null && prevInputLength > 0) {
        newInput.set(
          this._input.subarray(this._startIndex),
          this._bytesInBuffer + bytes.byteLength
        )
      }
      this._input = newInput;
    } else {
      this._input = bytes;
    }

    this._startIndex = 0;
    this._endIndex = MP2T_PACKET_LENGTH;
  }

  public readNextPacket(): Uint8Array | null {
    if (this._input === null) {
      return null;
    }

    const everything = this._input;

    // While we have enough data for a packet
    while (this._endIndex < everything.byteLength) {
      // Look for a pair of start and end sync bytes in the data..
      if (
        everything[this._startIndex] === SYNC_BYTE &&
        everything[this._endIndex] === SYNC_BYTE
      ) {
        // We found a packet so emit it and jump one whole packet forward in
        // the stream
        const data = everything.subarray(this._startIndex, this._endIndex);
        this._startIndex += MP2T_PACKET_LENGTH;
        this._endIndex += MP2T_PACKET_LENGTH;
        return data;
      }
      // If we get here, we have somehow become de-synchronized and we need to step
      // forward one byte at a time until we find a pair of sync bytes that denote
      // a packet
      this._startIndex++;
      this._endIndex++;
    }

    // If there was some data left over at the end of the segment that couldn't
    // possibly be a whole packet, keep it because it might be the start of a packet
    // that continues in the next segment
    if (this._startIndex < everything.byteLength) {
      this._mp2tPacketBuffer.set(everything.subarray(this._startIndex), 0);
      this._bytesInBuffer = everything.byteLength - this._startIndex;
    }
    // If the buffer contains a whole packet when we are being flushed, emit it
    // and empty the buffer. Otherwise hold onto the data because it may be
    // important for decoding the next segment
    if (
      this._bytesInBuffer === MP2T_PACKET_LENGTH &&
      this._mp2tPacketBuffer[0] === SYNC_BYTE
    ) {
      this._bytesInBuffer = 0;
      this._input = null;
      return this._mp2tPacketBuffer;
    }
    return null;
  }
}

/**
 * Splits an incoming stream of binary data into single MPEG-2 Transport
 * Stream packets.
 * @class TransportPacketStream
 */
class TransportPacketStream extends EventEmitter<TransportPacketStreamEvents> {
  private _buffer: Uint8Array;
  private _bytesInBuffer: number;

  constructor() {
    super();
    this._buffer = new Uint8Array(MP2T_PACKET_LENGTH);
    this._bytesInBuffer = 0;
  }

  public push(bytes: Uint8Array): void {
    let startIndex = 0;
    let endIndex = MP2T_PACKET_LENGTH;
    let everything: Uint8Array;

    // If there are bytes remaining from the last segment, prepend them to the
    // bytes that were pushed in
    if (this._bytesInBuffer > 0) {
      everything = new Uint8Array(bytes.byteLength + this._bytesInBuffer);
      everything.set(this._buffer.subarray(0, this._bytesInBuffer));
      everything.set(bytes, this._bytesInBuffer);
      this._bytesInBuffer = 0;
    } else {
      everything = bytes;
    }

    // While we have enough data for a packet
    while (endIndex < everything.byteLength) {
      // Look for a pair of start and end sync bytes in the data..
      if (
        everything[startIndex] === SYNC_BYTE &&
        everything[endIndex] === SYNC_BYTE
      ) {
        // We found a packet so emit it and jump one whole packet forward in
        // the stream
        this.trigger("data", everything.subarray(startIndex, endIndex));
        startIndex += MP2T_PACKET_LENGTH;
        endIndex += MP2T_PACKET_LENGTH;
        continue;
      }
      // If we get here, we have somehow become de-synchronized and we need to step
      // forward one byte at a time until we find a pair of sync bytes that denote
      // a packet
      startIndex++;
      endIndex++;
    }

    // If there was some data left over at the end of the segment that couldn't
    // possibly be a whole packet, keep it because it might be the start of a packet
    // that continues in the next segment
    if (startIndex < everything.byteLength) {
      this._buffer.set(everything.subarray(startIndex), 0);
      this._bytesInBuffer = everything.byteLength - startIndex;
    }
  }

  /**
   * Passes identified M2TS packets to the TransportParseStream to be parsed
   **/
  public flush = function () {
    // If the buffer contains a whole packet when we are being flushed, emit it
    // and empty the buffer. Otherwise hold onto the data because it may be
    // important for decoding the next segment
    if (
      this._bytesInBuffer === MP2T_PACKET_LENGTH &&
      this._buffer[0] === SYNC_BYTE
    ) {
      this.trigger("data", this._buffer);
      this._bytesInBuffer = 0;
    }
    this.trigger("done", null);
  };

  public endTimeline = function () {
    this.flush();
    this.trigger("endedtimeline", null);
  };

  public reset = function () {
    this._bytesInBuffer = 0;
    this.trigger("reset", null);
  };
}

export interface TransportParseStreamEvents {
  data: Uint8Array;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * Accepts an MP2T TransportPacketStream and emits data events with parsed
 * forms of the individual transport stream packets.
 */
class TransportParseStream extends EventEmitter<TransportParseStreamEvents> {
  private packetsWaitingForPmt: Array<[Uint8Array, number, any]>;
  private programMapTable: {
    audio: number | null;
    video: number | null;
    /* eslint-disable @typescript-eslint/naming-convention */
    "timed-metadata": Record<number, number>;
    /* eslint-enable @typescript-eslint/naming-convention */
  } | null;
  private pmtPid: number | undefined;

  constructor() {
    super();
    this.packetsWaitingForPmt = [];
    this.programMapTable = null;
  }

  private _parsePsi(payload: Uint8Array, psi: any): void {
    let offset = 0;

    // PSI packets may be split into multiple sections and those
    // sections may be split into multiple packets. If a PSI
    // section starts in this packet, the payload_unit_start_indicator
    // will be true and the first byte of the payload will indicate
    // the offset from the current position to the start of the
    // section.
    if (psi.payloadUnitStartIndicator as boolean) {
      offset += payload[offset] + 1;
    }

    if (psi.type === "pat") {
      this._parsePat(payload.subarray(offset), psi);
    } else {
      this._parsePmt(payload.subarray(offset), psi);
    }
  }

  private _parsePat(payload: Uint8Array, pat: any): void {
    pat.section_number = payload[7]; // eslint-disable-line camelcase
    pat.last_section_number = payload[8]; // eslint-disable-line camelcase

    // skip the PSI header and parse the first PMT entry
    this.pmtPid = ((payload[10] & 0x1f) << 8) | payload[11];
    pat.pmtPid = this.pmtPid;
  }

  /**
   * Parse out the relevant fields of a Program Map Table (PMT).
   * @param payload {Uint8Array} the PMT-specific portion of an MP2T
   * packet. The first byte in this array should be the table_id
   * field.
   * @param pmt {object} the object that should be decorated with
   * fields parsed from the PMT.
   */
  private _parsePmt(payload: Uint8Array, pmt: any): void {
    // PMTs can be sent ahead of the time when they should actually
    // take effect. We don't believe this should ever be the case
    // for HLS but we'll ignore "forward" PMT declarations if we see
    // them. Future PMT declarations have the current_next_indicator
    // set to zero.
    if (!(payload[5] & 0x01)) {
      return;
    }

    // overwrite any existing program map table
    /* eslint-disable @typescript-eslint/naming-convention */
    this.programMapTable = {
      video: null,
      audio: null,
      "timed-metadata": {},
    };
    /* eslint-enable @typescript-eslint/naming-convention */

    // the mapping table ends at the end of the current section
    const sectionLength = ((payload[1] & 0x0f) << 8) | payload[2];
    const tableEnd = 3 + sectionLength - 4;

    // to determine where the table is, we have to figure out how
    // long the program info descriptors are
    const programInfoLength = ((payload[10] & 0x0f) << 8) | payload[11];

    // advance the offset to the first entry in the mapping table
    let offset = 12 + programInfoLength;
    while (offset < tableEnd) {
      const streamType = payload[offset];
      const pid = ((payload[offset + 1] & 0x1f) << 8) | payload[offset + 2];

      // only map a single elementary_pid for audio and video stream types
      // TODO: should this be done for metadata too? for now maintain behavior of
      //       multiple metadata streams
      if (
        streamType === H264_STREAM_TYPE &&
        this.programMapTable.video === null
      ) {
        this.programMapTable.video = pid;
      } else if (
        streamType === ADTS_STREAM_TYPE &&
        this.programMapTable.audio === null
      ) {
        this.programMapTable.audio = pid;
      } else if (streamType === METADATA_STREAM_TYPE) {
        // map pid to stream type for metadata streams
        this.programMapTable["timed-metadata"][pid] = streamType;
      }

      // move to the next table entry
      // skip past the elementary stream descriptors, if present
      offset += (((payload[offset + 3] & 0x0f) << 8) | payload[offset + 4]) + 5;
    }

    // record the map on the packet as well
    pmt.programMapTable = this.programMapTable;
  }

  /**
   * Deliver a new MP2T packet to the next stream in the pipeline.
   */
  public push(packet: Uint8Array): void {
    let offset = 4;
    const result: any = {};
    result.payloadUnitStartIndicator = !!(packet[1] & 0x40);

    // pid is a 13-bit field starting at the last bit of packet[1]
    result.pid = packet[1] & 0x1f;
    result.pid <<= 8;
    result.pid |= packet[2];

    // if an adaption field is present, its length is specified by the
    // fifth byte of the TS packet header. The adaptation field is
    // used to add stuffing to PES packets that don't fill a complete
    // TS packet, and to specify some forms of timing and control data
    // that we do not currently use.
    if ((packet[3] & 0x30) >>> 4 > 0x01) {
      offset += packet[offset] + 1;
    }

    // parse the rest of the packet based on the type
    if (result.pid === 0) {
      result.type = "pat";
      this._parsePsi(packet.subarray(offset), result);
      this.trigger("data", result);
    } else if (result.pid === this.pmtPid) {
      result.type = "pmt";
      this._parsePsi(packet.subarray(offset), result);
      this.trigger("data", result);

      // if there are any packets waiting for a PMT to be found, process them now
      while (this.packetsWaitingForPmt.length) {
        const waiting = this.packetsWaitingForPmt.shift();
        if (waiting !== undefined) {
          this._processPes(waiting[0], waiting[1], waiting[2]);
        }
      }
    } else if (this.programMapTable === null) {
      // When we have not seen a PMT yet, defer further processing of
      // PES packets until one has been parsed
      this.packetsWaitingForPmt.push([packet, offset, result]);
    } else {
      this._processPes(packet, offset, result);
    }
  }

  private _processPes(packet: Uint8Array, offset: number, result: any): void {
    // set the appropriate stream type
    if (result.pid === this.programMapTable?.video) {
      result.streamType = H264_STREAM_TYPE;
    } else if (result.pid === this.programMapTable?.audio) {
      result.streamType = ADTS_STREAM_TYPE;
    } else {
      // if not video or audio, it is timed-metadata or unknown
      // if unknown, streamType will be undefined
      result.streamType = this.programMapTable?.["timed-metadata"][result.pid];
    }

    result.type = "pes";
    result.data = packet.subarray(offset);
    this.trigger("data", result);
  }
}

const STREAM_TYPES = {
  h264: 0x1b,
  adts: 0x0f,
};

export interface ElementaryStreamEvents {
  data: Uint8Array;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * Reconsistutes program elementary stream (PES) packets from parsed
 * transport stream packets. That is, if you pipe an
 * mp2t.TransportParseStream into a mp2t.ElementaryStream, the output
 * events will be events which capture the bytes for individual PES
 * packets plus relevant metadata that has been extracted from the
 * container.
 */
class ElementaryStream extends EventEmitter<ElementaryStreamEvents> {
  private _segmentHadPmt: boolean;
  private _audio: {
    data: any[];
    size: number;
  };
  private _video: {
    data: any[];
    size: number;
  };
  private _timedMetadata: {
    data: any[];
    size: number;
  };
  private _programMapTable: any | null;

  constructor() {
    super();
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

  private _parsePes(payload: Uint8Array, pes: any): void {
    const startPrefix = (payload[0] << 16) | (payload[1] << 8) | payload[2];
    // default to an empty array
    pes.data = new Uint8Array();
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
   * Pass completely parsed PES packets to the next stream in the pipeline
   **/
  private _flushStream(stream: any, type: string, forceFlush?: boolean): void {
    const packetData = new Uint8Array(stream.size);
    let offset = 0;
    let packetFlushable = false;

    // do nothing if there is not enough buffered data for a complete
    // PES header
    if (stream.data.length === 0 || stream.size < 9) {
      return;
    }
    const event: any = {
      type,
      trackId: stream.data[0].pid,
    };
    // reassemble the packet
    for (let i = 0; i < stream.data.length; i++) {
      const fragment = stream.data[i];

      packetData.set(fragment.data, offset);
      offset += fragment.data.byteLength;
    }

    // parse assembled packet's PES header
    this._parsePes(packetData, event);

    // non-video PES packets MUST have a non-zero PES_packet_length
    // check that there is enough stream data to fill the packet
    packetFlushable = type === "video" || event.packetLength <= stream.size;

    // flush pending packets if the conditions are right
    if (forceFlush === true || packetFlushable) {
      stream.size = 0;
      stream.data.length = 0;
    }

    // only emit packets that are complete. this is to avoid assembling
    // incomplete PES packets due to poor segmentation
    if (packetFlushable) {
      this.trigger("data", event);
    }
  }

  /**
   * Identifies M2TS packet types and parses PES packets using metadata
   * parsed from the PMT
   **/
  public push(data: any): void {
    switch (data.type) {
      case "pat":
        // we have to wait for the PMT to arrive as well before we
        // have any meaningful metadata
        break;
      case "pes":
        {
          let stream: { data: any[]; size: number };
          let streamType: string;
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
              return;
          }

          // if a new packet is starting, we can flush the completed
          // packet
          if (data.payloadUnitStartIndicator as boolean) {
            this._flushStream(stream, streamType, true);
          }

          // buffer this fragment until we are sure we've received the
          // complete payload
          stream.data.push(data);
          stream.size += data.data.byteLength;
        }
        break;
      case "pmt":
        {
          const event: any = {
            type: "metadata",
            tracks: [],
          };

          const programMapTable = data.programMapTable;
          this._programMapTable = programMapTable;

          // translate audio and video streams to tracks
          if (programMapTable.video !== null) {
            event.tracks.push({
              timelineStartInfo: {
                baseMediaDecodeTime: 0,
              },
              id: +programMapTable.video,
              codec: "avc",
              type: "video",
            });
          }
          if (programMapTable.audio !== null) {
            event.tracks.push({
              timelineStartInfo: {
                baseMediaDecodeTime: 0,
              },
              id: +programMapTable.audio,
              codec: "adts",
              type: "audio",
            });
          }

          this._segmentHadPmt = true;

          this.trigger("data", event);
        }
        break;
    }
  }

  public reset() {
    this._video.size = 0;
    this._video.data.length = 0;
    this._audio.size = 0;
    this._audio.data.length = 0;
    this.trigger("reset", null);
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
  public flushStreams_() {
    // !!THIS ORDER IS IMPORTANT!!
    // video first then audio
    this._flushStream(this._video, "video");
    this._flushStream(this._audio, "audio");
    this._flushStream(this._timedMetadata, "timed-metadata");
  }

  public flush() {
    // if on flush we haven't had a pmt emitted
    // and we have a pmt to emit. emit the pmt
    // so that we trigger a trackinfo downstream.
    if (!this._segmentHadPmt && this._programMapTable !== null) {
      const programMapTable = this._programMapTable;
      const pmt: any = {
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

      this.trigger("data", pmt);
    }

    this._segmentHadPmt = false;
    this.flushStreams_();
    this.trigger("done", null);
  }
}

export {
  MetadataStream,
  PAT_PID,
  MP2T_PACKET_LENGTH,
  TransportPacketStream,
  TransportParseStream,
  ElementaryStream,
  TimestampRolloverStream,
  CaptionStream,
  Cea608Stream,
  Cea708Stream,
  H264_STREAM_TYPE,
  ADTS_STREAM_TYPE,
  METADATA_STREAM_TYPE,
  STREAM_TYPES,
};
