import EventEmitter from "../../ts-common/EventEmitter";
import logger from "../../ts-common/logger";
import {
  parseSei,
  parseUserData,
  parseCaptionPackets,
  USER_DATA_REGISTERED_ITU_T_T35,
  CaptionPacket,
} from "./caption-packet-parser";

const PADDING = 0x0000;

export interface CaptionStreamEvents {
  data: any;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

class CaptionStream extends EventEmitter<CaptionStreamEvents> {
  // parse708captions flag, default to true
  private _parse708Captions: boolean;

  private _captionPackets: any[];

  private latestDts_: number | null;
  private ignoreNextEqualDts_: boolean;
  private numSameDts_: number;
  private activeCea608Channel_: [number | null, number | null];

  private ccStreams_: [Cea608Stream, Cea608Stream, Cea608Stream, Cea608Stream];

  private cc708Stream_: Cea708Stream | null;

  constructor(options: any = {}) {
    super();
    this._parse708Captions = options._parse708Captions !== false;
    this._captionPackets = [];
    this.ccStreams_ = [
      new Cea608Stream(0, 0), // eslint-disable-line no-use-before-define
      new Cea608Stream(0, 1), // eslint-disable-line no-use-before-define
      new Cea608Stream(1, 0), // eslint-disable-line no-use-before-define
      new Cea608Stream(1, 1), // eslint-disable-line no-use-before-define
    ];

    this.cc708Stream_ = null;
    if (this._parse708Captions) {
      this.cc708Stream_ = new Cea708Stream({
        captionServices: options.captionServices,
      }); // eslint-disable-line no-use-before-define
    }

    this.latestDts_ = null;
    this.ignoreNextEqualDts_ = false;
    this.numSameDts_ = 0;
    this.activeCea608Channel_ = [null, null];

    // forward data and done events from CCs to this CaptionStream
    this.ccStreams_.forEach((cc) => {
      cc.addEventListener("data", (data) => this.trigger("data", data));
      cc.addEventListener("done", (data) => this.trigger("done", data));
      cc.addEventListener("partialdone", (data) =>
        this.trigger("partialdone", data)
      );
    }, this);

    if (this.cc708Stream_ !== null) {
      this.cc708Stream_.addEventListener("data", (data) =>
        this.trigger("data", data)
      );
      this.cc708Stream_.addEventListener("done", (data) =>
        this.trigger("done", data)
      );
      this.cc708Stream_.addEventListener("partialdone", (data) =>
        this.trigger("partialdone", data)
      );
    }
  }

  public push(event: any): void {
    if (event.nalUnitType !== "sei_rbsp") {
      return;
    }

    // parse the sei
    const sei = parseSei(event.escapedRBSP);

    // no payload data, skip
    if (sei.payload === null) {
      return;
    }

    // ignore everything but user_data_registered_itu_t_t35
    if (sei.payloadType !== USER_DATA_REGISTERED_ITU_T_T35) {
      return;
    }

    // parse out the user data payload
    const userData = parseUserData(sei);

    // ignore unrecognized userData
    if (userData === null) {
      return;
    }

    // Sometimes, the same segment # will be downloaded twice. To stop the
    // caption data from being processed twice, we track the latest dts we've
    // received and ignore everything with a dts before that. However, since
    // data for a specific dts can be split across packets on either side of
    // a segment boundary, we need to make sure we *don't* ignore the packets
    // from the *next* segment that have dts === this.latestDts_. By constantly
    // tracking the number of packets received with dts === this.latestDts_, we
    // know how many should be ignored once we start receiving duplicates.
    if (this.latestDts_ !== null && event.dts < this.latestDts_) {
      // We've started getting older data, so set the flag.
      this.ignoreNextEqualDts_ = true;
      return;
    } else if (event.dts === this.latestDts_ && this.ignoreNextEqualDts_) {
      this.numSameDts_--;
      if (!this.numSameDts_) {
        // We've received the last duplicate packet, time to start processing again
        this.ignoreNextEqualDts_ = false;
      }
      return;
    }

    // parse out CC data packets and save them for later
    const newCaptionPackets = parseCaptionPackets(event.pts, userData);
    this._captionPackets = this._captionPackets.concat(newCaptionPackets);
    if (this.latestDts_ !== event.dts) {
      this.numSameDts_ = 0;
    }
    this.numSameDts_++;
    this.latestDts_ = event.dts;
  }

  flushCCStreams(flushType: string): void {
    this.ccStreams_.forEach(function (cc) {
      return flushType === "flush" ? cc.flush() : cc.partialFlush();
    }, this);
  }

  flushStream(flushType: string): void {
    // make sure we actually parsed captions before proceeding
    if (!this._captionPackets.length) {
      this.flushCCStreams(flushType);
      return;
    }

    // In Chrome, the Array#sort function is not stable so add a
    // presortIndex that we can use to ensure we get a stable-sort
    this._captionPackets.forEach(function (elem, idx) {
      elem.presortIndex = idx;
    });

    // sort caption byte-pairs based on their PTS values
    this._captionPackets.sort(function (a, b) {
      if (a.pts === b.pts) {
        return a.presortIndex - b.presortIndex;
      }
      return a.pts - b.pts;
    });

    this._captionPackets.forEach(function (packet) {
      if (packet.type < 2) {
        // Dispatch packet to the right Cea608Stream
        this.dispatchCea608Packet(packet);
      } else {
        // Dispatch packet to the Cea708Stream
        this.dispatchCea708Packet(packet);
      }
    }, this);

    this._captionPackets.length = 0;
    this.flushCCStreams(flushType);
  }

  flush(): void {
    return this.flushStream("flush");
  }

  partialFlush(): void {
    return this.flushStream("partialFlush");
  }

  reset() {
    this.latestDts_ = null;
    this.ignoreNextEqualDts_ = false;
    this.numSameDts_ = 0;
    this.activeCea608Channel_ = [null, null];
    this.ccStreams_.forEach(function (ccStream) {
      ccStream.reset();
    });
  }

  /**
   * From the CEA-608 spec:
   * When XDS sub-packets are interleaved with other services, the end of each
   * sub-packet shall be followed by a control pair to change to a different
   * service.
   * When any of the control codes from 0x10 to 0x1F is used to begin a control
   * code pair, it indicates the return to captioning or Text data. The control
   * code pair and subsequent data should then be processed according to the FCC
   * rules.
   * It may be necessary for the line 21 data encoder to automatically insert a
   * control code pair (i.e. RCL, RU2, RU3, RU4, RDC, or RTD)
   * to switch to captioning or Text.
   *
   * With that in mind, we ignore any data between an XDS control code and a
   * subsequent closed-captioning control code.
   */
  public dispatchCea608Packet(packet: any): void {
    // NOTE: packet.type is the CEA608 field
    if (this.setsTextOrXDSActive(packet)) {
      this.activeCea608Channel_[packet.type] = null;
    } else if (this.setsChannel1Active(packet)) {
      this.activeCea608Channel_[packet.type] = 0;
    } else if (this.setsChannel2Active(packet)) {
      this.activeCea608Channel_[packet.type] = 1;
    }
    if (this.activeCea608Channel_[packet.type] === null) {
      // If we haven't received anything to set the active channel, or the
      // packets are Text/XDS data, discard the data; we don't want jumbled
      // captions
      return;
    }
    const channel = this.activeCea608Channel_[packet.type];
    if (channel !== null) {
      this.ccStreams_[(packet.type << 1) + channel].push(packet);
    }
  }

  setsChannel1Active(packet: any): boolean {
    return (packet.ccData & 0x7800) === 0x1000;
  }
  setsChannel2Active(packet: any): boolean {
    return (packet.ccData & 0x7800) === 0x1800;
  }
  setsTextOrXDSActive(packet: any): boolean {
    return (
      (packet.ccData & 0x7100) === 0x0100 ||
      (packet.ccData & 0x78fe) === 0x102a ||
      (packet.ccData & 0x78fe) === 0x182a
    );
  }

  dispatchCea708Packet(packet: any): void {
    if (this._parse708Captions) {
      this.cc708Stream_?.push(packet);
    }
  }
}

// ----------------------
// Session to Application
// ----------------------

// This hash maps special and extended character codes to their
// proper Unicode equivalent. The first one-byte key is just a
// non-standard character code. The two-byte keys that follow are
// the extended CEA708 character codes, along with the preceding
// 0x10 extended character byte to distinguish these codes from
// non-extended character codes. Every CEA708 character code that
// is not in this object maps directly to a standard unicode
// character code.
// The transparent space and non-breaking transparent space are
// technically not fully supported since there is no code to
// make them transparent, so they have normal non-transparent
// stand-ins.
// The special closed caption (CC) character isn't a standard
// unicode character, so a fairly similar unicode character was
// chosen in it's place.
/* eslint-disable @typescript-eslint/naming-convention */
const CHARACTER_TRANSLATION_708 = {
  0x7f: 0x266a, // ♪
  0x1020: 0x20, // Transparent Space
  0x1021: 0xa0, // Nob-breaking Transparent Space
  0x1025: 0x2026, // …
  0x102a: 0x0160, // Š
  0x102c: 0x0152, // Œ
  0x1030: 0x2588, // █
  0x1031: 0x2018, // ‘
  0x1032: 0x2019, // ’
  0x1033: 0x201c, // “
  0x1034: 0x201d, // ”
  0x1035: 0x2022, // •
  0x1039: 0x2122, // ™
  0x103a: 0x0161, // š
  0x103c: 0x0153, // œ
  0x103d: 0x2120, // ℠
  0x103f: 0x0178, // Ÿ
  0x1076: 0x215b, // ⅛
  0x1077: 0x215c, // ⅜
  0x1078: 0x215d, // ⅝
  0x1079: 0x215e, // ⅞
  0x107a: 0x23d0, // ⏐
  0x107b: 0x23a4, // ⎤
  0x107c: 0x23a3, // ⎣
  0x107d: 0x23af, // ⎯
  0x107e: 0x23a6, // ⎦
  0x107f: 0x23a1, // ⎡
  0x10a0: 0x3138, // ㄸ (CC char)
};
/* eslint-enable @typescript-eslint/naming-convention */

function get708CharFromCode(code: number): string {
  const newCode = CHARACTER_TRANSLATION_708[code] ?? code;
  if (code & 0x1000 && code === newCode) {
    // Invalid extended code
    return "";
  }
  return String.fromCharCode(newCode);
}

function within708TextBlock(b: number): boolean {
  return (b >= 0x20 && b <= 0x7f) || (b >= 0xa0 && b <= 0xff);
}

class Cea708Window {
  public windowNum: number;
  public rows: string[];
  public rowIdx: number;
  public pendingNewLine: boolean;
  public winAttr: any;
  public penAttr: any;
  public penLoc: any;
  public penColor: any;
  public visible: number;
  public rowLock: number;
  public columnLock: number;
  public priority: number;
  public relativePositioning: number;
  public anchorVertical: number;
  public anchorHorizontal: number;
  public anchorPoint: number;
  public rowCount: number;
  public virtualRowCount: number;
  public columnCount: number;
  public windowStyle: number;
  public penStyle: number;
  public beforeRowOverflow: ((pts: number) => void) | null;

  constructor(windowNum: number) {
    this.windowNum = windowNum;
    this.rows = [""];
    this.rowIdx = 0;
    this.pendingNewLine = false;
    this.winAttr = {};
    this.penAttr = {};
    this.penLoc = {};
    this.penColor = {};
    this.visible = 0;
    this.rowLock = 0;
    this.columnLock = 0;
    this.priority = 0;
    this.relativePositioning = 0;
    this.anchorVertical = 0;
    this.anchorHorizontal = 0;
    this.anchorPoint = 0;
    this.rowCount = 1;
    this.virtualRowCount = this.rowCount + 1;
    this.columnCount = 41;
    this.windowStyle = 0;
    this.penStyle = 0;
    this.beforeRowOverflow = null;
  }

  public reset() {
    this.clearText();
    this.pendingNewLine = false;
    this.winAttr = {};
    this.penAttr = {};
    this.penLoc = {};
    this.penColor = {};

    // These default values are arbitrary,
    // defineWindow will usually override them
    this.visible = 0;
    this.rowLock = 0;
    this.columnLock = 0;
    this.priority = 0;
    this.relativePositioning = 0;
    this.anchorVertical = 0;
    this.anchorHorizontal = 0;
    this.anchorPoint = 0;
    this.rowCount = 1;
    this.virtualRowCount = this.rowCount + 1;
    this.columnCount = 41;
    this.windowStyle = 0;
    this.penStyle = 0;
  }

  public getText(): string {
    return this.rows.join("\n");
  }

  public clearText(): void {
    this.rows = [""];
    this.rowIdx = 0;
  }

  public newLine(pts: number): void {
    if (
      this.rows.length >= this.virtualRowCount &&
      typeof this.beforeRowOverflow === "function"
    ) {
      this.beforeRowOverflow(pts);
    }

    if (this.rows.length > 0) {
      this.rows.push("");
      this.rowIdx++;
    }

    // Show all virtual rows since there's no visible scrolling
    while (this.rows.length > this.virtualRowCount) {
      this.rows.shift();
      this.rowIdx--;
    }
  }

  public isEmpty(): boolean {
    if (this.rows.length === 0) {
      return true;
    } else if (this.rows.length === 1) {
      return this.rows[0] === "";
    }

    return false;
  }

  public addText(text: string): void {
    this.rows[this.rowIdx] += text;
  }

  public backspace(): void {
    if (!this.isEmpty()) {
      const row = this.rows[this.rowIdx];
      this.rows[this.rowIdx] = row.substring(0, row.length - 1);
    }
  }
}

class Cea708Service {
  public serviceNum: number;
  public text: string;
  public currentWindow: Cea708Window;
  public startPts: number | undefined;
  public endPts: number | undefined;
  public windows: Cea708Window[];
  public textDecoder_: TextDecoder | null;

  constructor(
    serviceNum: number,
    encoding: string | undefined,
    pts: number,
    beforeRowOverflow: (pts: number) => void
  ) {
    this.serviceNum = serviceNum;
    this.text = "";
    this.currentWindow = new Cea708Window(-1);
    this.windows = [];

    this.startPts = pts;

    for (let win = 0; win < 8; win++) {
      this.windows[win] = new Cea708Window(win);

      if (typeof beforeRowOverflow === "function") {
        this.windows[win].beforeRowOverflow = beforeRowOverflow;
      }
    }

    // Try to setup a TextDecoder if an `encoding` value was provided
    this.textDecoder_ = null;
    if (typeof encoding === "string") {
      this.createTextDecoder(encoding);
    }
  }

  /**
   * Set current window of service to be affected by commands
   * @param {number} windowNum - Window number
   */
  public setCurrentWindow(windowNum: number): void {
    this.currentWindow = this.windows[windowNum];
  }

  /**
   * Try to create a TextDecoder if it is natively supported
   */
  public createTextDecoder(encoding: string): void {
    if (typeof TextDecoder === "undefined") {
      logger.warn(
        "Transmuxer: The `encoding` option is unsupported without TextDecoder support"
      );
    } else {
      try {
        this.textDecoder_ = new TextDecoder(encoding);
      } catch (error) {
        const err = error instanceof Error ? error : new Error("Unknown Error");
        logger.warn(
          "Transmuxer: TextDecoder could not be created with " +
            encoding +
            " encoding. " +
            err
        );
      }
    }
  }
}

export interface Cea708StreamEvents {
  data: any;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

class Cea708Stream extends EventEmitter<Cea708StreamEvents> {
  private _serviceEncodings: any;
  private _current708Packet: any | null;
  private _services: Record<number, Cea708Service>;
  constructor(options: any = {}) {
    super();
    this._serviceEncodings = {};

    // Get service encodings from captionServices option block
    const captionServices = options.captionServices ?? {};
    Object.keys(captionServices).forEach((serviceName) => {
      const serviceProps = captionServices[serviceName];
      if (/^SERVICE/.test(serviceName)) {
        this._serviceEncodings[serviceName] = serviceProps.encoding;
      }
    });
    this._current708Packet = null;
    this._services = {};
  }

  public push(packet: any): void {
    if (packet.type === 3) {
      // 708 packet start
      this.new708Packet();
      this.add708Bytes(packet);
    } else {
      if (this._current708Packet === null) {
        // This should only happen at the start of a file if there's no packet start.
        this.new708Packet();
      }

      this.add708Bytes(packet);
    }
  }

  public flush() {
    this.trigger("done", null);
  }

  public partialFlush() {
    this.trigger("partialdone", null);
  }

  /**
   * Push current 708 packet, create new 708 packet.
   */
  public new708Packet() {
    if (this._current708Packet !== null) {
      this.push708Packet();
    }

    this._current708Packet = {
      data: [],
      ptsVals: [],
    };
  }

  /**
   * Add pts and both bytes from packet into current 708 packet.
   */
  public add708Bytes(packet: any): void {
    const data = packet.ccData;
    const byte0 = data >>> 8;
    const byte1 = data & 0xff;

    // I would just keep a list of packets instead of bytes, but it isn't clear
    // in the spec
    // that service blocks will always line up with byte pairs.
    this._current708Packet.ptsVals.push(packet.pts);
    this._current708Packet.data.push(byte0);
    this._current708Packet.data.push(byte1);
  }

  /**
   * Parse completed 708 packet into service blocks and push each service block.
   */
  public push708Packet(): void {
    const packet708 = this._current708Packet;
    const packetData = packet708.data;
    let i = 0;
    let b = packetData[i++];
    packet708.seq = b >> 6;
    packet708.sizeCode = b & 0x3f; // 0b00111111;

    for (; i < packetData.length; i++) {
      b = packetData[i++];
      let serviceNum = b >> 5;
      const blockSize = b & 0x1f; // 0b00011111

      if (serviceNum === 7 && blockSize > 0) {
        // Extended service num
        b = packetData[i++];
        serviceNum = b;
      }

      this.pushServiceBlock(serviceNum, i, blockSize);

      if (blockSize > 0) {
        i += blockSize - 1;
      }
    }
  }

  /**
   * Parse service block, execute commands, read text.
   *
   * Note: While many of these commands serve important purposes,
   * many others just parse out the parameters or attributes, but
   * nothing is done with them because this is not a full and complete
   * implementation of the entire 708 spec.
   *
   * @param {number} serviceNum Service number
   * @param {number} start      Start index of the 708 packet data
   * @param {number} size       Block size
   */
  public pushServiceBlock(
    serviceNum: number,
    start: number,
    size: number
  ): void {
    let i = start;
    const packetData = this._current708Packet.data;
    let service = this._services[serviceNum];
    if (service !== undefined) {
      service = this.initService(serviceNum, i);
    }

    for (; i < start + size && i < packetData.length; i++) {
      const b = packetData[i];

      if (within708TextBlock(b)) {
        i = this.handleText(i, service);
      } else if (b === 0x18) {
        i = this.multiByteCharacter(i, service);
      } else if (b === 0x10) {
        i = this.extendedCommands(i, service);
      } else if (b >= 0x80 && b <= 0x87) {
        i = this.setCurrentWindow(i, service);
      } else if (b >= 0x98 && b <= 0x9f) {
        i = this.defineWindow(i, service);
      } else if (b === 0x88) {
        i = this.clearWindows(i, service);
      } else if (b === 0x8c) {
        i = this.deleteWindows(i, service);
      } else if (b === 0x89) {
        i = this.displayWindows(i, service);
      } else if (b === 0x8a) {
        i = this.hideWindows(i, service);
      } else if (b === 0x8b) {
        i = this.toggleWindows(i, service);
      } else if (b === 0x97) {
        i = this.setWindowAttributes(i, service);
      } else if (b === 0x90) {
        i = this.setPenAttributes(i, service);
      } else if (b === 0x91) {
        i = this.setPenColor(i, service);
      } else if (b === 0x92) {
        i = this.setPenLocation(i, service);
      } else if (b === 0x8f) {
        service = this.reset(i, service);
      } else if (b === 0x08) {
        // BS: Backspace
        service.currentWindow.backspace();
      } else if (b === 0x0c) {
        // FF: Form feed
        service.currentWindow.clearText();
      } else if (b === 0x0d) {
        // CR: Carriage return
        service.currentWindow.pendingNewLine = true;
      } else if (b === 0x0e) {
        // HCR: Horizontal carriage return
        service.currentWindow.clearText();
      } else if (b === 0x8d) {
        // DLY: Delay, nothing to do
        i++;
      } else if (b === 0x8e) {
        // DLC: Delay cancel, nothing to do
      } else if (b === 0x03) {
        // ETX: End Text, don't need to do anything
      } else if (b === 0x00) {
        // Padding
      } else {
        // Unknown command
      }
    }
  }

  /**
   * Execute an extended command.
   * @param {number} currIndex - Current index in the 708 packet
   * @param {Object} service - The service object to be affected
   * @return {number} - New index after parsing
   */
  public extendedCommands(currIndex: number, service: Cea708Service): number {
    let i = currIndex;
    const packetData = this._current708Packet.data;
    const b = packetData[++i];
    if (within708TextBlock(b)) {
      i = this.handleText(i, service, { isExtended: true, isMultiByte: false });
    } else {
      // Unknown command
    }
    return i;
  }

  /**
   * Get PTS value of a given byte index
   *
   * @param {number} byteIndex - Index of the byte
   * @return {number}
   */
  public getPts(byteIndex: number): number {
    // There's 1 pts value per 2 bytes
    return this._current708Packet.ptsVals[Math.floor(byteIndex / 2)];
  }

  /**
   * Initializes a service
   * @param {number} serviceNum - Service number
   * @return {Service} - Initialized service object
   */
  public initService(serviceNum: number, i: number): Cea708Service {
    const serviceName = "SERVICE" + serviceNum;
    let encoding: string | undefined;
    if (serviceName in this._serviceEncodings) {
      encoding = this._serviceEncodings[serviceName];
    }

    this._services[serviceNum] = new Cea708Service(
      serviceNum,
      encoding,
      this.getPts(i),
      (pts) => {
        this.flushDisplayed(pts, this._services[serviceNum]);
      }
    );

    return this._services[serviceNum];
  }

  /**
   * Execute text writing to current window
   *
   * @param {number} currIndex        Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public handleText(
    currIndex: number,
    service: Cea708Service,
    options: {
      isExtended: boolean;
      isMultiByte: boolean;
    } = { isExtended: false, isMultiByte: false }
  ): number {
    let i = currIndex;
    const isExtended = options.isExtended;
    const isMultiByte = options.isMultiByte;
    const packetData = this._current708Packet.data;
    const extended = isExtended ? 0x1000 : 0x0000;
    const currentByte = packetData[i];
    const nextByte = packetData[i + 1];
    const win = service.currentWindow;
    let char: string;

    // Use the TextDecoder if one was created for this service
    if (service.textDecoder_ && !isExtended) {
      let charCodeArray: number[];
      if (isMultiByte) {
        charCodeArray = [currentByte, nextByte];
        i++;
      } else {
        charCodeArray = [currentByte];
      }

      char = service.textDecoder_.decode(new Uint8Array(charCodeArray));
    } else {
      char = get708CharFromCode(extended | currentByte);
    }

    if (win.pendingNewLine && !win.isEmpty()) {
      win.newLine(this.getPts(i));
    }

    win.pendingNewLine = false;
    win.addText(char);

    return i;
  }

  /**
   * Handle decoding of multibyte character
   *
   * @param {number} offset Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public multiByteCharacter(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    const firstByte = packetData[i + 1];
    const secondByte = packetData[i + 2];
    if (within708TextBlock(firstByte) && within708TextBlock(secondByte)) {
      i = this.handleText(++i, service, {
        isMultiByte: true,
        isExtended: false,
      });
    } else {
      // Unknown command
    }
    return i;
  }

  /**
   * Parse and execute the CW# command.
   *
   * Set the current window.
   *
   * @param {number} i        Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public setCurrentWindow(i: number, service: Cea708Service): number {
    const packetData = this._current708Packet.data;
    const b = packetData[i];
    const windowNum = b & 0x07;
    service.setCurrentWindow(windowNum);
    return i;
  }

  /**
   * Parse and execute the DF# command.
   *
   * Define a window and set it as the current window.
   *
   * @param {number} offset Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public defineWindow(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    let b = packetData[i];
    const windowNum = b & 0x07;
    service.setCurrentWindow(windowNum);
    const win = service.currentWindow;

    b = packetData[++i];
    win.visible = (b & 0x20) >> 5; // v
    win.rowLock = (b & 0x10) >> 4; // rl
    win.columnLock = (b & 0x08) >> 3; // cl
    win.priority = b & 0x07; // p

    b = packetData[++i];
    win.relativePositioning = (b & 0x80) >> 7; // rp
    win.anchorVertical = b & 0x7f; // av

    b = packetData[++i];
    win.anchorHorizontal = b; // ah

    b = packetData[++i];
    win.anchorPoint = (b & 0xf0) >> 4; // ap
    win.rowCount = b & 0x0f; // rc

    b = packetData[++i];
    win.columnCount = b & 0x3f; // cc

    b = packetData[++i];
    win.windowStyle = (b & 0x38) >> 3; // ws
    win.penStyle = b & 0x07; // ps

    // The spec says there are (rowCount+1) "virtual rows"
    win.virtualRowCount = win.rowCount + 1;

    return i;
  }

  /**
   * Parse and execute the SWA command.
   *
   * Set attributes of the current window.
   *
   * @param {number} offset Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public setWindowAttributes(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    let b = packetData[i];
    const winAttr = service.currentWindow.winAttr;

    b = packetData[++i];
    winAttr.fillOpacity = (b & 0xc0) >> 6; // fo
    winAttr.fillRed = (b & 0x30) >> 4; // fr
    winAttr.fillGreen = (b & 0x0c) >> 2; // fg
    winAttr.fillBlue = b & 0x03; // fb

    b = packetData[++i];
    winAttr.borderType = (b & 0xc0) >> 6; // bt
    winAttr.borderRed = (b & 0x30) >> 4; // br
    winAttr.borderGreen = (b & 0x0c) >> 2; // bg
    winAttr.borderBlue = b & 0x03; // bb

    b = packetData[++i];
    winAttr.borderType += (b & 0x80) >> 5; // bt
    winAttr.wordWrap = (b & 0x40) >> 6; // ww
    winAttr.printDirection = (b & 0x30) >> 4; // pd
    winAttr.scrollDirection = (b & 0x0c) >> 2; // sd
    winAttr.justify = b & 0x03; // j

    b = packetData[++i];
    winAttr.effectSpeed = (b & 0xf0) >> 4; // es
    winAttr.effectDirection = (b & 0x0c) >> 2; // ed
    winAttr.displayEffect = b & 0x03; // de

    return i;
  }

  /**
   * Gather text from all displayed windows and push a caption to output.
   */
  public flushDisplayed(pts: number, service: Cea708Service): void {
    const displayedText: string[] = [];

    // TODO: Positioning not supported, displaying multiple windows will not
    // necessarily display text in the correct order, but sample files so far
    // have not shown any issue.
    for (let winId = 0; winId < 8; winId++) {
      if (service.windows[winId].visible && !service.windows[winId].isEmpty()) {
        displayedText.push(service.windows[winId].getText());
      }
    }
    service.endPts = pts;
    service.text = displayedText.join("\n\n");
    this.pushCaption(service);
    service.startPts = pts;
  }

  /**
   * Push a caption to output if the caption contains text.
   *
   * @param {Service} service  The service object to be affected
   */
  public pushCaption(service: Cea708Service): void {
    if (service.text !== "") {
      this.trigger("data", {
        startPts: service.startPts,
        endPts: service.endPts,
        text: service.text,
        stream: "cc708_" + service.serviceNum,
      });

      service.text = "";
      service.startPts = service.endPts;
    }
  }

  /**
   * Parse and execute the DSW command.
   *
   * Set visible property of windows based on the parsed bitmask.
   *
   * @param {number} offset Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public displayWindows(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    const b = packetData[++i];
    const pts = this.getPts(i);

    this.flushDisplayed(pts, service);

    for (let winId = 0; winId < 8; winId++) {
      if (b & (0x01 << winId)) {
        service.windows[winId].visible = 1;
      }
    }

    return i;
  }

  /**
   * Parse and execute the HDW command.
   *
   * Set visible property of windows based on the parsed bitmask.
   *
   * @param {number} offset - Current index in the 708 packet
   * @param {Service} service - The service object to be affected
   * @return {number} - New index after parsing
   */
  public hideWindows(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    const b = packetData[++i];
    const pts = this.getPts(i);
    this.flushDisplayed(pts, service);
    for (let winId = 0; winId < 8; winId++) {
      if (b & (0x01 << winId)) {
        service.windows[winId].visible = 0;
      }
    }
    return i;
  }

  /**
   * Parse and execute the TGW command.
   *
   * Set visible property of windows based on the parsed bitmask.
   *
   * @param {number} offset - Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public toggleWindows(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    const b = packetData[++i];
    const pts = this.getPts(i);
    this.flushDisplayed(pts, service);
    for (let winId = 0; winId < 8; winId++) {
      if (b & (0x01 << winId)) {
        service.windows[winId].visible ^= 1;
      }
    }

    return i;
  }

  /**
   * Parse and execute the CLW command.
   *
   * Clear text of windows based on the parsed bitmask.
   *
   * @param {number} offset - Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public clearWindows(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    const b = packetData[++i];
    const pts = this.getPts(i);
    this.flushDisplayed(pts, service);

    for (let winId = 0; winId < 8; winId++) {
      if (b & (0x01 << winId)) {
        service.windows[winId].clearText();
      }
    }

    return i;
  }

  /**
   * Parse and execute the DLW command.
   *
   * Re-initialize windows based on the parsed bitmask.
   *
   * @param {number} offset Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public deleteWindows(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    const b = packetData[++i];
    const pts = this.getPts(i);
    this.flushDisplayed(pts, service);
    for (let winId = 0; winId < 8; winId++) {
      if (b & (0x01 << winId)) {
        service.windows[winId].reset();
      }
    }
    return i;
  }

  /**
   * Parse and execute the SPA command.
   *
   * Set pen attributes of the current window.
   *
   * @param {number} offset Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public setPenAttributes(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    let b = packetData[i];
    const penAttr = service.currentWindow.penAttr;

    b = packetData[++i];
    penAttr.textTag = (b & 0xf0) >> 4; // tt
    penAttr.offset = (b & 0x0c) >> 2; // o
    penAttr.penSize = b & 0x03; // s

    b = packetData[++i];
    penAttr.italics = (b & 0x80) >> 7; // i
    penAttr.underline = (b & 0x40) >> 6; // u
    penAttr.edgeType = (b & 0x38) >> 3; // et
    penAttr.fontStyle = b & 0x07; // fs

    return i;
  }

  /**
   * Parse and execute the SPC command.
   *
   * Set pen color of the current window.
   *
   * @param {number} offset - Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public setPenColor(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    let b = packetData[i];
    const penColor = service.currentWindow.penColor;

    b = packetData[++i];
    penColor.fgOpacity = (b & 0xc0) >> 6; // fo
    penColor.fgRed = (b & 0x30) >> 4; // fr
    penColor.fgGreen = (b & 0x0c) >> 2; // fg
    penColor.fgBlue = b & 0x03; // fb

    b = packetData[++i];
    penColor.bgOpacity = (b & 0xc0) >> 6; // bo
    penColor.bgRed = (b & 0x30) >> 4; // br
    penColor.bgGreen = (b & 0x0c) >> 2; // bg
    penColor.bgBlue = b & 0x03; // bb

    b = packetData[++i];
    penColor.edgeRed = (b & 0x30) >> 4; // er
    penColor.edgeGreen = (b & 0x0c) >> 2; // eg
    penColor.edgeBlue = b & 0x03; // eb

    return i;
  }

  /**
   * Parse and execute the SPL command.
   *
   * Set pen location of the current window.
   *
   * @param {number} offset - Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {number}          New index after parsing
   */
  public setPenLocation(offset: number, service: Cea708Service): number {
    let i = offset;
    const packetData = this._current708Packet.data;
    let b = packetData[i];
    const penLoc = service.currentWindow.penLoc;

    // Positioning isn't really supported at the moment, so this essentially
    // just inserts a linebreak
    service.currentWindow.pendingNewLine = true;

    b = packetData[++i];
    penLoc.row = b & 0x0f; // r

    b = packetData[++i];
    penLoc.column = b & 0x3f; // c

    return i;
  }

  /**
   * Execute the RST command.
   *
   * Reset service to a clean slate. Re-initialize.
   *
   * @param {number} i        Current index in the 708 packet
   * @param {Service} service  The service object to be affected
   * @return {Service}          Re-initialized service
   */
  public reset(i: number, service: Cea708Service): Cea708Service {
    const pts = this.getPts(i);
    this.flushDisplayed(pts, service);
    return this.initService(service.serviceNum, i);
  }
}

// This hash maps non-ASCII, special, and extended character codes to their
// proper Unicode equivalent. The first keys that are only a single byte
// are the non-standard ASCII characters, which simply map the CEA608 byte
// to the standard ASCII/Unicode. The two-byte keys that follow are the CEA608
// character codes, but have their MSB bitmasked with 0x03 so that a lookup
// can be performed regardless of the field and data channel on which the
// character code was received.
/* eslint-disable @typescript-eslint/naming-convention */
const CHARACTER_TRANSLATION = {
  0x2a: 0xe1, // á
  0x5c: 0xe9, // é
  0x5e: 0xed, // í
  0x5f: 0xf3, // ó
  0x60: 0xfa, // ú
  0x7b: 0xe7, // ç
  0x7c: 0xf7, // ÷
  0x7d: 0xd1, // Ñ
  0x7e: 0xf1, // ñ
  0x7f: 0x2588, // █
  0x0130: 0xae, // ®
  0x0131: 0xb0, // °
  0x0132: 0xbd, // ½
  0x0133: 0xbf, // ¿
  0x0134: 0x2122, // ™
  0x0135: 0xa2, // ¢
  0x0136: 0xa3, // £
  0x0137: 0x266a, // ♪
  0x0138: 0xe0, // à
  0x0139: 0xa0, //
  0x013a: 0xe8, // è
  0x013b: 0xe2, // â
  0x013c: 0xea, // ê
  0x013d: 0xee, // î
  0x013e: 0xf4, // ô
  0x013f: 0xfb, // û
  0x0220: 0xc1, // Á
  0x0221: 0xc9, // É
  0x0222: 0xd3, // Ó
  0x0223: 0xda, // Ú
  0x0224: 0xdc, // Ü
  0x0225: 0xfc, // ü
  0x0226: 0x2018, // ‘
  0x0227: 0xa1, // ¡
  0x0228: 0x2a, // *
  0x0229: 0x27, // '
  0x022a: 0x2014, // —
  0x022b: 0xa9, // ©
  0x022c: 0x2120, // ℠
  0x022d: 0x2022, // •
  0x022e: 0x201c, // “
  0x022f: 0x201d, // ”
  0x0230: 0xc0, // À
  0x0231: 0xc2, // Â
  0x0232: 0xc7, // Ç
  0x0233: 0xc8, // È
  0x0234: 0xca, // Ê
  0x0235: 0xcb, // Ë
  0x0236: 0xeb, // ë
  0x0237: 0xce, // Î
  0x0238: 0xcf, // Ï
  0x0239: 0xef, // ï
  0x023a: 0xd4, // Ô
  0x023b: 0xd9, // Ù
  0x023c: 0xf9, // ù
  0x023d: 0xdb, // Û
  0x023e: 0xab, // «
  0x023f: 0xbb, // »
  0x0320: 0xc3, // Ã
  0x0321: 0xe3, // ã
  0x0322: 0xcd, // Í
  0x0323: 0xcc, // Ì
  0x0324: 0xec, // ì
  0x0325: 0xd2, // Ò
  0x0326: 0xf2, // ò
  0x0327: 0xd5, // Õ
  0x0328: 0xf5, // õ
  0x0329: 0x7b, // {
  0x032a: 0x7d, // }
  0x032b: 0x5c, // \
  0x032c: 0x5e, // ^
  0x032d: 0x5f, // _
  0x032e: 0x7c, // |
  0x032f: 0x7e, // ~
  0x0330: 0xc4, // Ä
  0x0331: 0xe4, // ä
  0x0332: 0xd6, // Ö
  0x0333: 0xf6, // ö
  0x0334: 0xdf, // ß
  0x0335: 0xa5, // ¥
  0x0336: 0xa4, // ¤
  0x0337: 0x2502, // │
  0x0338: 0xc5, // Å
  0x0339: 0xe5, // å
  0x033a: 0xd8, // Ø
  0x033b: 0xf8, // ø
  0x033c: 0x250c, // ┌
  0x033d: 0x2510, // ┐
  0x033e: 0x2514, // └
  0x033f: 0x2518, // ┘
};
/* eslint-enable @typescript-eslint/naming-convention */

function getCharFromCode(code: number | null): string {
  if (code === null) {
    return "";
  }
  let nonNullCode: number = code;
  nonNullCode = CHARACTER_TRANSLATION[code] ?? nonNullCode;
  return String.fromCharCode(nonNullCode);
}

// the index of the last row in a CEA-608 display buffer
const BOTTOM_ROW = 14;

// This array is used for mapping PACs -> row #, since there's no way of
// getting it through bit logic.
const ROWS = [
  0x1100, 0x1120, 0x1200, 0x1220, 0x1500, 0x1520, 0x1600, 0x1620, 0x1700,
  0x1720, 0x1000, 0x1300, 0x1320, 0x1400, 0x1420,
];

// CEA-608 captions are rendered onto a 34x15 matrix of character
// cells. The "bottom" row is the last element in the outer array.
function createDisplayBuffer(): string[] {
  const result: string[] = [];
  let i = BOTTOM_ROW + 1;
  while (i--) {
    result.push("");
  }
  return result;
}

export interface Cea608StreamEvents {
  data: any;
  done: null;
  partialdone: null;
  reset: null;
  endedtimeline: null;
}

class Cea608Stream extends EventEmitter<Cea608StreamEvents> {
  private field_: number;
  private dataChannel_: number;
  private name_: string;
  private lastControlCode_: number | null;
  private BASE_: number;
  private EXT_: number;
  private CONTROL_: number;
  private OFFSET_: number;
  private RESUME_CAPTION_LOADING_: number;
  private END_OF_CAPTION_: number;
  private ROLL_UP_2_ROWS_: number;
  private ROLL_UP_3_ROWS_: number;
  private ROLL_UP_4_ROWS_: number;
  private CARRIAGE_RETURN_: number;
  private RESUME_DIRECT_CAPTIONING_: number;
  private BACKSPACE_: number;
  private ERASE_DISPLAYED_MEMORY_: number;
  private ERASE_NON_DISPLAYED_MEMORY_: number;
  private mode_: string;
  private topRow_: number;
  private startPts_: number;
  private displayed_: string[];
  private nonDisplayed_: string[];
  private row_: number;
  private rollUpRows_: number;
  private formatting_: string[];

  constructor(field: number | undefined, dataChannel: number | undefined) {
    super();
    this.field_ = field ?? 0;
    this.dataChannel_ = dataChannel ?? 0;
    this.name_ = "CC" + (((this.field_ << 1) | this.dataChannel_) + 1);
    this.lastControlCode_ = null;

    // The following attributes have these uses:
    // ext_ :    char0 for mid-row codes, and the base for extended
    //           chars (ext_+0, ext_+1, and ext_+2 are char0s for
    //           extended codes)
    // control_: char0 for control codes, except byte-shifted to the
    //           left so that we can do this.control_ | CONTROL_CODE
    // offset_:  char0 for tab offset codes
    //
    // It's also worth noting that control codes, and _only_ control codes,
    // differ between field 1 and field2. Field 2 control codes are always
    // their field 1 value plus 1. That's why there's the "| field" on the
    // control value.
    if (this.dataChannel_ === 0) {
      this.BASE_ = 0x10;
      this.EXT_ = 0x11;
      this.CONTROL_ = (0x14 | this.field_) << 8;
      this.OFFSET_ = 0x17;
    } else if (this.dataChannel_ === 1) {
      this.BASE_ = 0x18;
      this.EXT_ = 0x19;
      this.CONTROL_ = (0x1c | this.field_) << 8;
      this.OFFSET_ = 0x1f;
    } else {
      // TODO Check REALLY what to do here
      this.BASE_ = 0x10;
      this.EXT_ = 0x11;
      this.CONTROL_ = (0x14 | this.field_) << 8;
      this.OFFSET_ = 0x17;
    }

    // Constants for the LSByte command codes recognized by Cea608Stream. This
    // list is not exhaustive. For a more comprehensive listing and semantics see
    // eslint-disable-next-line max-len
    // http://www.gpo.gov/fdsys/pkg/CFR-2010-title47-vol1/pdf/CFR-2010-title47-vol1-sec15-119.pdf
    // Pop-on Mode
    this.RESUME_CAPTION_LOADING_ = this.CONTROL_ | 0x20;
    this.END_OF_CAPTION_ = this.CONTROL_ | 0x2f;
    // Roll-up Mode
    this.ROLL_UP_2_ROWS_ = this.CONTROL_ | 0x25;
    this.ROLL_UP_3_ROWS_ = this.CONTROL_ | 0x26;
    this.ROLL_UP_4_ROWS_ = this.CONTROL_ | 0x27;
    this.CARRIAGE_RETURN_ = this.CONTROL_ | 0x2d;
    // paint-on mode
    this.RESUME_DIRECT_CAPTIONING_ = this.CONTROL_ | 0x29;
    // Erasure
    this.BACKSPACE_ = this.CONTROL_ | 0x21;
    this.ERASE_DISPLAYED_MEMORY_ = this.CONTROL_ | 0x2c;
    this.ERASE_NON_DISPLAYED_MEMORY_ = this.CONTROL_ | 0x2e;
    this.mode_ = "popOn";
    this.topRow_ = 0;
    this.startPts_ = 0;
    this.displayed_ = createDisplayBuffer();
    this.nonDisplayed_ = createDisplayBuffer();
    this.lastControlCode_ = null;
    this.row_ = BOTTOM_ROW;
    this.rollUpRows_ = 2;
    this.formatting_ = [];
  }

  public push(packet: CaptionPacket): void {
    // remove the parity bits
    const data = packet.ccData & 0x7f7f;

    // ignore duplicate control codes; the spec demands they're sent twice
    if (data === this.lastControlCode_) {
      this.lastControlCode_ = null;
      return;
    }

    // Store control codes
    if ((data & 0xf000) === 0x1000) {
      this.lastControlCode_ = data;
    } else if (data !== PADDING) {
      this.lastControlCode_ = null;
    }

    let char0 = data >>> 8;
    let char1: number | null = data & 0xff;

    if (data === PADDING) {
      return;
    } else if (data === this.RESUME_CAPTION_LOADING_) {
      this.mode_ = "popOn";
    } else if (data === this.END_OF_CAPTION_) {
      // If an EOC is received while in paint-on mode, the displayed caption
      // text should be swapped to non-displayed memory as if it was a pop-on
      // caption. Because of that, we should explicitly switch back to pop-on
      // mode
      this.mode_ = "popOn";
      this.clearFormatting(packet.pts);
      // if a caption was being displayed, it's gone now
      this.flushDisplayed(packet.pts);

      // flip memory
      const swap = this.displayed_;
      this.displayed_ = this.nonDisplayed_;
      this.nonDisplayed_ = swap;

      // start measuring the time to display the caption
      this.startPts_ = packet.pts;
    } else if (data === this.ROLL_UP_2_ROWS_) {
      this.rollUpRows_ = 2;
      this.setRollUp(packet.pts);
    } else if (data === this.ROLL_UP_3_ROWS_) {
      this.rollUpRows_ = 3;
      this.setRollUp(packet.pts);
    } else if (data === this.ROLL_UP_4_ROWS_) {
      this.rollUpRows_ = 4;
      this.setRollUp(packet.pts);
    } else if (data === this.CARRIAGE_RETURN_) {
      this.clearFormatting(packet.pts);
      this.flushDisplayed(packet.pts);
      this.shiftRowsUp_();
      this.startPts_ = packet.pts;
    } else if (data === this.BACKSPACE_) {
      if (this.mode_ === "popOn") {
        this.nonDisplayed_[this.row_] = this.nonDisplayed_[this.row_].slice(
          0,
          -1
        );
      } else {
        this.displayed_[this.row_] = this.displayed_[this.row_].slice(0, -1);
      }
    } else if (data === this.ERASE_DISPLAYED_MEMORY_) {
      this.flushDisplayed(packet.pts);
      this.displayed_ = createDisplayBuffer();
    } else if (data === this.ERASE_NON_DISPLAYED_MEMORY_) {
      this.nonDisplayed_ = createDisplayBuffer();
    } else if (data === this.RESUME_DIRECT_CAPTIONING_) {
      if (this.mode_ !== "paintOn") {
        // NOTE: This should be removed when proper caption positioning is
        // implemented
        this.flushDisplayed(packet.pts);
        this.displayed_ = createDisplayBuffer();
      }
      this.mode_ = "paintOn";
      this.startPts_ = packet.pts;

      // Append special characters to caption text
    } else if (this.isSpecialCharacter(char0, char1)) {
      // Bitmask char0 so that we can apply character transformations
      // regardless of field and data channel.
      // Then byte-shift to the left and OR with char1 so we can pass the
      // entire character code to `getCharFromCode`.
      char0 = (char0 & 0x03) << 8;
      const text = getCharFromCode(char0 | char1);
      this[this.mode_](packet.pts, text);

      // Append extended characters to caption text
    } else if (this.isExtCharacter(char0, char1)) {
      // Extended characters always follow their "non-extended" equivalents.
      // IE if a "è" is desired, you'll always receive "eè"; non-compliant
      // decoders are supposed to drop the "è", while compliant decoders
      // backspace the "e" and insert "è".

      // Delete the previous character
      if (this.mode_ === "popOn") {
        this.nonDisplayed_[this.row_] = this.nonDisplayed_[this.row_].slice(
          0,
          -1
        );
      } else {
        this.displayed_[this.row_] = this.displayed_[this.row_].slice(0, -1);
      }

      // Bitmask char0 so that we can apply character transformations
      // regardless of field and data channel.
      // Then byte-shift to the left and OR with char1 so we can pass the
      // entire character code to `getCharFromCode`.
      char0 = (char0 & 0x03) << 8;
      const text = getCharFromCode(char0 | char1);
      this[this.mode_](packet.pts, text);

      // Process mid-row codes
    } else if (this.isMidRowCode(char0, char1)) {
      // Attributes are not additive, so clear all formatting
      this.clearFormatting(packet.pts);

      // According to the standard, mid-row codes
      // should be replaced with spaces, so add one now
      this[this.mode_](packet.pts, " ");

      if ((char1 & 0xe) === 0xe) {
        this.addFormatting(packet.pts, ["i"]);
      }

      if ((char1 & 0x1) === 0x1) {
        this.addFormatting(packet.pts, ["u"]);
      }

      // Detect offset control codes and adjust cursor
    } else if (this.isOffsetControlCode(char0, char1)) {
      // Cursor position is set by indent PAC (see below) in 4-column
      // increments, with an additional offset code of 1-3 to reach any
      // of the 32 columns specified by CEA-608. So all we need to do
      // here is increment the column cursor by the given offset.
      // Detect PACs (Preamble Address Codes)
    } else if (this.isPAC(char0, char1)) {
      // There's no logic for PAC -> row mapping, so we have to just
      // find the row code in an array and use its index :(
      let row = ROWS.indexOf(data & 0x1f20);

      // Configure the caption window if we're in roll-up mode
      if (this.mode_ === "rollUp") {
        // This implies that the base row is incorrectly set.
        // As per the recommendation in CEA-608(Base Row Implementation), defer
        // to the number of roll-up rows set.
        if (row - this.rollUpRows_ + 1 < 0) {
          row = this.rollUpRows_ - 1;
        }

        this.setRollUp(packet.pts, row);
      }

      if (row !== this.row_) {
        // formatting is only persistent for current row
        this.clearFormatting(packet.pts);
        this.row_ = row;
      }
      // All PACs can apply underline, so detect and apply
      // (All odd-numbered second bytes set underline)
      if (char1 & 0x1 && this.formatting_.indexOf("u") === -1) {
        this.addFormatting(packet.pts, ["u"]);
      }

      if ((data & 0x10) === 0x10) {
        // We've got an indent level code. Each successive even number
        // increments the column cursor by 4, so we can get the desired
        // column position by bit-shifting to the right (to get n/2)
        // and multiplying by 4.
      }

      if (this.isColorPAC(char1)) {
        // it's a color code, though we only support white, which
        // can be either normal or italicized. white italics can be
        // either 0x4e or 0x6e depending on the row, so we just
        // bitwise-and with 0xe to see if italics should be turned on
        if ((char1 & 0xe) === 0xe) {
          this.addFormatting(packet.pts, ["i"]);
        }
      }

      // We have a normal character in char0, and possibly one in char1
    } else if (this.isNormalChar(char0)) {
      if (char1 === 0x00) {
        char1 = null;
      }
      let text = getCharFromCode(char0);
      text += getCharFromCode(char1);
      this[this.mode_](packet.pts, text);
    } // finish data processing
  }

  public flush() {
    this.trigger("done", null);
  }

  public partialFlush() {
    return this.trigger("partialdone", null);
  }

  /**
   * Zero out the data, used for startup and on seek
   */
  public reset(): void {
    this.mode_ = "popOn";
    // When in roll-up mode, the index of the last row that will
    // actually display captions. If a caption is shifted to a row
    // with a lower index than this, it is cleared from the display
    // buffer
    this.topRow_ = 0;
    this.startPts_ = 0;
    this.displayed_ = createDisplayBuffer();
    this.nonDisplayed_ = createDisplayBuffer();
    this.lastControlCode_ = null;

    // Track row and column for proper line-breaking and spacing
    this.row_ = BOTTOM_ROW;
    this.rollUpRows_ = 2;

    // This variable holds currently-applied formatting
    this.formatting_ = [];
  }

  // Trigger a cue point that captures the current state of the
  // display buffer
  public flushDisplayed(pts: number): void {
    const content = this.displayed_
      // remove spaces from the start and end of the string
      .map(function (row, index) {
        try {
          return row.trim();
        } catch (e) {
          // Ordinarily, this shouldn't happen. However, caption
          // parsing errors should not throw exceptions and
          // break playback.
          this.trigger("log", {
            level: "warn",
            message: "Skipping a malformed 608 caption at index " + index + ".",
          });
          return "";
        }
      }, this)
      // combine all text rows to display in one cue
      .join("\n")
      // and remove blank rows from the start and end, but not the middle
      .replace(/^\n+|\n+$/g, "");

    if (content.length) {
      this.trigger("data", {
        startPts: this.startPts_,
        endPts: pts,
        text: content,
        stream: this.name_,
      });
    }
  }

  /**
   * Detects if the 2-byte packet data is a special character
   *
   * Special characters have a second byte in the range 0x30 to 0x3f,
   * with the first byte being 0x11 (for data channel 1) or 0x19 (for
   * data channel 2).
   *
   * @param {number} char0 The first byte
   * @param {number} char1 The second byte
   * @return {Boolean}       Whether the 2 bytes are an special character
   */
  public isSpecialCharacter(char0: number, char1: number): boolean {
    return char0 === this.EXT_ && char1 >= 0x30 && char1 <= 0x3f;
  }

  /**
   * Detects if the 2-byte packet data is an extended character
   *
   * Extended characters have a second byte in the range 0x20 to 0x3f,
   * with the first byte being 0x12 or 0x13 (for data channel 1) or
   * 0x1a or 0x1b (for data channel 2).
   *
   * @param {number} char0 The first byte
   * @param {number} char1 The second byte
   * @return {Boolean}       Whether the 2 bytes are an extended character
   */
  public isExtCharacter(char0: number, char1: number): boolean {
    return (
      (char0 === this.EXT_ + 1 || char0 === this.EXT_ + 2) &&
      char1 >= 0x20 &&
      char1 <= 0x3f
    );
  }

  /**
   * Detects if the 2-byte packet is a mid-row code
   *
   * Mid-row codes have a second byte in the range 0x20 to 0x2f, with
   * the first byte being 0x11 (for data channel 1) or 0x19 (for data
   * channel 2).
   *
   * @param {number} char0 The first byte
   * @param {number} char1 The second byte
   * @return {Boolean}       Whether the 2 bytes are a mid-row code
   */
  public isMidRowCode(char0: number, char1: number): boolean {
    return char0 === this.EXT_ && char1 >= 0x20 && char1 <= 0x2f;
  }

  /**
   * Detects if the 2-byte packet is an offset control code
   *
   * Offset control codes have a second byte in the range 0x21 to 0x23,
   * with the first byte being 0x17 (for data channel 1) or 0x1f (for
   * data channel 2).
   *
   * @param {number} char0 The first byte
   * @param {number} char1 The second byte
   * @return {Boolean}       Whether the 2 bytes are an offset control code
   */
  public isOffsetControlCode(char0: number, char1: number): boolean {
    return char0 === this.OFFSET_ && char1 >= 0x21 && char1 <= 0x23;
  }

  /**
   * Detects if the 2-byte packet is a Preamble Address Code
   *
   * PACs have a first byte in the range 0x10 to 0x17 (for data channel 1)
   * or 0x18 to 0x1f (for data channel 2), with the second byte in the
   * range 0x40 to 0x7f.
   *
   * @param {number} char0 The first byte
   * @param {number} char1 The second byte
   * @return {Boolean}       Whether the 2 bytes are a PAC
   */
  public isPAC(char0: number, char1: number): boolean {
    return (
      char0 >= this.BASE_ &&
      char0 < this.BASE_ + 8 &&
      char1 >= 0x40 &&
      char1 <= 0x7f
    );
  }

  /**
   * Detects if a packet's second byte is in the range of a PAC color code
   *
   * PAC color codes have the second byte be in the range 0x40 to 0x4f, or
   * 0x60 to 0x6f.
   *
   * @param {number} char1 The second byte
   * @return {Boolean}       Whether the byte is a color PAC
   */
  public isColorPAC(char1: number): boolean {
    return (char1 >= 0x40 && char1 <= 0x4f) || (char1 >= 0x60 && char1 <= 0x7f);
  }

  /**
   * Detects if a single byte is in the range of a normal character
   *
   * Normal text bytes are in the range 0x20 to 0x7f.
   *
   * @param {number} char  The byte
   * @return {Boolean}       Whether the byte is a normal character
   */
  public isNormalChar(char: number): boolean {
    return char >= 0x20 && char <= 0x7f;
  }

  /**
   * Configures roll-up
   *
   * @param {number} pts         Current PTS
   * @param {number} baseRowParam  Used by PACs to slide the current window to
   *                               a new position
   */
  public setRollUp(pts: number, baseRowParam?: number): void {
    let newBaseRow = baseRowParam;

    // Reset the base row to the bottom row when switching modes
    if (this.mode_ !== "rollUp") {
      this.row_ = BOTTOM_ROW;
      this.mode_ = "rollUp";
      // Spec says to wipe memories when switching to roll-up
      this.flushDisplayed(pts);
      this.nonDisplayed_ = createDisplayBuffer();
      this.displayed_ = createDisplayBuffer();
    }

    if (newBaseRow !== undefined && newBaseRow !== this.row_) {
      // move currently displayed captions (up or down) to the new base row
      for (let i = 0; i < this.rollUpRows_; i++) {
        this.displayed_[newBaseRow - i] = this.displayed_[this.row_ - i];
        this.displayed_[this.row_ - i] = "";
      }
    }

    if (newBaseRow === undefined) {
      newBaseRow = this.row_;
    }

    this.topRow_ = newBaseRow - this.rollUpRows_ + 1;
  }

  // Adds the opening HTML tag for the passed character to the caption text,
  // and keeps track of it for later closing
  public addFormatting(pts: number, format: string[]): void {
    this.formatting_ = this.formatting_.concat(format);
    const text = format.reduce(function (txt, fmt) {
      return txt + "<" + fmt + ">";
    }, "");
    this[this.mode_](pts, text);
  }

  // Adds HTML closing tags for current formatting to caption text and
  // clears remembered formatting
  public clearFormatting(pts: number): void {
    if (!this.formatting_.length) {
      return;
    }
    const text = this.formatting_.reverse().reduce(function (txt, format) {
      return txt + "</" + format + ">";
    }, "");
    this.formatting_ = [];
    this[this.mode_](pts, text);
  }

  // Mode Implementations
  public popOn(_pts: number, text: string): void {
    let baseRow = this.nonDisplayed_[this.row_];

    // buffer characters
    baseRow += text;
    this.nonDisplayed_[this.row_] = baseRow;
  }

  public rollUp(_pts: number, text: string): void {
    let baseRow = this.displayed_[this.row_];
    baseRow += text;
    this.displayed_[this.row_] = baseRow;
  }

  public shiftRowsUp_(): void {
    // clear out inactive rows
    for (let i = 0; i < this.topRow_; i++) {
      this.displayed_[i] = "";
    }
    for (let i = this.row_ + 1; i < BOTTOM_ROW + 1; i++) {
      this.displayed_[i] = "";
    }
    // shift displayed rows up
    for (let i = this.topRow_; i < this.row_; i++) {
      this.displayed_[i] = this.displayed_[i + 1];
    }
    // clear out the bottom row
    this.displayed_[this.row_] = "";
  }

  public paintOn(_pts: number, text: string): void {
    let baseRow = this.displayed_[this.row_];
    baseRow += text;
    this.displayed_[this.row_] = baseRow;
  }
}

export default CaptionStream;
export { Cea608Stream, Cea708Stream };
