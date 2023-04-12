import { VideoElementaryPacket } from "./ElementaryPacketParser";
import ExpGolomb from "./exp-golomb";

/**
 * Accepts a NAL unit byte stream and unpacks the embedded NAL units.
 */
class NalUnitFinder {
  private _syncPoint: number;
  private _buffer: Uint8Array | null;

  // TODO Better name
  private _nalBound: number | undefined;

  constructor() {
    this._syncPoint = 0;
    this._buffer = null;
    this._nalBound = undefined;
  }

  /**
   * Scans a byte stream and return the NAL units found.
   * @param {Uint8Array} data - The h264 byte stream to be scanned
   * @returns {Uint8Array|null}
   */
  public findNext(data: Uint8Array): Uint8Array[] {
    if (this._buffer === null) {
      this._buffer = data;
    } else {
      const swapBuffer = new Uint8Array(
        this._buffer.byteLength + data.byteLength
      );
      swapBuffer.set(this._buffer);
      swapBuffer.set(data, this._buffer.byteLength);
      this._buffer = swapBuffer;
    }

    const len = this._buffer.byteLength;
    const res: Uint8Array[] = [];

    // Rec. ITU-T H.264, Annex B
    // scan for NAL unit boundaries

    // a match looks like this:
    // 0 0 1 .. NAL .. 0 0 1
    // ^ sync point        ^ i
    // or this:
    // 0 0 1 .. NAL .. 0 0 0
    // ^ sync point        ^ i

    // advance the sync point to a NAL start, if necessary
    for (; this._syncPoint < len - 3; this._syncPoint++) {
      if (this._buffer[this._syncPoint + 2] === 1) {
        // the sync point is properly aligned
        this._nalBound = this._syncPoint + 5;
        break;
      }
    }

    const buffer = this._buffer;
    while (this._nalBound !== undefined && this._nalBound < len) {
      // look at the current byte to determine if we've hit the end of
      // a NAL unit boundary
      switch (buffer[this._nalBound]) {
        case 0:
          // skip past non-sync sequences
          if (buffer[this._nalBound - 1] !== 0) {
            this._nalBound += 2;
            break;
          } else if (buffer[this._nalBound - 2] !== 0) {
            this._nalBound++;
            break;
          }

          // deliver the NAL unit if it isn't empty
          if (this._syncPoint + 3 !== this._nalBound - 2) {
            res.push(buffer.subarray(this._syncPoint + 3, this._nalBound - 2));
          }

          // drop trailing zeroes
          do {
            this._nalBound++;
          } while (buffer[this._nalBound] !== 1 && this._nalBound < len);
          this._syncPoint = this._nalBound - 2;
          this._nalBound += 3;
          break;
        case 1:
          // skip past non-sync sequences
          if (
            buffer[this._nalBound - 1] !== 0 ||
            buffer[this._nalBound - 2] !== 0
          ) {
            this._nalBound += 3;
            break;
          }

          // deliver the NAL unit
          res.push(buffer.subarray(this._syncPoint + 3, this._nalBound - 2));
          this._syncPoint = this._nalBound - 2;
          this._nalBound += 3;
          break;
        default:
          // the current byte isn't a one or zero, so it cannot be part
          // of a sync sequence
          this._nalBound += 3;
          break;
      }
    }
    // filter out the NAL units that were delivered
    this._buffer = this._buffer.subarray(this._syncPoint);
    if (this._nalBound !== undefined) {
      this._nalBound -= this._syncPoint;
    }
    this._syncPoint = 0;
    return res;
  }

  public reset(): void {
    this._buffer = null;
    this._nalBound = undefined;
    this._syncPoint = 0;
  }

  public flush(): Uint8Array | null {
    let nalUnit: Uint8Array | null = null;
    // deliver the last buffered NAL unit
    if (this._buffer !== null && this._buffer.byteLength > 3) {
      nalUnit = this._buffer.subarray(this._syncPoint + 3);
    }
    this._buffer = null;
    this._syncPoint = 0;
    this._nalBound = undefined;
    return nalUnit;
  }
}

// values of profile_idc that indicate additional fields are included in the SPS
// see Recommendation ITU-T H.264 (4/2013),
// 7.3.2.1.1 Sequence parameter set data syntax
/* eslint-disable @typescript-eslint/naming-convention */
const PROFILES_WITH_OPTIONAL_SPS_DATA: Partial<Record<number, true>> = {
  100: true,
  110: true,
  122: true,
  244: true,
  44: true,
  83: true,
  86: true,
  118: true,
  128: true,

  // TODO: the three profiles below don't
  // appear to have sps data in the specificiation anymore?
  138: true,
  139: true,
  134: true,
};
/* eslint-enable @typescript-eslint/naming-convention */

export const enum NalUnitType {
  SliceLayerWo = 0,
  SeiRbsp = 1,
  SeqParamSetRbsp = 2,
  PicParamSet = 3,
  AccessUnitDelim = 4,
  Undefined = 5,
}

export interface SliceLayerWoNalUnit extends ParsedNalUnitBase {
  nalUnitType: NalUnitType.SliceLayerWo;
}

export interface SeiRbspNalUnit extends ParsedNalUnitBase {
  nalUnitType: NalUnitType.SeiRbsp;
  escapedRBSP: Uint8Array;
}

export interface SeqParamSetRbspNalUnit extends ParsedNalUnitBase {
  nalUnitType: NalUnitType.SeqParamSetRbsp;
  escapedRBSP: Uint8Array;
  config: NalVideoProperties;
}

export interface PicParamSetNalUnit extends ParsedNalUnitBase {
  nalUnitType: NalUnitType.PicParamSet;
}

export interface AccessUnitDelimNalUnit extends ParsedNalUnitBase {
  nalUnitType: NalUnitType.AccessUnitDelim;
}

export interface UndefinedNalUnit extends ParsedNalUnitBase {
  nalUnitType: NalUnitType.Undefined;
}

export type ParsedNalUnit =
  | SliceLayerWoNalUnit
  | SeiRbspNalUnit
  | SeqParamSetRbspNalUnit
  | PicParamSetNalUnit
  | AccessUnitDelimNalUnit
  | UndefinedNalUnit;

interface ParsedNalUnitBase {
  trackId: number;
  pts: number;
  dts: number;
  data: Uint8Array;
}

export interface NalVideoProperties {
  profileIdc: number;
  levelIdc: number;
  profileCompatibility: number;
  width: number;
  height: number;
  sarRatio: number[];
}

/**
 * Produces H.264 NAL unit data events.
 * @class H264NalUnitProducer
 */
export default class H264NalUnitProducer {
  private _nalUnitFinder: NalUnitFinder;
  private _lastTrackId: number | undefined;
  private _lastPts: number | undefined;
  private _lastDts: number | undefined;
  constructor() {
    this._nalUnitFinder = new NalUnitFinder();
    this._lastTrackId = undefined;
    this._lastPts = undefined;
    this._lastDts = undefined;
  }

  /**
   * Pushes a video packet to parse its inner Nal Units.
   * @param {Object} packet
   * @param {Uint8Array} packet.data - The raw bytes of the packet
   * @param {number} packet.dts - Decode timestamp of the packet
   * @param {number} packet.pts - Presentation timestamp of the packet
   * @param {number} packet.trackId - The id of the h264 track this packet came
   * from.
   * @param {string} packet.type - The type of packet. Should normally be video.
   */
  public pushPacket(packet: VideoElementaryPacket): ParsedNalUnit[] {
    if (packet.type !== "video") {
      return [];
    }
    const trackId = packet.trackId;
    const pts = packet.pts as number;
    const dts = packet.dts as number;
    this._lastTrackId = trackId;
    this._lastPts = pts;
    this._lastDts = dts;
    const nalUnits = this._nalUnitFinder.findNext(packet.data);
    return nalUnits.map((data) => {
      return this._onNalUnit(data, trackId, pts, dts);
    });
  }

  /**
   * Identify NAL unit types and pass on the NALU, trackId, presentation and
   * decode timestamps.
   * Also, preprocess caption and sequence parameter NALUs.
   *
   * @param {Uint8Array} data - A NAL unit
   */
  private _onNalUnit(
    data: Uint8Array,
    trackId: number,
    pts: number,
    dts: number
  ): ParsedNalUnit {
    const parsed: ParsedNalUnitBase = {
      trackId,
      pts,
      dts,
      data,
    };
    const nalUnitTypeCode = data[0] & 0x1f;
    switch (nalUnitTypeCode) {
      case 0x05:
        return Object.assign(parsed, {
          nalUnitType: NalUnitType.SliceLayerWo,
        } as const);
      case 0x06:
        return Object.assign(parsed, {
          nalUnitType: NalUnitType.SeiRbsp,
          escapedRBSP: this._discardEmulationPreventionBytes(data.subarray(1)),
        } as const);
      case 0x07:
        const escapedRBSP = this._discardEmulationPreventionBytes(
          data.subarray(1)
        );
        return Object.assign(parsed, {
          nalUnitType: NalUnitType.SeqParamSetRbsp,
          escapedRBSP,
          config: this._readSequenceParameterSet(escapedRBSP),
        } as const);
      case 0x08:
        return Object.assign(parsed, {
          nalUnitType: NalUnitType.PicParamSet,
        } as const);
      case 0x09:
        return Object.assign(parsed, {
          nalUnitType: NalUnitType.AccessUnitDelim,
        } as const);
      default:
        return Object.assign(parsed, {
          nalUnitType: NalUnitType.Undefined,
        } as const);
    }
  }

  public flush(): ParsedNalUnit | null {
    const lastNalUnit = this._nalUnitFinder.flush();
    if (
      lastNalUnit !== null &&
      this._lastTrackId !== undefined &&
      this._lastDts !== undefined &&
      this._lastPts !== undefined
    ) {
      return this._onNalUnit(
        lastNalUnit,
        this._lastTrackId,
        this._lastPts,
        this._lastDts
      );
    }
    return null;
  }

  public reset() {
    this._nalUnitFinder.reset();
  }

  /**
   * Advance the ExpGolomb decoder past a scaling list. The scaling
   * list is optionally transmitted as part of a sequence parameter
   * set and is not relevant to transmuxing.
   * @param count {number} the number of entries in this scaling list
   * @param expGolombDecoder {object} an ExpGolomb pointed to the
   * start of a scaling list
   * @see Recommendation ITU-T H.264, Section 7.3.2.1.1.1
   */
  private _skipScalingList(count: number, expGolombDecoder: ExpGolomb): void {
    let lastScale = 8;
    let nextScale = 8;

    for (let j = 0; j < count; j++) {
      if (nextScale !== 0) {
        const deltaScale = expGolombDecoder.readExpGolomb();
        nextScale = (lastScale + deltaScale + 256) % 256;
      }

      lastScale = nextScale === 0 ? lastScale : nextScale;
    }
  }

  /**
   * Expunge any "Emulation Prevention" bytes from a "Raw Byte
   * Sequence Payload"
   * @param data {Uint8Array} the bytes of a RBSP from a NAL
   * unit
   * @return {Uint8Array} the RBSP without any Emulation
   * Prevention Bytes
   */
  private _discardEmulationPreventionBytes(data: Uint8Array): Uint8Array {
    const length = data.byteLength;
    const emulationPreventionBytesPositions: number[] = [];
    let i = 1;

    // Find all `Emulation Prevention Bytes`
    while (i < length - 2) {
      if (data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 0x03) {
        emulationPreventionBytesPositions.push(i + 2);
        i += 2;
      } else {
        i++;
      }
    }

    // If no Emulation Prevention Bytes were found just return the original
    // array
    if (emulationPreventionBytesPositions.length === 0) {
      return data;
    }

    // Create a new array to hold the NAL unit data
    const newLength = length - emulationPreventionBytesPositions.length;
    const newData = new Uint8Array(newLength);
    let sourceIndex = 0;

    for (i = 0; i < newLength; sourceIndex++, i++) {
      if (sourceIndex === emulationPreventionBytesPositions[0]) {
        // Skip this byte
        sourceIndex++;
        // Remove this position index
        emulationPreventionBytesPositions.shift();
      }
      newData[i] = data[sourceIndex];
    }

    return newData;
  }

  /**
   * Read a sequence parameter set and return some interesting video
   * properties. A sequence parameter set is the H264 metadata that
   * describes the properties of upcoming video frames.
   * @param data {Uint8Array} the bytes of a sequence parameter set
   * @return {object} an object with configuration parsed from the
   * sequence parameter set, including the dimensions of the
   * associated video frames.
   */
  private _readSequenceParameterSet(data: Uint8Array): NalVideoProperties {
    let frameCropLeftOffset = 0;
    let frameCropRightOffset = 0;
    let frameCropTopOffset = 0;
    let frameCropBottomOffset = 0;
    let sarRatio = [1, 1];

    const expGolombDecoder = new ExpGolomb(data);
    const profileIdc = expGolombDecoder.readUnsignedByte(); // profile_idc
    // constraint_set[0-5]_flag
    const profileCompatibility = expGolombDecoder.readUnsignedByte();
    const levelIdc = expGolombDecoder.readUnsignedByte(); // level_idc u(8)
    expGolombDecoder.skipUnsignedExpGolomb(); // seq_parameter_set_id

    // some profiles have more optional data we don't need
    if (PROFILES_WITH_OPTIONAL_SPS_DATA[profileIdc] !== undefined) {
      const chromaFormatIdc = expGolombDecoder.readUnsignedExpGolomb();
      if (chromaFormatIdc === 3) {
        expGolombDecoder.skipBits(1); // separate_colour_plane_flag
      }
      expGolombDecoder.skipUnsignedExpGolomb(); // bit_depth_luma_minus8
      expGolombDecoder.skipUnsignedExpGolomb(); // bit_depth_chroma_minus8
      expGolombDecoder.skipBits(1); // qpprime_y_zero_transform_bypass_flag
      if (expGolombDecoder.readBoolean()) {
        // seq_scaling_matrix_present_flag
        const scalingListCount = chromaFormatIdc !== 3 ? 8 : 12;
        for (let i = 0; i < scalingListCount; i++) {
          if (expGolombDecoder.readBoolean()) {
            // seq_scaling_list_present_flag[ i ]
            if (i < 6) {
              this._skipScalingList(16, expGolombDecoder);
            } else {
              this._skipScalingList(64, expGolombDecoder);
            }
          }
        }
      }
    }

    expGolombDecoder.skipUnsignedExpGolomb(); // log2_max_frame_num_minus4
    const picOrderCntType = expGolombDecoder.readUnsignedExpGolomb();

    if (picOrderCntType === 0) {
      expGolombDecoder.readUnsignedExpGolomb(); // log2_max_pic_order_cnt_lsb_minus4
    } else if (picOrderCntType === 1) {
      expGolombDecoder.skipBits(1); // delta_pic_order_always_zero_flag
      expGolombDecoder.skipExpGolomb(); // offset_for_non_ref_pic
      expGolombDecoder.skipExpGolomb(); // offset_for_top_to_bottom_field
      const numRefFramesInPicOrderCntCycle =
        expGolombDecoder.readUnsignedExpGolomb();
      for (let i = 0; i < numRefFramesInPicOrderCntCycle; i++) {
        expGolombDecoder.skipExpGolomb(); // offset_for_ref_frame[ i ]
      }
    }

    expGolombDecoder.skipUnsignedExpGolomb(); // max_num_ref_frames
    expGolombDecoder.skipBits(1); // gaps_in_frame_num_value_allowed_flag

    const picWidthInMbsMinus1 = expGolombDecoder.readUnsignedExpGolomb();
    const picHeightInMapUnitsMinus1 = expGolombDecoder.readUnsignedExpGolomb();

    const frameMbsOnlyFlag = expGolombDecoder.readBits(1);
    if (frameMbsOnlyFlag === 0) {
      expGolombDecoder.skipBits(1); // mb_adaptive_frame_field_flag
    }

    expGolombDecoder.skipBits(1); // direct_8x8_inference_flag
    if (expGolombDecoder.readBoolean()) {
      // frame_cropping_flag
      frameCropLeftOffset = expGolombDecoder.readUnsignedExpGolomb();
      frameCropRightOffset = expGolombDecoder.readUnsignedExpGolomb();
      frameCropTopOffset = expGolombDecoder.readUnsignedExpGolomb();
      frameCropBottomOffset = expGolombDecoder.readUnsignedExpGolomb();
    }

    if (expGolombDecoder.readBoolean()) {
      // vui_parameters_present_flag
      if (expGolombDecoder.readBoolean()) {
        // aspect_ratio_info_present_flag
        const aspectRatioIdc = expGolombDecoder.readUnsignedByte();
        switch (aspectRatioIdc) {
          case 1:
            sarRatio = [1, 1];
            break;
          case 2:
            sarRatio = [12, 11];
            break;
          case 3:
            sarRatio = [10, 11];
            break;
          case 4:
            sarRatio = [16, 11];
            break;
          case 5:
            sarRatio = [40, 33];
            break;
          case 6:
            sarRatio = [24, 11];
            break;
          case 7:
            sarRatio = [20, 11];
            break;
          case 8:
            sarRatio = [32, 11];
            break;
          case 9:
            sarRatio = [80, 33];
            break;
          case 10:
            sarRatio = [18, 11];
            break;
          case 11:
            sarRatio = [15, 11];
            break;
          case 12:
            sarRatio = [64, 33];
            break;
          case 13:
            sarRatio = [160, 99];
            break;
          case 14:
            sarRatio = [4, 3];
            break;
          case 15:
            sarRatio = [3, 2];
            break;
          case 16:
            sarRatio = [2, 1];
            break;
          case 255: {
            sarRatio = [
              (expGolombDecoder.readUnsignedByte() << 8) |
                expGolombDecoder.readUnsignedByte(),
              (expGolombDecoder.readUnsignedByte() << 8) |
                expGolombDecoder.readUnsignedByte(),
            ];
            break;
          }
        }
      }
    }
    return {
      profileIdc,
      levelIdc,
      profileCompatibility,
      width:
        (picWidthInMbsMinus1 + 1) * 16 -
        frameCropLeftOffset * 2 -
        frameCropRightOffset * 2,
      height:
        (2 - frameMbsOnlyFlag) * (picHeightInMapUnitsMinus1 + 1) * 16 -
        frameCropTopOffset * 2 -
        frameCropBottomOffset * 2,
      // sar is sample aspect ratio
      sarRatio,
    };
  }
}

export { NalUnitFinder };
