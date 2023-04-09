import EventEmitter from "../../ts-common/EventEmitter";
import ExpGolomb from "./exp-golomb";

export interface NalByteStreamEvents {
  data: Uint8Array;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * Accepts a NAL unit byte stream and unpacks the embedded NAL units.
 */
class NalByteStream extends EventEmitter<NalByteStreamEvents> {
  private _syncPoint: number;
  private _buffer: Uint8Array | null;

  // TODO Better name
  private _nalBound: number | undefined;
  constructor() {
    super();
    this._syncPoint = 0;
    this._buffer = null;
    this._nalBound = undefined;
  }

  /*
   * Scans a byte stream and triggers a data event with the NAL units found.
   * @param {Object} data Event received from H264Stream
   * @param {Uint8Array} data.data The h264 byte stream to be scanned
   *
   * @see H264Stream.push
   */
  public push(data: any): void {
    if (this._buffer === null) {
      this._buffer = data.data as Uint8Array;
    } else {
      const swapBuffer = new Uint8Array(
        this._buffer.byteLength + data.data.byteLength
      );
      swapBuffer.set(this._buffer);
      swapBuffer.set(data.data, this._buffer.byteLength);
      this._buffer = swapBuffer;
    }

    const len = this._buffer.byteLength;

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
            this.trigger(
              "data",
              buffer.subarray(this._syncPoint + 3, this._nalBound - 2)
            );
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
          this.trigger(
            "data",
            buffer.subarray(this._syncPoint + 3, this._nalBound - 2)
          );
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
  }

  public reset(): void {
    this._buffer = null;
    this._nalBound = undefined;
    this._syncPoint = 0;
    this.trigger("reset", null);
  }

  public flush(): void {
    // deliver the last buffered NAL unit
    if (this._buffer !== null && this._buffer.byteLength > 3) {
      this.trigger("data", this._buffer.subarray(this._syncPoint + 3));
    }
    // reset the stream state
    this._buffer = null;
    this._syncPoint = 0;
    this._nalBound = undefined;
    this.trigger("done", null);
  }

  public partialFlush(): void {
    this.trigger("partialdone", null);
  }

  public endTimeline() {
    this.flush();
    this.trigger("endedtimeline", null);
  }
}

// values of profile_idc that indicate additional fields are included in the SPS
// see Recommendation ITU-T H.264 (4/2013),
// 7.3.2.1.1 Sequence parameter set data syntax
/* eslint-disable @typescript-eslint/naming-convention */
const PROFILES_WITH_OPTIONAL_SPS_DATA = {
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
} as const;
/* eslint-enable @typescript-eslint/naming-convention */

export interface H264StreamEvents {
  data: any;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

/**
 * Accepts input from a ElementaryStream and produces H.264 NAL unit data
 * events.
 */
class H264Stream extends EventEmitter<H264StreamEvents> {
  private _nalByteStream: NalByteStream;
  private _trackId: number | undefined;
  private _currentPts: number | undefined;
  private _currentDts: number | undefined;
  constructor() {
    super();
    this._nalByteStream = new NalByteStream();
    this._trackId = undefined;
    this._currentPts = undefined;
    this._currentDts = undefined;

    /*
     * Identify NAL unit types and pass on the NALU, trackId, presentation and
     * decode timestamps for the NALUs to the next stream component.
     * Also, preprocess caption and sequence parameter NALUs.
     *
     * @param {Uint8Array} data - A NAL unit identified by `NalByteStream.push`
     * @see NalByteStream.push
     */
    this._nalByteStream.addEventListener("data", (data: Uint8Array): void => {
      const event: any = {
        trackId: this._trackId,
        pts: this._currentPts,
        dts: this._currentDts,
        data,
        nalUnitTypeCode: data[0] & 0x1f,
      };

      switch (event.nalUnitTypeCode) {
        case 0x05:
          event.nalUnitType = "slice_layer_without_partitioning_rbsp_idr";
          break;
        case 0x06:
          event.nalUnitType = "sei_rbsp";
          event.escapedRBSP = this._discardEmulationPreventionBytes(
            data.subarray(1)
          );
          break;
        case 0x07:
          event.nalUnitType = "seq_parameter_set_rbsp";
          event.escapedRBSP = this._discardEmulationPreventionBytes(
            data.subarray(1)
          );
          event.config = this._readSequenceParameterSet(event.escapedRBSP);
          break;
        case 0x08:
          event.nalUnitType = "pic_parameter_set_rbsp";
          break;
        case 0x09:
          event.nalUnitType = "access_unit_delimiter_rbsp";
          break;
        default:
          break;
      }
      // This triggers data on the H264Stream
      this.trigger("data", event);
    });
    this._nalByteStream.addEventListener("done", () => {
      this.trigger("done", null);
    });
    this._nalByteStream.addEventListener("partialdone", () => {
      this.trigger("partialdone", null);
    });
    this._nalByteStream.addEventListener("reset", () => {
      this.trigger("reset", null);
    });
    this._nalByteStream.addEventListener("endedtimeline", () => {
      this.trigger("endedtimeline", null);
    });
  }

  /*
   * Pushes a packet from a stream onto the NalByteStream
   *
   * @param {Object} packet - A packet received from a stream
   * @param {Uint8Array} packet.data - The raw bytes of the packet
   * @param {Number} packet.dts - Decode timestamp of the packet
   * @param {Number} packet.pts - Presentation timestamp of the packet
   * @param {Number} packet.trackId - The id of the h264 track this packet came from
   * @param {('video'|'audio')} packet.type - The type of packet
   *
   */
  public push(packet: any): void {
    if (packet.type !== "video") {
      return;
    }
    this._trackId = packet.trackId;
    this._currentPts = packet.pts;
    this._currentDts = packet.dts;
    this._nalByteStream.push(packet);
  }

  public flush() {
    this._nalByteStream.flush();
  }

  public partialFlush() {
    this._nalByteStream.partialFlush();
  }

  public reset() {
    this._nalByteStream.reset();
  }

  public endTimeline() {
    this._nalByteStream.endTimeline();
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
  private _readSequenceParameterSet(data: Uint8Array): any {
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

export { H264Stream, NalByteStream };
