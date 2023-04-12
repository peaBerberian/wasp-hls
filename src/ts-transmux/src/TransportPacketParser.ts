import {
  ADTS_STREAM_TYPE,
  H264_STREAM_TYPE,
  METADATA_STREAM_TYPE,
} from "./constants";

export interface PatPacket extends PacketBase {
  type: "pat";
  sectionNumber: number;
  lastSectionNumber: number;
  pmtPid: number;
}

export interface PmtPacket extends PacketBase {
  type: "pmt";
  programMapTable: ProgramMapTable;
}

export interface PesPacket extends PacketBase {
  type: "pes";
  streamType: number | undefined;
  data: Uint8Array;
}

export interface ProgramMapTable {
  audio: number | null;
  video: number | null;
  /* eslint-disable @typescript-eslint/naming-convention */
  "timed-metadata": Record<number, number>;
  /* eslint-enable @typescript-eslint/naming-convention */
}

export type ParsedPacket = PatPacket | PmtPacket | PesPacket;

interface PartialPatPacket extends PacketBase {
  type: "pat";
  /* eslint-disable @typescript-eslint/naming-convention */
  section_number?: number;
  last_section_number?: number;
  /* eslint-enable @typescript-eslint/naming-convention */
  pmtPid?: number;
}
interface PartialPmtPacket extends PacketBase {
  type: "pmt";
  programMapTable?: ProgramMapTable;
}
interface PacketBase {
  pid: number;
  payloadUnitStartIndicator: boolean;
}

/**
 * Accepts an MP2T Transport packet and parses it into legible
 * information.
 * @class TransportPacketParser
 */
export default class TransportPacketParser {
  private packetsWaitingForPmt: Array<[Uint8Array, number, PacketBase]>;
  private programMapTable: ProgramMapTable | null;
  private pmtPid: number | undefined;

  constructor() {
    this.packetsWaitingForPmt = [];
    this.programMapTable = null;
  }

  /**
   * Parse a new MP2T Transport packet.
   * @param {Uint8Array} packet
   * @returns {Array.<Object>}
   */
  public parse(packet: Uint8Array): ParsedPacket[] {
    let offset = 4;

    const payloadUnitStartIndicator = !!(packet[1] & 0x40);

    // pid is a 13-bit field starting at the last bit of packet[1]
    let pid = packet[1] & 0x1f;
    pid <<= 8;
    pid |= packet[2];

    // if an adaption field is present, its length is specified by the
    // fifth byte of the TS packet header. The adaptation field is
    // used to add stuffing to PES packets that don't fill a complete
    // TS packet, and to specify some forms of timing and control data
    // that we do not currently use.
    if ((packet[3] & 0x30) >>> 4 > 0x01) {
      offset += packet[offset] + 1;
    }

    // parse the rest of the packet based on the type
    if (pid === 0) {
      const part: PartialPatPacket = {
        type: "pat",
        pid,
        payloadUnitStartIndicator,
      };
      return [this._parsePsi(packet.subarray(offset), part)];
    } else if (pid === this.pmtPid) {
      const part: PartialPmtPacket = {
        type: "pmt",
        pid,
        payloadUnitStartIndicator,
      };
      const result = this._parsePsi(packet.subarray(offset), part);
      if (result === null) {
        return [];
      }
      const ret: ParsedPacket[] = [result];

      // if there are any packets waiting for a PMT to be found, process them now
      while (this.packetsWaitingForPmt.length) {
        const waiting = this.packetsWaitingForPmt.shift();
        if (waiting !== undefined) {
          ret.push(this._processPes(waiting[0], waiting[1], waiting[2]));
        }
      }
      return ret;
    } else if (this.programMapTable === null) {
      // When we have not seen a PMT yet, defer further processing of
      // PES packets until one has been parsed
      this.packetsWaitingForPmt.push([
        packet,
        offset,
        { pid, payloadUnitStartIndicator },
      ]);
      return [];
    } else {
      return [
        this._processPes(packet, offset, { pid, payloadUnitStartIndicator }),
      ];
    }
  }

  private _parsePsi(payload: Uint8Array, psi: PartialPatPacket): PatPacket;
  private _parsePsi(
    payload: Uint8Array,
    psi: PartialPmtPacket
  ): PmtPacket | null;
  private _parsePsi(
    payload: Uint8Array,
    psi: PartialPatPacket | PartialPmtPacket
  ): PatPacket | PmtPacket | null {
    let offset = 0;

    // PSI packets may be split into multiple sections and those
    // sections may be split into multiple packets. If a PSI
    // section starts in this packet, the payload_unit_start_indicator
    // will be true and the first byte of the payload will indicate
    // the offset from the current position to the start of the
    // section.
    if (psi.payloadUnitStartIndicator) {
      offset += payload[offset] + 1;
    }

    if (psi.type === "pat") {
      return this._parsePat(payload.subarray(offset), psi);
    } else {
      return this._parsePmt(payload.subarray(offset), psi);
    }
  }

  private _parsePat(payload: Uint8Array, pat: PartialPatPacket): PatPacket {
    const fullPat = Object.assign(pat, {
      sectionNumber: payload[7],
      lastSectionNumber: payload[8],

      // skip the PSI header and parse the first PMT entry
      pmtPid: ((payload[10] & 0x1f) << 8) | payload[11],
    });
    this.pmtPid = fullPat.pmtPid;
    return fullPat;
  }

  /**
   * Parse out the relevant fields of a Program Map Table (PMT).
   * @param {Uint8Array} payload - The PMT-specific portion of an MP2T
   * packet. The first byte in this array should be the table_id
   * field.
   * @param {Object} pmt - The object that should be decorated with
   * fields parsed from the PMT.
   * @returns {Object|null}
   */
  private _parsePmt(
    payload: Uint8Array,
    pmt: PartialPmtPacket
  ): PmtPacket | null {
    // PMTs can be sent ahead of the time when they should actually
    // take effect. We don't believe this should ever be the case
    // for HLS but we'll ignore "forward" PMT declarations if we see
    // them. Future PMT declarations have the current_next_indicator
    // set to zero.
    if (!(payload[5] & 0x01)) {
      return null;
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

    return Object.assign(pmt, {
      programMapTable: this.programMapTable,
    });
  }

  private _processPes(
    packet: Uint8Array,
    offset: number,
    result: PacketBase
  ): PesPacket {
    // set the appropriate stream type
    let streamType: number | undefined;
    if (result.pid === this.programMapTable?.video) {
      streamType = H264_STREAM_TYPE;
    } else if (result.pid === this.programMapTable?.audio) {
      streamType = ADTS_STREAM_TYPE;
    } else {
      // if not video or audio, it is timed-metadata or unknown
      // if unknown, streamType will be undefined
      streamType = this.programMapTable?.["timed-metadata"][result.pid];
    }

    return {
      pid: result.pid,
      payloadUnitStartIndicator: result.payloadUnitStartIndicator,
      streamType,
      type: "pes",
      data: packet.subarray(offset),
    };
  }
}
