(() => {
  var __create = Object.create;
  var __defProp = Object.defineProperty;
  var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
  var __getOwnPropNames = Object.getOwnPropertyNames;
  var __getProtoOf = Object.getPrototypeOf;
  var __hasOwnProp = Object.prototype.hasOwnProperty;
  var __commonJS = (cb, mod) => function __require() {
    return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
  };
  var __copyProps = (to, from, except, desc) => {
    if (from && typeof from === "object" || typeof from === "function") {
      for (let key of __getOwnPropNames(from))
        if (!__hasOwnProp.call(to, key) && key !== except)
          __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
    }
    return to;
  };
  var __toESM = (mod, isNodeMode, target) => (target = mod != null ? __create(__getProtoOf(mod)) : {}, __copyProps(isNodeMode || !mod || !mod.__esModule ? __defProp(target, "default", { value: mod, enumerable: true }) : target, mod));

  // node_modules/global/window.js
  var require_window = __commonJS({
    "node_modules/global/window.js"(exports, module) {
      var win;
      if (typeof window !== "undefined") {
        win = window;
      } else if (typeof global !== "undefined") {
        win = global;
      } else if (typeof self !== "undefined") {
        win = self;
      } else {
        win = {};
      }
      module.exports = win;
    }
  });

  // node_modules/mux.js/dist/mux.js
  var require_mux = __commonJS({
    "node_modules/mux.js/dist/mux.js"(exports, module) {
      (function(global3, factory) {
        typeof exports === "object" && typeof module !== "undefined" ? module.exports = factory(require_window()) : typeof define === "function" && define.amd ? define(["global/window"], factory) : (global3 = typeof globalThis !== "undefined" ? globalThis : global3 || self, global3.muxjs = factory(global3.window));
      })(exports, function(window2) {
        "use strict";
        function _interopDefaultLegacy(e) {
          return e && typeof e === "object" && "default" in e ? e : { "default": e };
        }
        var window__default = /* @__PURE__ */ _interopDefaultLegacy(window2);
        var Stream = function Stream2() {
          this.init = function() {
            var listeners = {};
            this.on = function(type2, listener) {
              if (!listeners[type2]) {
                listeners[type2] = [];
              }
              listeners[type2] = listeners[type2].concat(listener);
            };
            this.off = function(type2, listener) {
              var index;
              if (!listeners[type2]) {
                return false;
              }
              index = listeners[type2].indexOf(listener);
              listeners[type2] = listeners[type2].slice();
              listeners[type2].splice(index, 1);
              return index > -1;
            };
            this.trigger = function(type2) {
              var callbacks, i, length, args;
              callbacks = listeners[type2];
              if (!callbacks) {
                return;
              }
              if (arguments.length === 2) {
                length = callbacks.length;
                for (i = 0; i < length; ++i) {
                  callbacks[i].call(this, arguments[1]);
                }
              } else {
                args = [];
                i = arguments.length;
                for (i = 1; i < arguments.length; ++i) {
                  args.push(arguments[i]);
                }
                length = callbacks.length;
                for (i = 0; i < length; ++i) {
                  callbacks[i].apply(this, args);
                }
              }
            };
            this.dispose = function() {
              listeners = {};
            };
          };
        };
        Stream.prototype.pipe = function(destination) {
          this.on("data", function(data) {
            destination.push(data);
          });
          this.on("done", function(flushSource) {
            destination.flush(flushSource);
          });
          this.on("partialdone", function(flushSource) {
            destination.partialFlush(flushSource);
          });
          this.on("endedtimeline", function(flushSource) {
            destination.endTimeline(flushSource);
          });
          this.on("reset", function(flushSource) {
            destination.reset(flushSource);
          });
          return destination;
        };
        Stream.prototype.push = function(data) {
          this.trigger("data", data);
        };
        Stream.prototype.flush = function(flushSource) {
          this.trigger("done", flushSource);
        };
        Stream.prototype.partialFlush = function(flushSource) {
          this.trigger("partialdone", flushSource);
        };
        Stream.prototype.endTimeline = function(flushSource) {
          this.trigger("endedtimeline", flushSource);
        };
        Stream.prototype.reset = function(flushSource) {
          this.trigger("reset", flushSource);
        };
        var stream = Stream;
        var ONE_SECOND_IN_TS$5 = 9e4, secondsToVideoTs, secondsToAudioTs, videoTsToSeconds, audioTsToSeconds, audioTsToVideoTs, videoTsToAudioTs, metadataTsToSeconds;
        secondsToVideoTs = function secondsToVideoTs2(seconds) {
          return seconds * ONE_SECOND_IN_TS$5;
        };
        secondsToAudioTs = function secondsToAudioTs2(seconds, sampleRate) {
          return seconds * sampleRate;
        };
        videoTsToSeconds = function videoTsToSeconds2(timestamp) {
          return timestamp / ONE_SECOND_IN_TS$5;
        };
        audioTsToSeconds = function audioTsToSeconds2(timestamp, sampleRate) {
          return timestamp / sampleRate;
        };
        audioTsToVideoTs = function audioTsToVideoTs2(timestamp, sampleRate) {
          return secondsToVideoTs(audioTsToSeconds(timestamp, sampleRate));
        };
        videoTsToAudioTs = function videoTsToAudioTs2(timestamp, sampleRate) {
          return secondsToAudioTs(videoTsToSeconds(timestamp), sampleRate);
        };
        metadataTsToSeconds = function metadataTsToSeconds2(timestamp, timelineStartPts, keepOriginalTimestamps) {
          return videoTsToSeconds(keepOriginalTimestamps ? timestamp : timestamp - timelineStartPts);
        };
        var clock = {
          ONE_SECOND_IN_TS: ONE_SECOND_IN_TS$5,
          secondsToVideoTs,
          secondsToAudioTs,
          videoTsToSeconds,
          audioTsToSeconds,
          audioTsToVideoTs,
          videoTsToAudioTs,
          metadataTsToSeconds
        };
        var ONE_SECOND_IN_TS$4 = clock.ONE_SECOND_IN_TS;
        var _AdtsStream;
        var ADTS_SAMPLING_FREQUENCIES$1 = [96e3, 88200, 64e3, 48e3, 44100, 32e3, 24e3, 22050, 16e3, 12e3, 11025, 8e3, 7350];
        _AdtsStream = function AdtsStream(handlePartialSegments) {
          var buffer, frameNum = 0;
          _AdtsStream.prototype.init.call(this);
          this.skipWarn_ = function(start, end) {
            this.trigger("log", {
              level: "warn",
              message: "adts skiping bytes " + start + " to " + end + " in frame " + frameNum + " outside syncword"
            });
          };
          this.push = function(packet) {
            var i = 0, frameLength, protectionSkipBytes, oldBuffer, sampleCount, adtsFrameDuration;
            if (!handlePartialSegments) {
              frameNum = 0;
            }
            if (packet.type !== "audio") {
              return;
            }
            if (buffer && buffer.length) {
              oldBuffer = buffer;
              buffer = new Uint8Array(oldBuffer.byteLength + packet.data.byteLength);
              buffer.set(oldBuffer);
              buffer.set(packet.data, oldBuffer.byteLength);
            } else {
              buffer = packet.data;
            }
            var skip;
            while (i + 7 < buffer.length) {
              if (buffer[i] !== 255 || (buffer[i + 1] & 246) !== 240) {
                if (typeof skip !== "number") {
                  skip = i;
                }
                i++;
                continue;
              }
              if (typeof skip === "number") {
                this.skipWarn_(skip, i);
                skip = null;
              }
              protectionSkipBytes = (~buffer[i + 1] & 1) * 2;
              frameLength = (buffer[i + 3] & 3) << 11 | buffer[i + 4] << 3 | (buffer[i + 5] & 224) >> 5;
              sampleCount = ((buffer[i + 6] & 3) + 1) * 1024;
              adtsFrameDuration = sampleCount * ONE_SECOND_IN_TS$4 / ADTS_SAMPLING_FREQUENCIES$1[(buffer[i + 2] & 60) >>> 2];
              if (buffer.byteLength - i < frameLength) {
                break;
              }
              this.trigger("data", {
                pts: packet.pts + frameNum * adtsFrameDuration,
                dts: packet.dts + frameNum * adtsFrameDuration,
                sampleCount,
                audioobjecttype: (buffer[i + 2] >>> 6 & 3) + 1,
                channelcount: (buffer[i + 2] & 1) << 2 | (buffer[i + 3] & 192) >>> 6,
                samplerate: ADTS_SAMPLING_FREQUENCIES$1[(buffer[i + 2] & 60) >>> 2],
                samplingfrequencyindex: (buffer[i + 2] & 60) >>> 2,
                samplesize: 16,
                data: buffer.subarray(i + 7 + protectionSkipBytes, i + frameLength)
              });
              frameNum++;
              i += frameLength;
            }
            if (typeof skip === "number") {
              this.skipWarn_(skip, i);
              skip = null;
            }
            buffer = buffer.subarray(i);
          };
          this.flush = function() {
            frameNum = 0;
            this.trigger("done");
          };
          this.reset = function() {
            buffer = void 0;
            this.trigger("reset");
          };
          this.endTimeline = function() {
            buffer = void 0;
            this.trigger("endedtimeline");
          };
        };
        _AdtsStream.prototype = new stream();
        var adts = _AdtsStream;
        var ExpGolomb;
        ExpGolomb = function ExpGolomb2(workingData) {
          var workingBytesAvailable = workingData.byteLength, workingWord = 0, workingBitsAvailable = 0;
          this.length = function() {
            return 8 * workingBytesAvailable;
          };
          this.bitsAvailable = function() {
            return 8 * workingBytesAvailable + workingBitsAvailable;
          };
          this.loadWord = function() {
            var position = workingData.byteLength - workingBytesAvailable, workingBytes = new Uint8Array(4), availableBytes = Math.min(4, workingBytesAvailable);
            if (availableBytes === 0) {
              throw new Error("no bytes available");
            }
            workingBytes.set(workingData.subarray(position, position + availableBytes));
            workingWord = new DataView(workingBytes.buffer).getUint32(0);
            workingBitsAvailable = availableBytes * 8;
            workingBytesAvailable -= availableBytes;
          };
          this.skipBits = function(count) {
            var skipBytes;
            if (workingBitsAvailable > count) {
              workingWord <<= count;
              workingBitsAvailable -= count;
            } else {
              count -= workingBitsAvailable;
              skipBytes = Math.floor(count / 8);
              count -= skipBytes * 8;
              workingBytesAvailable -= skipBytes;
              this.loadWord();
              workingWord <<= count;
              workingBitsAvailable -= count;
            }
          };
          this.readBits = function(size) {
            var bits = Math.min(workingBitsAvailable, size), valu = workingWord >>> 32 - bits;
            workingBitsAvailable -= bits;
            if (workingBitsAvailable > 0) {
              workingWord <<= bits;
            } else if (workingBytesAvailable > 0) {
              this.loadWord();
            }
            bits = size - bits;
            if (bits > 0) {
              return valu << bits | this.readBits(bits);
            }
            return valu;
          };
          this.skipLeadingZeros = function() {
            var leadingZeroCount;
            for (leadingZeroCount = 0; leadingZeroCount < workingBitsAvailable; ++leadingZeroCount) {
              if ((workingWord & 2147483648 >>> leadingZeroCount) !== 0) {
                workingWord <<= leadingZeroCount;
                workingBitsAvailable -= leadingZeroCount;
                return leadingZeroCount;
              }
            }
            this.loadWord();
            return leadingZeroCount + this.skipLeadingZeros();
          };
          this.skipUnsignedExpGolomb = function() {
            this.skipBits(1 + this.skipLeadingZeros());
          };
          this.skipExpGolomb = function() {
            this.skipBits(1 + this.skipLeadingZeros());
          };
          this.readUnsignedExpGolomb = function() {
            var clz = this.skipLeadingZeros();
            return this.readBits(clz + 1) - 1;
          };
          this.readExpGolomb = function() {
            var valu = this.readUnsignedExpGolomb();
            if (1 & valu) {
              return 1 + valu >>> 1;
            }
            return -1 * (valu >>> 1);
          };
          this.readBoolean = function() {
            return this.readBits(1) === 1;
          };
          this.readUnsignedByte = function() {
            return this.readBits(8);
          };
          this.loadWord();
        };
        var expGolomb = ExpGolomb;
        var _H264Stream, _NalByteStream;
        var PROFILES_WITH_OPTIONAL_SPS_DATA;
        _NalByteStream = function NalByteStream() {
          var syncPoint = 0, i, buffer;
          _NalByteStream.prototype.init.call(this);
          this.push = function(data) {
            var swapBuffer;
            if (!buffer) {
              buffer = data.data;
            } else {
              swapBuffer = new Uint8Array(buffer.byteLength + data.data.byteLength);
              swapBuffer.set(buffer);
              swapBuffer.set(data.data, buffer.byteLength);
              buffer = swapBuffer;
            }
            var len = buffer.byteLength;
            for (; syncPoint < len - 3; syncPoint++) {
              if (buffer[syncPoint + 2] === 1) {
                i = syncPoint + 5;
                break;
              }
            }
            while (i < len) {
              switch (buffer[i]) {
                case 0:
                  if (buffer[i - 1] !== 0) {
                    i += 2;
                    break;
                  } else if (buffer[i - 2] !== 0) {
                    i++;
                    break;
                  }
                  if (syncPoint + 3 !== i - 2) {
                    this.trigger("data", buffer.subarray(syncPoint + 3, i - 2));
                  }
                  do {
                    i++;
                  } while (buffer[i] !== 1 && i < len);
                  syncPoint = i - 2;
                  i += 3;
                  break;
                case 1:
                  if (buffer[i - 1] !== 0 || buffer[i - 2] !== 0) {
                    i += 3;
                    break;
                  }
                  this.trigger("data", buffer.subarray(syncPoint + 3, i - 2));
                  syncPoint = i - 2;
                  i += 3;
                  break;
                default:
                  i += 3;
                  break;
              }
            }
            buffer = buffer.subarray(syncPoint);
            i -= syncPoint;
            syncPoint = 0;
          };
          this.reset = function() {
            buffer = null;
            syncPoint = 0;
            this.trigger("reset");
          };
          this.flush = function() {
            if (buffer && buffer.byteLength > 3) {
              this.trigger("data", buffer.subarray(syncPoint + 3));
            }
            buffer = null;
            syncPoint = 0;
            this.trigger("done");
          };
          this.endTimeline = function() {
            this.flush();
            this.trigger("endedtimeline");
          };
        };
        _NalByteStream.prototype = new stream();
        PROFILES_WITH_OPTIONAL_SPS_DATA = {
          100: true,
          110: true,
          122: true,
          244: true,
          44: true,
          83: true,
          86: true,
          118: true,
          128: true,
          138: true,
          139: true,
          134: true
        };
        _H264Stream = function H264Stream2() {
          var nalByteStream = new _NalByteStream(), self2, trackId, currentPts, currentDts, discardEmulationPreventionBytes2, readSequenceParameterSet, skipScalingList;
          _H264Stream.prototype.init.call(this);
          self2 = this;
          this.push = function(packet) {
            if (packet.type !== "video") {
              return;
            }
            trackId = packet.trackId;
            currentPts = packet.pts;
            currentDts = packet.dts;
            nalByteStream.push(packet);
          };
          nalByteStream.on("data", function(data) {
            var event = {
              trackId,
              pts: currentPts,
              dts: currentDts,
              data,
              nalUnitTypeCode: data[0] & 31
            };
            switch (event.nalUnitTypeCode) {
              case 5:
                event.nalUnitType = "slice_layer_without_partitioning_rbsp_idr";
                break;
              case 6:
                event.nalUnitType = "sei_rbsp";
                event.escapedRBSP = discardEmulationPreventionBytes2(data.subarray(1));
                break;
              case 7:
                event.nalUnitType = "seq_parameter_set_rbsp";
                event.escapedRBSP = discardEmulationPreventionBytes2(data.subarray(1));
                event.config = readSequenceParameterSet(event.escapedRBSP);
                break;
              case 8:
                event.nalUnitType = "pic_parameter_set_rbsp";
                break;
              case 9:
                event.nalUnitType = "access_unit_delimiter_rbsp";
                break;
            }
            self2.trigger("data", event);
          });
          nalByteStream.on("done", function() {
            self2.trigger("done");
          });
          nalByteStream.on("partialdone", function() {
            self2.trigger("partialdone");
          });
          nalByteStream.on("reset", function() {
            self2.trigger("reset");
          });
          nalByteStream.on("endedtimeline", function() {
            self2.trigger("endedtimeline");
          });
          this.flush = function() {
            nalByteStream.flush();
          };
          this.partialFlush = function() {
            nalByteStream.partialFlush();
          };
          this.reset = function() {
            nalByteStream.reset();
          };
          this.endTimeline = function() {
            nalByteStream.endTimeline();
          };
          skipScalingList = function skipScalingList2(count, expGolombDecoder) {
            var lastScale = 8, nextScale = 8, j, deltaScale;
            for (j = 0; j < count; j++) {
              if (nextScale !== 0) {
                deltaScale = expGolombDecoder.readExpGolomb();
                nextScale = (lastScale + deltaScale + 256) % 256;
              }
              lastScale = nextScale === 0 ? lastScale : nextScale;
            }
          };
          discardEmulationPreventionBytes2 = function discardEmulationPreventionBytes3(data) {
            var length = data.byteLength, emulationPreventionBytesPositions = [], i = 1, newLength, newData;
            while (i < length - 2) {
              if (data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 3) {
                emulationPreventionBytesPositions.push(i + 2);
                i += 2;
              } else {
                i++;
              }
            }
            if (emulationPreventionBytesPositions.length === 0) {
              return data;
            }
            newLength = length - emulationPreventionBytesPositions.length;
            newData = new Uint8Array(newLength);
            var sourceIndex = 0;
            for (i = 0; i < newLength; sourceIndex++, i++) {
              if (sourceIndex === emulationPreventionBytesPositions[0]) {
                sourceIndex++;
                emulationPreventionBytesPositions.shift();
              }
              newData[i] = data[sourceIndex];
            }
            return newData;
          };
          readSequenceParameterSet = function readSequenceParameterSet2(data) {
            var frameCropLeftOffset = 0, frameCropRightOffset = 0, frameCropTopOffset = 0, frameCropBottomOffset = 0, expGolombDecoder, profileIdc, levelIdc, profileCompatibility, chromaFormatIdc, picOrderCntType, numRefFramesInPicOrderCntCycle, picWidthInMbsMinus1, picHeightInMapUnitsMinus1, frameMbsOnlyFlag, scalingListCount, sarRatio = [1, 1], aspectRatioIdc, i;
            expGolombDecoder = new expGolomb(data);
            profileIdc = expGolombDecoder.readUnsignedByte();
            profileCompatibility = expGolombDecoder.readUnsignedByte();
            levelIdc = expGolombDecoder.readUnsignedByte();
            expGolombDecoder.skipUnsignedExpGolomb();
            if (PROFILES_WITH_OPTIONAL_SPS_DATA[profileIdc]) {
              chromaFormatIdc = expGolombDecoder.readUnsignedExpGolomb();
              if (chromaFormatIdc === 3) {
                expGolombDecoder.skipBits(1);
              }
              expGolombDecoder.skipUnsignedExpGolomb();
              expGolombDecoder.skipUnsignedExpGolomb();
              expGolombDecoder.skipBits(1);
              if (expGolombDecoder.readBoolean()) {
                scalingListCount = chromaFormatIdc !== 3 ? 8 : 12;
                for (i = 0; i < scalingListCount; i++) {
                  if (expGolombDecoder.readBoolean()) {
                    if (i < 6) {
                      skipScalingList(16, expGolombDecoder);
                    } else {
                      skipScalingList(64, expGolombDecoder);
                    }
                  }
                }
              }
            }
            expGolombDecoder.skipUnsignedExpGolomb();
            picOrderCntType = expGolombDecoder.readUnsignedExpGolomb();
            if (picOrderCntType === 0) {
              expGolombDecoder.readUnsignedExpGolomb();
            } else if (picOrderCntType === 1) {
              expGolombDecoder.skipBits(1);
              expGolombDecoder.skipExpGolomb();
              expGolombDecoder.skipExpGolomb();
              numRefFramesInPicOrderCntCycle = expGolombDecoder.readUnsignedExpGolomb();
              for (i = 0; i < numRefFramesInPicOrderCntCycle; i++) {
                expGolombDecoder.skipExpGolomb();
              }
            }
            expGolombDecoder.skipUnsignedExpGolomb();
            expGolombDecoder.skipBits(1);
            picWidthInMbsMinus1 = expGolombDecoder.readUnsignedExpGolomb();
            picHeightInMapUnitsMinus1 = expGolombDecoder.readUnsignedExpGolomb();
            frameMbsOnlyFlag = expGolombDecoder.readBits(1);
            if (frameMbsOnlyFlag === 0) {
              expGolombDecoder.skipBits(1);
            }
            expGolombDecoder.skipBits(1);
            if (expGolombDecoder.readBoolean()) {
              frameCropLeftOffset = expGolombDecoder.readUnsignedExpGolomb();
              frameCropRightOffset = expGolombDecoder.readUnsignedExpGolomb();
              frameCropTopOffset = expGolombDecoder.readUnsignedExpGolomb();
              frameCropBottomOffset = expGolombDecoder.readUnsignedExpGolomb();
            }
            if (expGolombDecoder.readBoolean()) {
              if (expGolombDecoder.readBoolean()) {
                aspectRatioIdc = expGolombDecoder.readUnsignedByte();
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
                    sarRatio = [expGolombDecoder.readUnsignedByte() << 8 | expGolombDecoder.readUnsignedByte(), expGolombDecoder.readUnsignedByte() << 8 | expGolombDecoder.readUnsignedByte()];
                    break;
                  }
                }
                if (sarRatio) {
                  sarRatio[0] / sarRatio[1];
                }
              }
            }
            return {
              profileIdc,
              levelIdc,
              profileCompatibility,
              width: (picWidthInMbsMinus1 + 1) * 16 - frameCropLeftOffset * 2 - frameCropRightOffset * 2,
              height: (2 - frameMbsOnlyFlag) * (picHeightInMapUnitsMinus1 + 1) * 16 - frameCropTopOffset * 2 - frameCropBottomOffset * 2,
              sarRatio
            };
          };
        };
        _H264Stream.prototype = new stream();
        var h264 = {
          H264Stream: _H264Stream,
          NalByteStream: _NalByteStream
        };
        var codecs = {
          Adts: adts,
          h264
        };
        var MAX_UINT32$1 = Math.pow(2, 32);
        var getUint64$4 = function getUint642(uint8) {
          var dv = new DataView(uint8.buffer, uint8.byteOffset, uint8.byteLength);
          var value;
          if (dv.getBigUint64) {
            value = dv.getBigUint64(0);
            if (value < Number.MAX_SAFE_INTEGER) {
              return Number(value);
            }
            return value;
          }
          return dv.getUint32(0) * MAX_UINT32$1 + dv.getUint32(4);
        };
        var numbers = {
          getUint64: getUint64$4,
          MAX_UINT32: MAX_UINT32$1
        };
        var MAX_UINT32 = numbers.MAX_UINT32;
        var box, dinf, esds, ftyp, mdat, mfhd, minf, moof, moov, mvex, mvhd, trak, tkhd, mdia, mdhd, hdlr, sdtp, stbl, stsd, traf, trex, trun$1, types, MAJOR_BRAND, MINOR_VERSION, AVC1_BRAND, VIDEO_HDLR, AUDIO_HDLR, HDLR_TYPES, VMHD, SMHD, DREF, STCO, STSC, STSZ, STTS;
        (function() {
          var i;
          types = {
            avc1: [],
            avcC: [],
            btrt: [],
            dinf: [],
            dref: [],
            esds: [],
            ftyp: [],
            hdlr: [],
            mdat: [],
            mdhd: [],
            mdia: [],
            mfhd: [],
            minf: [],
            moof: [],
            moov: [],
            mp4a: [],
            mvex: [],
            mvhd: [],
            pasp: [],
            sdtp: [],
            smhd: [],
            stbl: [],
            stco: [],
            stsc: [],
            stsd: [],
            stsz: [],
            stts: [],
            styp: [],
            tfdt: [],
            tfhd: [],
            traf: [],
            trak: [],
            trun: [],
            trex: [],
            tkhd: [],
            vmhd: []
          };
          if (typeof Uint8Array === "undefined") {
            return;
          }
          for (i in types) {
            if (types.hasOwnProperty(i)) {
              types[i] = [i.charCodeAt(0), i.charCodeAt(1), i.charCodeAt(2), i.charCodeAt(3)];
            }
          }
          MAJOR_BRAND = new Uint8Array(["i".charCodeAt(0), "s".charCodeAt(0), "o".charCodeAt(0), "m".charCodeAt(0)]);
          AVC1_BRAND = new Uint8Array(["a".charCodeAt(0), "v".charCodeAt(0), "c".charCodeAt(0), "1".charCodeAt(0)]);
          MINOR_VERSION = new Uint8Array([0, 0, 0, 1]);
          VIDEO_HDLR = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            118,
            105,
            100,
            101,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            86,
            105,
            100,
            101,
            111,
            72,
            97,
            110,
            100,
            108,
            101,
            114,
            0
          ]);
          AUDIO_HDLR = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            115,
            111,
            117,
            110,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            83,
            111,
            117,
            110,
            100,
            72,
            97,
            110,
            100,
            108,
            101,
            114,
            0
          ]);
          HDLR_TYPES = {
            video: VIDEO_HDLR,
            audio: AUDIO_HDLR
          };
          DREF = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            12,
            117,
            114,
            108,
            32,
            0,
            0,
            0,
            1
          ]);
          SMHD = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
          ]);
          STCO = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
          ]);
          STSC = STCO;
          STSZ = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
          ]);
          STTS = STCO;
          VMHD = new Uint8Array([
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
          ]);
        })();
        box = function box2(type2) {
          var payload = [], size = 0, i, result, view;
          for (i = 1; i < arguments.length; i++) {
            payload.push(arguments[i]);
          }
          i = payload.length;
          while (i--) {
            size += payload[i].byteLength;
          }
          result = new Uint8Array(size + 8);
          view = new DataView(result.buffer, result.byteOffset, result.byteLength);
          view.setUint32(0, result.byteLength);
          result.set(type2, 4);
          for (i = 0, size = 8; i < payload.length; i++) {
            result.set(payload[i], size);
            size += payload[i].byteLength;
          }
          return result;
        };
        dinf = function dinf2() {
          return box(types.dinf, box(types.dref, DREF));
        };
        esds = function esds2(track) {
          return box(types.esds, new Uint8Array([
            0,
            0,
            0,
            0,
            3,
            25,
            0,
            0,
            0,
            4,
            17,
            64,
            21,
            0,
            6,
            0,
            0,
            0,
            218,
            192,
            0,
            0,
            218,
            192,
            5,
            2,
            track.audioobjecttype << 3 | track.samplingfrequencyindex >>> 1,
            track.samplingfrequencyindex << 7 | track.channelcount << 3,
            6,
            1,
            2
          ]));
        };
        ftyp = function ftyp2() {
          return box(types.ftyp, MAJOR_BRAND, MINOR_VERSION, MAJOR_BRAND, AVC1_BRAND);
        };
        hdlr = function hdlr2(type2) {
          return box(types.hdlr, HDLR_TYPES[type2]);
        };
        mdat = function mdat2(data) {
          return box(types.mdat, data);
        };
        mdhd = function mdhd2(track) {
          var result = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            2,
            0,
            0,
            0,
            3,
            0,
            1,
            95,
            144,
            track.duration >>> 24 & 255,
            track.duration >>> 16 & 255,
            track.duration >>> 8 & 255,
            track.duration & 255,
            85,
            196,
            0,
            0
          ]);
          if (track.samplerate) {
            result[12] = track.samplerate >>> 24 & 255;
            result[13] = track.samplerate >>> 16 & 255;
            result[14] = track.samplerate >>> 8 & 255;
            result[15] = track.samplerate & 255;
          }
          return box(types.mdhd, result);
        };
        mdia = function mdia2(track) {
          return box(types.mdia, mdhd(track), hdlr(track.type), minf(track));
        };
        mfhd = function mfhd2(sequenceNumber) {
          return box(types.mfhd, new Uint8Array([
            0,
            0,
            0,
            0,
            (sequenceNumber & 4278190080) >> 24,
            (sequenceNumber & 16711680) >> 16,
            (sequenceNumber & 65280) >> 8,
            sequenceNumber & 255
          ]));
        };
        minf = function minf2(track) {
          return box(types.minf, track.type === "video" ? box(types.vmhd, VMHD) : box(types.smhd, SMHD), dinf(), stbl(track));
        };
        moof = function moof2(sequenceNumber, tracks) {
          var trackFragments = [], i = tracks.length;
          while (i--) {
            trackFragments[i] = traf(tracks[i]);
          }
          return box.apply(null, [types.moof, mfhd(sequenceNumber)].concat(trackFragments));
        };
        moov = function moov2(tracks) {
          var i = tracks.length, boxes = [];
          while (i--) {
            boxes[i] = trak(tracks[i]);
          }
          return box.apply(null, [types.moov, mvhd(4294967295)].concat(boxes).concat(mvex(tracks)));
        };
        mvex = function mvex2(tracks) {
          var i = tracks.length, boxes = [];
          while (i--) {
            boxes[i] = trex(tracks[i]);
          }
          return box.apply(null, [types.mvex].concat(boxes));
        };
        mvhd = function mvhd2(duration) {
          var bytes = new Uint8Array([
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            2,
            0,
            1,
            95,
            144,
            (duration & 4278190080) >> 24,
            (duration & 16711680) >> 16,
            (duration & 65280) >> 8,
            duration & 255,
            0,
            1,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            64,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            255,
            255,
            255,
            255
          ]);
          return box(types.mvhd, bytes);
        };
        sdtp = function sdtp2(track) {
          var samples = track.samples || [], bytes = new Uint8Array(4 + samples.length), flags, i;
          for (i = 0; i < samples.length; i++) {
            flags = samples[i].flags;
            bytes[i + 4] = flags.dependsOn << 4 | flags.isDependedOn << 2 | flags.hasRedundancy;
          }
          return box(types.sdtp, bytes);
        };
        stbl = function stbl2(track) {
          return box(types.stbl, stsd(track), box(types.stts, STTS), box(types.stsc, STSC), box(types.stsz, STSZ), box(types.stco, STCO));
        };
        (function() {
          var videoSample, audioSample;
          stsd = function stsd2(track) {
            return box(types.stsd, new Uint8Array([
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              1
            ]), track.type === "video" ? videoSample(track) : audioSample(track));
          };
          videoSample = function videoSample2(track) {
            var sps = track.sps || [], pps = track.pps || [], sequenceParameterSets = [], pictureParameterSets = [], i, avc1Box;
            for (i = 0; i < sps.length; i++) {
              sequenceParameterSets.push((sps[i].byteLength & 65280) >>> 8);
              sequenceParameterSets.push(sps[i].byteLength & 255);
              sequenceParameterSets = sequenceParameterSets.concat(Array.prototype.slice.call(sps[i]));
            }
            for (i = 0; i < pps.length; i++) {
              pictureParameterSets.push((pps[i].byteLength & 65280) >>> 8);
              pictureParameterSets.push(pps[i].byteLength & 255);
              pictureParameterSets = pictureParameterSets.concat(Array.prototype.slice.call(pps[i]));
            }
            avc1Box = [types.avc1, new Uint8Array([
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              1,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              (track.width & 65280) >> 8,
              track.width & 255,
              (track.height & 65280) >> 8,
              track.height & 255,
              0,
              72,
              0,
              0,
              0,
              72,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              1,
              19,
              118,
              105,
              100,
              101,
              111,
              106,
              115,
              45,
              99,
              111,
              110,
              116,
              114,
              105,
              98,
              45,
              104,
              108,
              115,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              24,
              17,
              17
            ]), box(types.avcC, new Uint8Array([
              1,
              track.profileIdc,
              track.profileCompatibility,
              track.levelIdc,
              255
            ].concat([sps.length], sequenceParameterSets, [pps.length], pictureParameterSets))), box(types.btrt, new Uint8Array([
              0,
              28,
              156,
              128,
              0,
              45,
              198,
              192,
              0,
              45,
              198,
              192
            ]))];
            if (track.sarRatio) {
              var hSpacing = track.sarRatio[0], vSpacing = track.sarRatio[1];
              avc1Box.push(box(types.pasp, new Uint8Array([(hSpacing & 4278190080) >> 24, (hSpacing & 16711680) >> 16, (hSpacing & 65280) >> 8, hSpacing & 255, (vSpacing & 4278190080) >> 24, (vSpacing & 16711680) >> 16, (vSpacing & 65280) >> 8, vSpacing & 255])));
            }
            return box.apply(null, avc1Box);
          };
          audioSample = function audioSample2(track) {
            return box(types.mp4a, new Uint8Array([
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              1,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              0,
              (track.channelcount & 65280) >> 8,
              track.channelcount & 255,
              (track.samplesize & 65280) >> 8,
              track.samplesize & 255,
              0,
              0,
              0,
              0,
              (track.samplerate & 65280) >> 8,
              track.samplerate & 255,
              0,
              0
            ]), esds(track));
          };
        })();
        tkhd = function tkhd2(track) {
          var result = new Uint8Array([
            0,
            0,
            0,
            7,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            (track.id & 4278190080) >> 24,
            (track.id & 16711680) >> 16,
            (track.id & 65280) >> 8,
            track.id & 255,
            0,
            0,
            0,
            0,
            (track.duration & 4278190080) >> 24,
            (track.duration & 16711680) >> 16,
            (track.duration & 65280) >> 8,
            track.duration & 255,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            64,
            0,
            0,
            0,
            (track.width & 65280) >> 8,
            track.width & 255,
            0,
            0,
            (track.height & 65280) >> 8,
            track.height & 255,
            0,
            0
          ]);
          return box(types.tkhd, result);
        };
        traf = function traf2(track) {
          var trackFragmentHeader, trackFragmentDecodeTime, trackFragmentRun, sampleDependencyTable, dataOffset, upperWordBaseMediaDecodeTime, lowerWordBaseMediaDecodeTime;
          trackFragmentHeader = box(types.tfhd, new Uint8Array([
            0,
            0,
            0,
            58,
            (track.id & 4278190080) >> 24,
            (track.id & 16711680) >> 16,
            (track.id & 65280) >> 8,
            track.id & 255,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0
          ]));
          upperWordBaseMediaDecodeTime = Math.floor(track.baseMediaDecodeTime / MAX_UINT32);
          lowerWordBaseMediaDecodeTime = Math.floor(track.baseMediaDecodeTime % MAX_UINT32);
          trackFragmentDecodeTime = box(types.tfdt, new Uint8Array([
            1,
            0,
            0,
            0,
            upperWordBaseMediaDecodeTime >>> 24 & 255,
            upperWordBaseMediaDecodeTime >>> 16 & 255,
            upperWordBaseMediaDecodeTime >>> 8 & 255,
            upperWordBaseMediaDecodeTime & 255,
            lowerWordBaseMediaDecodeTime >>> 24 & 255,
            lowerWordBaseMediaDecodeTime >>> 16 & 255,
            lowerWordBaseMediaDecodeTime >>> 8 & 255,
            lowerWordBaseMediaDecodeTime & 255
          ]));
          dataOffset = 32 + 20 + 8 + 16 + 8 + 8;
          if (track.type === "audio") {
            trackFragmentRun = trun$1(track, dataOffset);
            return box(types.traf, trackFragmentHeader, trackFragmentDecodeTime, trackFragmentRun);
          }
          sampleDependencyTable = sdtp(track);
          trackFragmentRun = trun$1(track, sampleDependencyTable.length + dataOffset);
          return box(types.traf, trackFragmentHeader, trackFragmentDecodeTime, trackFragmentRun, sampleDependencyTable);
        };
        trak = function trak2(track) {
          track.duration = track.duration || 4294967295;
          return box(types.trak, tkhd(track), mdia(track));
        };
        trex = function trex2(track) {
          var result = new Uint8Array([
            0,
            0,
            0,
            0,
            (track.id & 4278190080) >> 24,
            (track.id & 16711680) >> 16,
            (track.id & 65280) >> 8,
            track.id & 255,
            0,
            0,
            0,
            1,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
            0,
            1
          ]);
          if (track.type !== "video") {
            result[result.length - 1] = 0;
          }
          return box(types.trex, result);
        };
        (function() {
          var audioTrun, videoTrun, trunHeader;
          trunHeader = function trunHeader2(samples, offset) {
            var durationPresent = 0, sizePresent = 0, flagsPresent = 0, compositionTimeOffset = 0;
            if (samples.length) {
              if (samples[0].duration !== void 0) {
                durationPresent = 1;
              }
              if (samples[0].size !== void 0) {
                sizePresent = 2;
              }
              if (samples[0].flags !== void 0) {
                flagsPresent = 4;
              }
              if (samples[0].compositionTimeOffset !== void 0) {
                compositionTimeOffset = 8;
              }
            }
            return [
              0,
              0,
              durationPresent | sizePresent | flagsPresent | compositionTimeOffset,
              1,
              (samples.length & 4278190080) >>> 24,
              (samples.length & 16711680) >>> 16,
              (samples.length & 65280) >>> 8,
              samples.length & 255,
              (offset & 4278190080) >>> 24,
              (offset & 16711680) >>> 16,
              (offset & 65280) >>> 8,
              offset & 255
            ];
          };
          videoTrun = function videoTrun2(track, offset) {
            var bytesOffest, bytes, header, samples, sample, i;
            samples = track.samples || [];
            offset += 8 + 12 + 16 * samples.length;
            header = trunHeader(samples, offset);
            bytes = new Uint8Array(header.length + samples.length * 16);
            bytes.set(header);
            bytesOffest = header.length;
            for (i = 0; i < samples.length; i++) {
              sample = samples[i];
              bytes[bytesOffest++] = (sample.duration & 4278190080) >>> 24;
              bytes[bytesOffest++] = (sample.duration & 16711680) >>> 16;
              bytes[bytesOffest++] = (sample.duration & 65280) >>> 8;
              bytes[bytesOffest++] = sample.duration & 255;
              bytes[bytesOffest++] = (sample.size & 4278190080) >>> 24;
              bytes[bytesOffest++] = (sample.size & 16711680) >>> 16;
              bytes[bytesOffest++] = (sample.size & 65280) >>> 8;
              bytes[bytesOffest++] = sample.size & 255;
              bytes[bytesOffest++] = sample.flags.isLeading << 2 | sample.flags.dependsOn;
              bytes[bytesOffest++] = sample.flags.isDependedOn << 6 | sample.flags.hasRedundancy << 4 | sample.flags.paddingValue << 1 | sample.flags.isNonSyncSample;
              bytes[bytesOffest++] = sample.flags.degradationPriority & 240 << 8;
              bytes[bytesOffest++] = sample.flags.degradationPriority & 15;
              bytes[bytesOffest++] = (sample.compositionTimeOffset & 4278190080) >>> 24;
              bytes[bytesOffest++] = (sample.compositionTimeOffset & 16711680) >>> 16;
              bytes[bytesOffest++] = (sample.compositionTimeOffset & 65280) >>> 8;
              bytes[bytesOffest++] = sample.compositionTimeOffset & 255;
            }
            return box(types.trun, bytes);
          };
          audioTrun = function audioTrun2(track, offset) {
            var bytes, bytesOffest, header, samples, sample, i;
            samples = track.samples || [];
            offset += 8 + 12 + 8 * samples.length;
            header = trunHeader(samples, offset);
            bytes = new Uint8Array(header.length + samples.length * 8);
            bytes.set(header);
            bytesOffest = header.length;
            for (i = 0; i < samples.length; i++) {
              sample = samples[i];
              bytes[bytesOffest++] = (sample.duration & 4278190080) >>> 24;
              bytes[bytesOffest++] = (sample.duration & 16711680) >>> 16;
              bytes[bytesOffest++] = (sample.duration & 65280) >>> 8;
              bytes[bytesOffest++] = sample.duration & 255;
              bytes[bytesOffest++] = (sample.size & 4278190080) >>> 24;
              bytes[bytesOffest++] = (sample.size & 16711680) >>> 16;
              bytes[bytesOffest++] = (sample.size & 65280) >>> 8;
              bytes[bytesOffest++] = sample.size & 255;
            }
            return box(types.trun, bytes);
          };
          trun$1 = function trun2(track, offset) {
            if (track.type === "audio") {
              return audioTrun(track, offset);
            }
            return videoTrun(track, offset);
          };
        })();
        var mp4Generator = {
          ftyp,
          mdat,
          moof,
          moov,
          initSegment: function initSegment(tracks) {
            var fileType = ftyp(), movie = moov(tracks), result;
            result = new Uint8Array(fileType.byteLength + movie.byteLength);
            result.set(fileType);
            result.set(movie, fileType.byteLength);
            return result;
          }
        };
        var toUnsigned$3 = function toUnsigned2(value) {
          return value >>> 0;
        };
        var toHexString$1 = function toHexString2(value) {
          return ("00" + value.toString(16)).slice(-2);
        };
        var bin = {
          toUnsigned: toUnsigned$3,
          toHexString: toHexString$1
        };
        var parseType$2 = function parseType2(buffer) {
          var result = "";
          result += String.fromCharCode(buffer[0]);
          result += String.fromCharCode(buffer[1]);
          result += String.fromCharCode(buffer[2]);
          result += String.fromCharCode(buffer[3]);
          return result;
        };
        var parseType_1 = parseType$2;
        var toUnsigned$2 = bin.toUnsigned;
        var findBox = function findBox2(data, path) {
          var results = [], i, size, type2, end, subresults;
          if (!path.length) {
            return null;
          }
          for (i = 0; i < data.byteLength; ) {
            size = toUnsigned$2(data[i] << 24 | data[i + 1] << 16 | data[i + 2] << 8 | data[i + 3]);
            type2 = parseType_1(data.subarray(i + 4, i + 8));
            end = size > 1 ? i + size : data.byteLength;
            if (type2 === path[0]) {
              if (path.length === 1) {
                results.push(data.subarray(i + 8, end));
              } else {
                subresults = findBox2(data.subarray(i + 8, end), path.slice(1));
                if (subresults.length) {
                  results = results.concat(subresults);
                }
              }
            }
            i = end;
          }
          return results;
        };
        var findBox_1 = findBox;
        var tfhd = function tfhd2(data) {
          var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
            version: data[0],
            flags: new Uint8Array(data.subarray(1, 4)),
            trackId: view.getUint32(4)
          }, baseDataOffsetPresent = result.flags[2] & 1, sampleDescriptionIndexPresent = result.flags[2] & 2, defaultSampleDurationPresent = result.flags[2] & 8, defaultSampleSizePresent = result.flags[2] & 16, defaultSampleFlagsPresent = result.flags[2] & 32, durationIsEmpty = result.flags[0] & 65536, defaultBaseIsMoof = result.flags[0] & 131072, i;
          i = 8;
          if (baseDataOffsetPresent) {
            i += 4;
            result.baseDataOffset = view.getUint32(12);
            i += 4;
          }
          if (sampleDescriptionIndexPresent) {
            result.sampleDescriptionIndex = view.getUint32(i);
            i += 4;
          }
          if (defaultSampleDurationPresent) {
            result.defaultSampleDuration = view.getUint32(i);
            i += 4;
          }
          if (defaultSampleSizePresent) {
            result.defaultSampleSize = view.getUint32(i);
            i += 4;
          }
          if (defaultSampleFlagsPresent) {
            result.defaultSampleFlags = view.getUint32(i);
          }
          if (durationIsEmpty) {
            result.durationIsEmpty = true;
          }
          if (!baseDataOffsetPresent && defaultBaseIsMoof) {
            result.baseDataOffsetIsMoof = true;
          }
          return result;
        };
        var parseTfhd = tfhd;
        var parseSampleFlags = function parseSampleFlags2(flags) {
          return {
            isLeading: (flags[0] & 12) >>> 2,
            dependsOn: flags[0] & 3,
            isDependedOn: (flags[1] & 192) >>> 6,
            hasRedundancy: (flags[1] & 48) >>> 4,
            paddingValue: (flags[1] & 14) >>> 1,
            isNonSyncSample: flags[1] & 1,
            degradationPriority: flags[2] << 8 | flags[3]
          };
        };
        var parseSampleFlags_1 = parseSampleFlags;
        var trun = function trun2(data) {
          var result = {
            version: data[0],
            flags: new Uint8Array(data.subarray(1, 4)),
            samples: []
          }, view = new DataView(data.buffer, data.byteOffset, data.byteLength), dataOffsetPresent = result.flags[2] & 1, firstSampleFlagsPresent = result.flags[2] & 4, sampleDurationPresent = result.flags[1] & 1, sampleSizePresent = result.flags[1] & 2, sampleFlagsPresent = result.flags[1] & 4, sampleCompositionTimeOffsetPresent = result.flags[1] & 8, sampleCount = view.getUint32(4), offset = 8, sample;
          if (dataOffsetPresent) {
            result.dataOffset = view.getInt32(offset);
            offset += 4;
          }
          if (firstSampleFlagsPresent && sampleCount) {
            sample = {
              flags: parseSampleFlags_1(data.subarray(offset, offset + 4))
            };
            offset += 4;
            if (sampleDurationPresent) {
              sample.duration = view.getUint32(offset);
              offset += 4;
            }
            if (sampleSizePresent) {
              sample.size = view.getUint32(offset);
              offset += 4;
            }
            if (sampleCompositionTimeOffsetPresent) {
              if (result.version === 1) {
                sample.compositionTimeOffset = view.getInt32(offset);
              } else {
                sample.compositionTimeOffset = view.getUint32(offset);
              }
              offset += 4;
            }
            result.samples.push(sample);
            sampleCount--;
          }
          while (sampleCount--) {
            sample = {};
            if (sampleDurationPresent) {
              sample.duration = view.getUint32(offset);
              offset += 4;
            }
            if (sampleSizePresent) {
              sample.size = view.getUint32(offset);
              offset += 4;
            }
            if (sampleFlagsPresent) {
              sample.flags = parseSampleFlags_1(data.subarray(offset, offset + 4));
              offset += 4;
            }
            if (sampleCompositionTimeOffsetPresent) {
              if (result.version === 1) {
                sample.compositionTimeOffset = view.getInt32(offset);
              } else {
                sample.compositionTimeOffset = view.getUint32(offset);
              }
              offset += 4;
            }
            result.samples.push(sample);
          }
          return result;
        };
        var parseTrun = trun;
        var toUnsigned$1 = bin.toUnsigned;
        var getUint64$3 = numbers.getUint64;
        var tfdt = function tfdt2(data) {
          var result = {
            version: data[0],
            flags: new Uint8Array(data.subarray(1, 4))
          };
          if (result.version === 1) {
            result.baseMediaDecodeTime = getUint64$3(data.subarray(4));
          } else {
            result.baseMediaDecodeTime = toUnsigned$1(data[4] << 24 | data[5] << 16 | data[6] << 8 | data[7]);
          }
          return result;
        };
        var parseTfdt = tfdt;
        var toUnsigned = bin.toUnsigned;
        var toHexString = bin.toHexString;
        var getUint64$2 = numbers.getUint64;
        var timescale, startTime, compositionStartTime, getVideoTrackIds, getTracks, getTimescaleFromMediaHeader;
        timescale = function timescale2(init2) {
          var result = {}, traks = findBox_1(init2, ["moov", "trak"]);
          return traks.reduce(function(result2, trak2) {
            var tkhd2, version, index, id, mdhd2;
            tkhd2 = findBox_1(trak2, ["tkhd"])[0];
            if (!tkhd2) {
              return null;
            }
            version = tkhd2[0];
            index = version === 0 ? 12 : 20;
            id = toUnsigned(tkhd2[index] << 24 | tkhd2[index + 1] << 16 | tkhd2[index + 2] << 8 | tkhd2[index + 3]);
            mdhd2 = findBox_1(trak2, ["mdia", "mdhd"])[0];
            if (!mdhd2) {
              return null;
            }
            version = mdhd2[0];
            index = version === 0 ? 12 : 20;
            result2[id] = toUnsigned(mdhd2[index] << 24 | mdhd2[index + 1] << 16 | mdhd2[index + 2] << 8 | mdhd2[index + 3]);
            return result2;
          }, result);
        };
        startTime = function startTime2(timescale2, fragment) {
          var trafs;
          trafs = findBox_1(fragment, ["moof", "traf"]);
          var lowestTime = trafs.reduce(function(acc, traf2) {
            var tfhd2 = findBox_1(traf2, ["tfhd"])[0];
            var id = toUnsigned(tfhd2[4] << 24 | tfhd2[5] << 16 | tfhd2[6] << 8 | tfhd2[7]);
            var scale = timescale2[id] || 9e4;
            var tfdt2 = findBox_1(traf2, ["tfdt"])[0];
            var dv = new DataView(tfdt2.buffer, tfdt2.byteOffset, tfdt2.byteLength);
            var baseTime;
            if (tfdt2[0] === 1) {
              baseTime = getUint64$2(tfdt2.subarray(4, 12));
            } else {
              baseTime = dv.getUint32(4);
            }
            var seconds;
            if (typeof baseTime === "bigint") {
              seconds = baseTime / window__default["default"].BigInt(scale);
            } else if (typeof baseTime === "number" && !isNaN(baseTime)) {
              seconds = baseTime / scale;
            }
            if (seconds < Number.MAX_SAFE_INTEGER) {
              seconds = Number(seconds);
            }
            if (seconds < acc) {
              acc = seconds;
            }
            return acc;
          }, Infinity);
          return typeof lowestTime === "bigint" || isFinite(lowestTime) ? lowestTime : 0;
        };
        compositionStartTime = function compositionStartTime2(timescales, fragment) {
          var trafBoxes = findBox_1(fragment, ["moof", "traf"]);
          var baseMediaDecodeTime = 0;
          var compositionTimeOffset = 0;
          var trackId;
          if (trafBoxes && trafBoxes.length) {
            var tfhd2 = findBox_1(trafBoxes[0], ["tfhd"])[0];
            var trun2 = findBox_1(trafBoxes[0], ["trun"])[0];
            var tfdt2 = findBox_1(trafBoxes[0], ["tfdt"])[0];
            if (tfhd2) {
              var parsedTfhd = parseTfhd(tfhd2);
              trackId = parsedTfhd.trackId;
            }
            if (tfdt2) {
              var parsedTfdt = parseTfdt(tfdt2);
              baseMediaDecodeTime = parsedTfdt.baseMediaDecodeTime;
            }
            if (trun2) {
              var parsedTrun = parseTrun(trun2);
              if (parsedTrun.samples && parsedTrun.samples.length) {
                compositionTimeOffset = parsedTrun.samples[0].compositionTimeOffset || 0;
              }
            }
          }
          var timescale2 = timescales[trackId] || 9e4;
          if (typeof baseMediaDecodeTime === "bigint") {
            compositionTimeOffset = window__default["default"].BigInt(compositionTimeOffset);
            timescale2 = window__default["default"].BigInt(timescale2);
          }
          var result = (baseMediaDecodeTime + compositionTimeOffset) / timescale2;
          if (typeof result === "bigint" && result < Number.MAX_SAFE_INTEGER) {
            result = Number(result);
          }
          return result;
        };
        getVideoTrackIds = function getVideoTrackIds2(init2) {
          var traks = findBox_1(init2, ["moov", "trak"]);
          var videoTrackIds = [];
          traks.forEach(function(trak2) {
            var hdlrs = findBox_1(trak2, ["mdia", "hdlr"]);
            var tkhds = findBox_1(trak2, ["tkhd"]);
            hdlrs.forEach(function(hdlr2, index) {
              var handlerType = parseType_1(hdlr2.subarray(8, 12));
              var tkhd2 = tkhds[index];
              var view;
              var version;
              var trackId;
              if (handlerType === "vide") {
                view = new DataView(tkhd2.buffer, tkhd2.byteOffset, tkhd2.byteLength);
                version = view.getUint8(0);
                trackId = version === 0 ? view.getUint32(12) : view.getUint32(20);
                videoTrackIds.push(trackId);
              }
            });
          });
          return videoTrackIds;
        };
        getTimescaleFromMediaHeader = function getTimescaleFromMediaHeader2(mdhd2) {
          var version = mdhd2[0];
          var index = version === 0 ? 12 : 20;
          return toUnsigned(mdhd2[index] << 24 | mdhd2[index + 1] << 16 | mdhd2[index + 2] << 8 | mdhd2[index + 3]);
        };
        getTracks = function getTracks2(init2) {
          var traks = findBox_1(init2, ["moov", "trak"]);
          var tracks = [];
          traks.forEach(function(trak2) {
            var track = {};
            var tkhd2 = findBox_1(trak2, ["tkhd"])[0];
            var view, tkhdVersion;
            if (tkhd2) {
              view = new DataView(tkhd2.buffer, tkhd2.byteOffset, tkhd2.byteLength);
              tkhdVersion = view.getUint8(0);
              track.id = tkhdVersion === 0 ? view.getUint32(12) : view.getUint32(20);
            }
            var hdlr2 = findBox_1(trak2, ["mdia", "hdlr"])[0];
            if (hdlr2) {
              var type2 = parseType_1(hdlr2.subarray(8, 12));
              if (type2 === "vide") {
                track.type = "video";
              } else if (type2 === "soun") {
                track.type = "audio";
              } else {
                track.type = type2;
              }
            }
            var stsd2 = findBox_1(trak2, ["mdia", "minf", "stbl", "stsd"])[0];
            if (stsd2) {
              var sampleDescriptions = stsd2.subarray(8);
              track.codec = parseType_1(sampleDescriptions.subarray(4, 8));
              var codecBox = findBox_1(sampleDescriptions, [track.codec])[0];
              var codecConfig, codecConfigType;
              if (codecBox) {
                if (/^[asm]vc[1-9]$/i.test(track.codec)) {
                  codecConfig = codecBox.subarray(78);
                  codecConfigType = parseType_1(codecConfig.subarray(4, 8));
                  if (codecConfigType === "avcC" && codecConfig.length > 11) {
                    track.codec += ".";
                    track.codec += toHexString(codecConfig[9]);
                    track.codec += toHexString(codecConfig[10]);
                    track.codec += toHexString(codecConfig[11]);
                  } else {
                    track.codec = "avc1.4d400d";
                  }
                } else if (/^mp4[a,v]$/i.test(track.codec)) {
                  codecConfig = codecBox.subarray(28);
                  codecConfigType = parseType_1(codecConfig.subarray(4, 8));
                  if (codecConfigType === "esds" && codecConfig.length > 20 && codecConfig[19] !== 0) {
                    track.codec += "." + toHexString(codecConfig[19]);
                    track.codec += "." + toHexString(codecConfig[20] >>> 2 & 63).replace(/^0/, "");
                  } else {
                    track.codec = "mp4a.40.2";
                  }
                } else {
                  track.codec = track.codec.toLowerCase();
                }
              }
            }
            var mdhd2 = findBox_1(trak2, ["mdia", "mdhd"])[0];
            if (mdhd2) {
              track.timescale = getTimescaleFromMediaHeader(mdhd2);
            }
            tracks.push(track);
          });
          return tracks;
        };
        var probe$2 = {
          findBox: findBox_1,
          parseType: parseType_1,
          timescale,
          startTime,
          compositionStartTime,
          videoTrackIds: getVideoTrackIds,
          tracks: getTracks,
          getTimescaleFromMediaHeader
        };
        var groupNalsIntoFrames = function groupNalsIntoFrames2(nalUnits) {
          var i, currentNal, currentFrame = [], frames = [];
          frames.byteLength = 0;
          frames.nalCount = 0;
          frames.duration = 0;
          currentFrame.byteLength = 0;
          for (i = 0; i < nalUnits.length; i++) {
            currentNal = nalUnits[i];
            if (currentNal.nalUnitType === "access_unit_delimiter_rbsp") {
              if (currentFrame.length) {
                currentFrame.duration = currentNal.dts - currentFrame.dts;
                frames.byteLength += currentFrame.byteLength;
                frames.nalCount += currentFrame.length;
                frames.duration += currentFrame.duration;
                frames.push(currentFrame);
              }
              currentFrame = [currentNal];
              currentFrame.byteLength = currentNal.data.byteLength;
              currentFrame.pts = currentNal.pts;
              currentFrame.dts = currentNal.dts;
            } else {
              if (currentNal.nalUnitType === "slice_layer_without_partitioning_rbsp_idr") {
                currentFrame.keyFrame = true;
              }
              currentFrame.duration = currentNal.dts - currentFrame.dts;
              currentFrame.byteLength += currentNal.data.byteLength;
              currentFrame.push(currentNal);
            }
          }
          if (frames.length && (!currentFrame.duration || currentFrame.duration <= 0)) {
            currentFrame.duration = frames[frames.length - 1].duration;
          }
          frames.byteLength += currentFrame.byteLength;
          frames.nalCount += currentFrame.length;
          frames.duration += currentFrame.duration;
          frames.push(currentFrame);
          return frames;
        };
        var groupFramesIntoGops = function groupFramesIntoGops2(frames) {
          var i, currentFrame, currentGop = [], gops = [];
          currentGop.byteLength = 0;
          currentGop.nalCount = 0;
          currentGop.duration = 0;
          currentGop.pts = frames[0].pts;
          currentGop.dts = frames[0].dts;
          gops.byteLength = 0;
          gops.nalCount = 0;
          gops.duration = 0;
          gops.pts = frames[0].pts;
          gops.dts = frames[0].dts;
          for (i = 0; i < frames.length; i++) {
            currentFrame = frames[i];
            if (currentFrame.keyFrame) {
              if (currentGop.length) {
                gops.push(currentGop);
                gops.byteLength += currentGop.byteLength;
                gops.nalCount += currentGop.nalCount;
                gops.duration += currentGop.duration;
              }
              currentGop = [currentFrame];
              currentGop.nalCount = currentFrame.length;
              currentGop.byteLength = currentFrame.byteLength;
              currentGop.pts = currentFrame.pts;
              currentGop.dts = currentFrame.dts;
              currentGop.duration = currentFrame.duration;
            } else {
              currentGop.duration += currentFrame.duration;
              currentGop.nalCount += currentFrame.length;
              currentGop.byteLength += currentFrame.byteLength;
              currentGop.push(currentFrame);
            }
          }
          if (gops.length && currentGop.duration <= 0) {
            currentGop.duration = gops[gops.length - 1].duration;
          }
          gops.byteLength += currentGop.byteLength;
          gops.nalCount += currentGop.nalCount;
          gops.duration += currentGop.duration;
          gops.push(currentGop);
          return gops;
        };
        var extendFirstKeyFrame = function extendFirstKeyFrame2(gops) {
          var currentGop;
          if (!gops[0][0].keyFrame && gops.length > 1) {
            currentGop = gops.shift();
            gops.byteLength -= currentGop.byteLength;
            gops.nalCount -= currentGop.nalCount;
            gops[0][0].dts = currentGop.dts;
            gops[0][0].pts = currentGop.pts;
            gops[0][0].duration += currentGop.duration;
          }
          return gops;
        };
        var createDefaultSample = function createDefaultSample2() {
          return {
            size: 0,
            flags: {
              isLeading: 0,
              dependsOn: 1,
              isDependedOn: 0,
              hasRedundancy: 0,
              degradationPriority: 0,
              isNonSyncSample: 1
            }
          };
        };
        var sampleForFrame = function sampleForFrame2(frame, dataOffset) {
          var sample = createDefaultSample();
          sample.dataOffset = dataOffset;
          sample.compositionTimeOffset = frame.pts - frame.dts;
          sample.duration = frame.duration;
          sample.size = 4 * frame.length;
          sample.size += frame.byteLength;
          if (frame.keyFrame) {
            sample.flags.dependsOn = 2;
            sample.flags.isNonSyncSample = 0;
          }
          return sample;
        };
        var generateSampleTable$1 = function generateSampleTable2(gops, baseDataOffset) {
          var h, i, sample, currentGop, currentFrame, dataOffset = baseDataOffset || 0, samples = [];
          for (h = 0; h < gops.length; h++) {
            currentGop = gops[h];
            for (i = 0; i < currentGop.length; i++) {
              currentFrame = currentGop[i];
              sample = sampleForFrame(currentFrame, dataOffset);
              dataOffset += sample.size;
              samples.push(sample);
            }
          }
          return samples;
        };
        var concatenateNalData = function concatenateNalData2(gops) {
          var h, i, j, currentGop, currentFrame, currentNal, dataOffset = 0, nalsByteLength = gops.byteLength, numberOfNals = gops.nalCount, totalByteLength = nalsByteLength + 4 * numberOfNals, data = new Uint8Array(totalByteLength), view = new DataView(data.buffer);
          for (h = 0; h < gops.length; h++) {
            currentGop = gops[h];
            for (i = 0; i < currentGop.length; i++) {
              currentFrame = currentGop[i];
              for (j = 0; j < currentFrame.length; j++) {
                currentNal = currentFrame[j];
                view.setUint32(dataOffset, currentNal.data.byteLength);
                dataOffset += 4;
                data.set(currentNal.data, dataOffset);
                dataOffset += currentNal.data.byteLength;
              }
            }
          }
          return data;
        };
        var generateSampleTableForFrame = function generateSampleTableForFrame2(frame, baseDataOffset) {
          var sample, dataOffset = baseDataOffset || 0, samples = [];
          sample = sampleForFrame(frame, dataOffset);
          samples.push(sample);
          return samples;
        };
        var concatenateNalDataForFrame = function concatenateNalDataForFrame2(frame) {
          var i, currentNal, dataOffset = 0, nalsByteLength = frame.byteLength, numberOfNals = frame.length, totalByteLength = nalsByteLength + 4 * numberOfNals, data = new Uint8Array(totalByteLength), view = new DataView(data.buffer);
          for (i = 0; i < frame.length; i++) {
            currentNal = frame[i];
            view.setUint32(dataOffset, currentNal.data.byteLength);
            dataOffset += 4;
            data.set(currentNal.data, dataOffset);
            dataOffset += currentNal.data.byteLength;
          }
          return data;
        };
        var frameUtils = {
          groupNalsIntoFrames,
          groupFramesIntoGops,
          extendFirstKeyFrame,
          generateSampleTable: generateSampleTable$1,
          concatenateNalData,
          generateSampleTableForFrame,
          concatenateNalDataForFrame
        };
        var highPrefix = [33, 16, 5, 32, 164, 27];
        var lowPrefix = [33, 65, 108, 84, 1, 2, 4, 8, 168, 2, 4, 8, 17, 191, 252];
        var zeroFill = function zeroFill2(count) {
          var a = [];
          while (count--) {
            a.push(0);
          }
          return a;
        };
        var makeTable = function makeTable2(metaTable) {
          return Object.keys(metaTable).reduce(function(obj, key) {
            obj[key] = new Uint8Array(metaTable[key].reduce(function(arr, part) {
              return arr.concat(part);
            }, []));
            return obj;
          }, {});
        };
        var silence;
        var silence_1 = function silence_12() {
          if (!silence) {
            var coneOfSilence = {
              96e3: [highPrefix, [227, 64], zeroFill(154), [56]],
              88200: [highPrefix, [231], zeroFill(170), [56]],
              64e3: [highPrefix, [248, 192], zeroFill(240), [56]],
              48e3: [highPrefix, [255, 192], zeroFill(268), [55, 148, 128], zeroFill(54), [112]],
              44100: [highPrefix, [255, 192], zeroFill(268), [55, 163, 128], zeroFill(84), [112]],
              32e3: [highPrefix, [255, 192], zeroFill(268), [55, 234], zeroFill(226), [112]],
              24e3: [highPrefix, [255, 192], zeroFill(268), [55, 255, 128], zeroFill(268), [111, 112], zeroFill(126), [224]],
              16e3: [highPrefix, [255, 192], zeroFill(268), [55, 255, 128], zeroFill(268), [111, 255], zeroFill(269), [223, 108], zeroFill(195), [1, 192]],
              12e3: [lowPrefix, zeroFill(268), [3, 127, 248], zeroFill(268), [6, 255, 240], zeroFill(268), [13, 255, 224], zeroFill(268), [27, 253, 128], zeroFill(259), [56]],
              11025: [lowPrefix, zeroFill(268), [3, 127, 248], zeroFill(268), [6, 255, 240], zeroFill(268), [13, 255, 224], zeroFill(268), [27, 255, 192], zeroFill(268), [55, 175, 128], zeroFill(108), [112]],
              8e3: [lowPrefix, zeroFill(268), [3, 121, 16], zeroFill(47), [7]]
            };
            silence = makeTable(coneOfSilence);
          }
          return silence;
        };
        var sumFrameByteLengths = function sumFrameByteLengths2(array) {
          var i, currentObj, sum = 0;
          for (i = 0; i < array.length; i++) {
            currentObj = array[i];
            sum += currentObj.data.byteLength;
          }
          return sum;
        };
        var prefixWithSilence = function prefixWithSilence2(track, frames, audioAppendStartTs, videoBaseMediaDecodeTime) {
          var baseMediaDecodeTimeTs, frameDuration = 0, audioGapDuration = 0, audioFillFrameCount = 0, audioFillDuration = 0, silentFrame, i, firstFrame;
          if (!frames.length) {
            return;
          }
          baseMediaDecodeTimeTs = clock.audioTsToVideoTs(track.baseMediaDecodeTime, track.samplerate);
          frameDuration = Math.ceil(clock.ONE_SECOND_IN_TS / (track.samplerate / 1024));
          if (audioAppendStartTs && videoBaseMediaDecodeTime) {
            audioGapDuration = baseMediaDecodeTimeTs - Math.max(audioAppendStartTs, videoBaseMediaDecodeTime);
            audioFillFrameCount = Math.floor(audioGapDuration / frameDuration);
            audioFillDuration = audioFillFrameCount * frameDuration;
          }
          if (audioFillFrameCount < 1 || audioFillDuration > clock.ONE_SECOND_IN_TS / 2) {
            return;
          }
          silentFrame = silence_1()[track.samplerate];
          if (!silentFrame) {
            silentFrame = frames[0].data;
          }
          for (i = 0; i < audioFillFrameCount; i++) {
            firstFrame = frames[0];
            frames.splice(0, 0, {
              data: silentFrame,
              dts: firstFrame.dts - frameDuration,
              pts: firstFrame.pts - frameDuration
            });
          }
          track.baseMediaDecodeTime -= Math.floor(clock.videoTsToAudioTs(audioFillDuration, track.samplerate));
          return audioFillDuration;
        };
        var trimAdtsFramesByEarliestDts = function trimAdtsFramesByEarliestDts2(adtsFrames, track, earliestAllowedDts) {
          if (track.minSegmentDts >= earliestAllowedDts) {
            return adtsFrames;
          }
          track.minSegmentDts = Infinity;
          return adtsFrames.filter(function(currentFrame) {
            if (currentFrame.dts >= earliestAllowedDts) {
              track.minSegmentDts = Math.min(track.minSegmentDts, currentFrame.dts);
              track.minSegmentPts = track.minSegmentDts;
              return true;
            }
            return false;
          });
        };
        var generateSampleTable = function generateSampleTable2(frames) {
          var i, currentFrame, samples = [];
          for (i = 0; i < frames.length; i++) {
            currentFrame = frames[i];
            samples.push({
              size: currentFrame.data.byteLength,
              duration: 1024
            });
          }
          return samples;
        };
        var concatenateFrameData = function concatenateFrameData2(frames) {
          var i, currentFrame, dataOffset = 0, data = new Uint8Array(sumFrameByteLengths(frames));
          for (i = 0; i < frames.length; i++) {
            currentFrame = frames[i];
            data.set(currentFrame.data, dataOffset);
            dataOffset += currentFrame.data.byteLength;
          }
          return data;
        };
        var audioFrameUtils = {
          prefixWithSilence,
          trimAdtsFramesByEarliestDts,
          generateSampleTable,
          concatenateFrameData
        };
        var ONE_SECOND_IN_TS$3 = clock.ONE_SECOND_IN_TS;
        var collectDtsInfo = function collectDtsInfo2(track, data) {
          if (typeof data.pts === "number") {
            if (track.timelineStartInfo.pts === void 0) {
              track.timelineStartInfo.pts = data.pts;
            }
            if (track.minSegmentPts === void 0) {
              track.minSegmentPts = data.pts;
            } else {
              track.minSegmentPts = Math.min(track.minSegmentPts, data.pts);
            }
            if (track.maxSegmentPts === void 0) {
              track.maxSegmentPts = data.pts;
            } else {
              track.maxSegmentPts = Math.max(track.maxSegmentPts, data.pts);
            }
          }
          if (typeof data.dts === "number") {
            if (track.timelineStartInfo.dts === void 0) {
              track.timelineStartInfo.dts = data.dts;
            }
            if (track.minSegmentDts === void 0) {
              track.minSegmentDts = data.dts;
            } else {
              track.minSegmentDts = Math.min(track.minSegmentDts, data.dts);
            }
            if (track.maxSegmentDts === void 0) {
              track.maxSegmentDts = data.dts;
            } else {
              track.maxSegmentDts = Math.max(track.maxSegmentDts, data.dts);
            }
          }
        };
        var clearDtsInfo = function clearDtsInfo2(track) {
          delete track.minSegmentDts;
          delete track.maxSegmentDts;
          delete track.minSegmentPts;
          delete track.maxSegmentPts;
        };
        var calculateTrackBaseMediaDecodeTime = function calculateTrackBaseMediaDecodeTime2(track, keepOriginalTimestamps) {
          var baseMediaDecodeTime, scale, minSegmentDts = track.minSegmentDts;
          if (!keepOriginalTimestamps) {
            minSegmentDts -= track.timelineStartInfo.dts;
          }
          baseMediaDecodeTime = track.timelineStartInfo.baseMediaDecodeTime;
          baseMediaDecodeTime += minSegmentDts;
          baseMediaDecodeTime = Math.max(0, baseMediaDecodeTime);
          if (track.type === "audio") {
            scale = track.samplerate / ONE_SECOND_IN_TS$3;
            baseMediaDecodeTime *= scale;
            baseMediaDecodeTime = Math.floor(baseMediaDecodeTime);
          }
          return baseMediaDecodeTime;
        };
        var trackDecodeInfo = {
          clearDtsInfo,
          calculateTrackBaseMediaDecodeTime,
          collectDtsInfo
        };
        var USER_DATA_REGISTERED_ITU_T_T35 = 4, RBSP_TRAILING_BITS = 128;
        var parseSei = function parseSei2(bytes) {
          var i = 0, result = {
            payloadType: -1,
            payloadSize: 0
          }, payloadType = 0, payloadSize = 0;
          while (i < bytes.byteLength) {
            if (bytes[i] === RBSP_TRAILING_BITS) {
              break;
            }
            while (bytes[i] === 255) {
              payloadType += 255;
              i++;
            }
            payloadType += bytes[i++];
            while (bytes[i] === 255) {
              payloadSize += 255;
              i++;
            }
            payloadSize += bytes[i++];
            if (!result.payload && payloadType === USER_DATA_REGISTERED_ITU_T_T35) {
              var userIdentifier = String.fromCharCode(bytes[i + 3], bytes[i + 4], bytes[i + 5], bytes[i + 6]);
              if (userIdentifier === "GA94") {
                result.payloadType = payloadType;
                result.payloadSize = payloadSize;
                result.payload = bytes.subarray(i, i + payloadSize);
                break;
              } else {
                result.payload = void 0;
              }
            }
            i += payloadSize;
            payloadType = 0;
            payloadSize = 0;
          }
          return result;
        };
        var parseUserData = function parseUserData2(sei) {
          if (sei.payload[0] !== 181) {
            return null;
          }
          if ((sei.payload[1] << 8 | sei.payload[2]) !== 49) {
            return null;
          }
          if (String.fromCharCode(sei.payload[3], sei.payload[4], sei.payload[5], sei.payload[6]) !== "GA94") {
            return null;
          }
          if (sei.payload[7] !== 3) {
            return null;
          }
          return sei.payload.subarray(8, sei.payload.length - 1);
        };
        var parseCaptionPackets = function parseCaptionPackets2(pts, userData) {
          var results = [], i, count, offset, data;
          if (!(userData[0] & 64)) {
            return results;
          }
          count = userData[0] & 31;
          for (i = 0; i < count; i++) {
            offset = i * 3;
            data = {
              type: userData[offset + 2] & 3,
              pts
            };
            if (userData[offset + 2] & 4) {
              data.ccData = userData[offset + 3] << 8 | userData[offset + 4];
              results.push(data);
            }
          }
          return results;
        };
        var discardEmulationPreventionBytes$1 = function discardEmulationPreventionBytes2(data) {
          var length = data.byteLength, emulationPreventionBytesPositions = [], i = 1, newLength, newData;
          while (i < length - 2) {
            if (data[i] === 0 && data[i + 1] === 0 && data[i + 2] === 3) {
              emulationPreventionBytesPositions.push(i + 2);
              i += 2;
            } else {
              i++;
            }
          }
          if (emulationPreventionBytesPositions.length === 0) {
            return data;
          }
          newLength = length - emulationPreventionBytesPositions.length;
          newData = new Uint8Array(newLength);
          var sourceIndex = 0;
          for (i = 0; i < newLength; sourceIndex++, i++) {
            if (sourceIndex === emulationPreventionBytesPositions[0]) {
              sourceIndex++;
              emulationPreventionBytesPositions.shift();
            }
            newData[i] = data[sourceIndex];
          }
          return newData;
        };
        var captionPacketParser = {
          parseSei,
          parseUserData,
          parseCaptionPackets,
          discardEmulationPreventionBytes: discardEmulationPreventionBytes$1,
          USER_DATA_REGISTERED_ITU_T_T35
        };
        var CaptionStream$1 = function CaptionStream2(options) {
          options = options || {};
          CaptionStream2.prototype.init.call(this);
          this.parse708captions_ = typeof options.parse708captions === "boolean" ? options.parse708captions : true;
          this.captionPackets_ = [];
          this.ccStreams_ = [
            new Cea608Stream(0, 0),
            new Cea608Stream(0, 1),
            new Cea608Stream(1, 0),
            new Cea608Stream(1, 1)
          ];
          if (this.parse708captions_) {
            this.cc708Stream_ = new Cea708Stream({
              captionServices: options.captionServices
            });
          }
          this.reset();
          this.ccStreams_.forEach(function(cc) {
            cc.on("data", this.trigger.bind(this, "data"));
            cc.on("partialdone", this.trigger.bind(this, "partialdone"));
            cc.on("done", this.trigger.bind(this, "done"));
          }, this);
          if (this.parse708captions_) {
            this.cc708Stream_.on("data", this.trigger.bind(this, "data"));
            this.cc708Stream_.on("partialdone", this.trigger.bind(this, "partialdone"));
            this.cc708Stream_.on("done", this.trigger.bind(this, "done"));
          }
        };
        CaptionStream$1.prototype = new stream();
        CaptionStream$1.prototype.push = function(event) {
          var sei, userData, newCaptionPackets;
          if (event.nalUnitType !== "sei_rbsp") {
            return;
          }
          sei = captionPacketParser.parseSei(event.escapedRBSP);
          if (!sei.payload) {
            return;
          }
          if (sei.payloadType !== captionPacketParser.USER_DATA_REGISTERED_ITU_T_T35) {
            return;
          }
          userData = captionPacketParser.parseUserData(sei);
          if (!userData) {
            return;
          }
          if (event.dts < this.latestDts_) {
            this.ignoreNextEqualDts_ = true;
            return;
          } else if (event.dts === this.latestDts_ && this.ignoreNextEqualDts_) {
            this.numSameDts_--;
            if (!this.numSameDts_) {
              this.ignoreNextEqualDts_ = false;
            }
            return;
          }
          newCaptionPackets = captionPacketParser.parseCaptionPackets(event.pts, userData);
          this.captionPackets_ = this.captionPackets_.concat(newCaptionPackets);
          if (this.latestDts_ !== event.dts) {
            this.numSameDts_ = 0;
          }
          this.numSameDts_++;
          this.latestDts_ = event.dts;
        };
        CaptionStream$1.prototype.flushCCStreams = function(flushType) {
          this.ccStreams_.forEach(function(cc) {
            return flushType === "flush" ? cc.flush() : cc.partialFlush();
          }, this);
        };
        CaptionStream$1.prototype.flushStream = function(flushType) {
          if (!this.captionPackets_.length) {
            this.flushCCStreams(flushType);
            return;
          }
          this.captionPackets_.forEach(function(elem, idx) {
            elem.presortIndex = idx;
          });
          this.captionPackets_.sort(function(a, b) {
            if (a.pts === b.pts) {
              return a.presortIndex - b.presortIndex;
            }
            return a.pts - b.pts;
          });
          this.captionPackets_.forEach(function(packet) {
            if (packet.type < 2) {
              this.dispatchCea608Packet(packet);
            } else {
              this.dispatchCea708Packet(packet);
            }
          }, this);
          this.captionPackets_.length = 0;
          this.flushCCStreams(flushType);
        };
        CaptionStream$1.prototype.flush = function() {
          return this.flushStream("flush");
        };
        CaptionStream$1.prototype.partialFlush = function() {
          return this.flushStream("partialFlush");
        };
        CaptionStream$1.prototype.reset = function() {
          this.latestDts_ = null;
          this.ignoreNextEqualDts_ = false;
          this.numSameDts_ = 0;
          this.activeCea608Channel_ = [null, null];
          this.ccStreams_.forEach(function(ccStream) {
            ccStream.reset();
          });
        };
        CaptionStream$1.prototype.dispatchCea608Packet = function(packet) {
          if (this.setsTextOrXDSActive(packet)) {
            this.activeCea608Channel_[packet.type] = null;
          } else if (this.setsChannel1Active(packet)) {
            this.activeCea608Channel_[packet.type] = 0;
          } else if (this.setsChannel2Active(packet)) {
            this.activeCea608Channel_[packet.type] = 1;
          }
          if (this.activeCea608Channel_[packet.type] === null) {
            return;
          }
          this.ccStreams_[(packet.type << 1) + this.activeCea608Channel_[packet.type]].push(packet);
        };
        CaptionStream$1.prototype.setsChannel1Active = function(packet) {
          return (packet.ccData & 30720) === 4096;
        };
        CaptionStream$1.prototype.setsChannel2Active = function(packet) {
          return (packet.ccData & 30720) === 6144;
        };
        CaptionStream$1.prototype.setsTextOrXDSActive = function(packet) {
          return (packet.ccData & 28928) === 256 || (packet.ccData & 30974) === 4138 || (packet.ccData & 30974) === 6186;
        };
        CaptionStream$1.prototype.dispatchCea708Packet = function(packet) {
          if (this.parse708captions_) {
            this.cc708Stream_.push(packet);
          }
        };
        var CHARACTER_TRANSLATION_708 = {
          127: 9834,
          4128: 32,
          4129: 160,
          4133: 8230,
          4138: 352,
          4140: 338,
          4144: 9608,
          4145: 8216,
          4146: 8217,
          4147: 8220,
          4148: 8221,
          4149: 8226,
          4153: 8482,
          4154: 353,
          4156: 339,
          4157: 8480,
          4159: 376,
          4214: 8539,
          4215: 8540,
          4216: 8541,
          4217: 8542,
          4218: 9168,
          4219: 9124,
          4220: 9123,
          4221: 9135,
          4222: 9126,
          4223: 9121,
          4256: 12600
        };
        var get708CharFromCode = function get708CharFromCode2(code) {
          var newCode = CHARACTER_TRANSLATION_708[code] || code;
          if (code & 4096 && code === newCode) {
            return "";
          }
          return String.fromCharCode(newCode);
        };
        var within708TextBlock = function within708TextBlock2(b) {
          return 32 <= b && b <= 127 || 160 <= b && b <= 255;
        };
        var Cea708Window = function Cea708Window2(windowNum) {
          this.windowNum = windowNum;
          this.reset();
        };
        Cea708Window.prototype.reset = function() {
          this.clearText();
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
        };
        Cea708Window.prototype.getText = function() {
          return this.rows.join("\n");
        };
        Cea708Window.prototype.clearText = function() {
          this.rows = [""];
          this.rowIdx = 0;
        };
        Cea708Window.prototype.newLine = function(pts) {
          if (this.rows.length >= this.virtualRowCount && typeof this.beforeRowOverflow === "function") {
            this.beforeRowOverflow(pts);
          }
          if (this.rows.length > 0) {
            this.rows.push("");
            this.rowIdx++;
          }
          while (this.rows.length > this.virtualRowCount) {
            this.rows.shift();
            this.rowIdx--;
          }
        };
        Cea708Window.prototype.isEmpty = function() {
          if (this.rows.length === 0) {
            return true;
          } else if (this.rows.length === 1) {
            return this.rows[0] === "";
          }
          return false;
        };
        Cea708Window.prototype.addText = function(text) {
          this.rows[this.rowIdx] += text;
        };
        Cea708Window.prototype.backspace = function() {
          if (!this.isEmpty()) {
            var row = this.rows[this.rowIdx];
            this.rows[this.rowIdx] = row.substr(0, row.length - 1);
          }
        };
        var Cea708Service = function Cea708Service2(serviceNum, encoding, stream2) {
          this.serviceNum = serviceNum;
          this.text = "";
          this.currentWindow = new Cea708Window(-1);
          this.windows = [];
          this.stream = stream2;
          if (typeof encoding === "string") {
            this.createTextDecoder(encoding);
          }
        };
        Cea708Service.prototype.init = function(pts, beforeRowOverflow) {
          this.startPts = pts;
          for (var win = 0; win < 8; win++) {
            this.windows[win] = new Cea708Window(win);
            if (typeof beforeRowOverflow === "function") {
              this.windows[win].beforeRowOverflow = beforeRowOverflow;
            }
          }
        };
        Cea708Service.prototype.setCurrentWindow = function(windowNum) {
          this.currentWindow = this.windows[windowNum];
        };
        Cea708Service.prototype.createTextDecoder = function(encoding) {
          if (typeof TextDecoder === "undefined") {
            this.stream.trigger("log", {
              level: "warn",
              message: "The `encoding` option is unsupported without TextDecoder support"
            });
          } else {
            try {
              this.textDecoder_ = new TextDecoder(encoding);
            } catch (error) {
              this.stream.trigger("log", {
                level: "warn",
                message: "TextDecoder could not be created with " + encoding + " encoding. " + error
              });
            }
          }
        };
        var Cea708Stream = function Cea708Stream2(options) {
          options = options || {};
          Cea708Stream2.prototype.init.call(this);
          var self2 = this;
          var captionServices = options.captionServices || {};
          var captionServiceEncodings = {};
          var serviceProps;
          Object.keys(captionServices).forEach(function(serviceName) {
            serviceProps = captionServices[serviceName];
            if (/^SERVICE/.test(serviceName)) {
              captionServiceEncodings[serviceName] = serviceProps.encoding;
            }
          });
          this.serviceEncodings = captionServiceEncodings;
          this.current708Packet = null;
          this.services = {};
          this.push = function(packet) {
            if (packet.type === 3) {
              self2.new708Packet();
              self2.add708Bytes(packet);
            } else {
              if (self2.current708Packet === null) {
                self2.new708Packet();
              }
              self2.add708Bytes(packet);
            }
          };
        };
        Cea708Stream.prototype = new stream();
        Cea708Stream.prototype.new708Packet = function() {
          if (this.current708Packet !== null) {
            this.push708Packet();
          }
          this.current708Packet = {
            data: [],
            ptsVals: []
          };
        };
        Cea708Stream.prototype.add708Bytes = function(packet) {
          var data = packet.ccData;
          var byte0 = data >>> 8;
          var byte1 = data & 255;
          this.current708Packet.ptsVals.push(packet.pts);
          this.current708Packet.data.push(byte0);
          this.current708Packet.data.push(byte1);
        };
        Cea708Stream.prototype.push708Packet = function() {
          var packet708 = this.current708Packet;
          var packetData = packet708.data;
          var serviceNum = null;
          var blockSize = null;
          var i = 0;
          var b = packetData[i++];
          packet708.seq = b >> 6;
          packet708.sizeCode = b & 63;
          for (; i < packetData.length; i++) {
            b = packetData[i++];
            serviceNum = b >> 5;
            blockSize = b & 31;
            if (serviceNum === 7 && blockSize > 0) {
              b = packetData[i++];
              serviceNum = b;
            }
            this.pushServiceBlock(serviceNum, i, blockSize);
            if (blockSize > 0) {
              i += blockSize - 1;
            }
          }
        };
        Cea708Stream.prototype.pushServiceBlock = function(serviceNum, start, size) {
          var b;
          var i = start;
          var packetData = this.current708Packet.data;
          var service = this.services[serviceNum];
          if (!service) {
            service = this.initService(serviceNum, i);
          }
          for (; i < start + size && i < packetData.length; i++) {
            b = packetData[i];
            if (within708TextBlock(b)) {
              i = this.handleText(i, service);
            } else if (b === 24) {
              i = this.multiByteCharacter(i, service);
            } else if (b === 16) {
              i = this.extendedCommands(i, service);
            } else if (128 <= b && b <= 135) {
              i = this.setCurrentWindow(i, service);
            } else if (152 <= b && b <= 159) {
              i = this.defineWindow(i, service);
            } else if (b === 136) {
              i = this.clearWindows(i, service);
            } else if (b === 140) {
              i = this.deleteWindows(i, service);
            } else if (b === 137) {
              i = this.displayWindows(i, service);
            } else if (b === 138) {
              i = this.hideWindows(i, service);
            } else if (b === 139) {
              i = this.toggleWindows(i, service);
            } else if (b === 151) {
              i = this.setWindowAttributes(i, service);
            } else if (b === 144) {
              i = this.setPenAttributes(i, service);
            } else if (b === 145) {
              i = this.setPenColor(i, service);
            } else if (b === 146) {
              i = this.setPenLocation(i, service);
            } else if (b === 143) {
              service = this.reset(i, service);
            } else if (b === 8) {
              service.currentWindow.backspace();
            } else if (b === 12) {
              service.currentWindow.clearText();
            } else if (b === 13) {
              service.currentWindow.pendingNewLine = true;
            } else if (b === 14) {
              service.currentWindow.clearText();
            } else if (b === 141) {
              i++;
            } else
              ;
          }
        };
        Cea708Stream.prototype.extendedCommands = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[++i];
          if (within708TextBlock(b)) {
            i = this.handleText(i, service, {
              isExtended: true
            });
          }
          return i;
        };
        Cea708Stream.prototype.getPts = function(byteIndex) {
          return this.current708Packet.ptsVals[Math.floor(byteIndex / 2)];
        };
        Cea708Stream.prototype.initService = function(serviceNum, i) {
          var serviceName = "SERVICE" + serviceNum;
          var self2 = this;
          var serviceName;
          var encoding;
          if (serviceName in this.serviceEncodings) {
            encoding = this.serviceEncodings[serviceName];
          }
          this.services[serviceNum] = new Cea708Service(serviceNum, encoding, self2);
          this.services[serviceNum].init(this.getPts(i), function(pts) {
            self2.flushDisplayed(pts, self2.services[serviceNum]);
          });
          return this.services[serviceNum];
        };
        Cea708Stream.prototype.handleText = function(i, service, options) {
          var isExtended = options && options.isExtended;
          var isMultiByte = options && options.isMultiByte;
          var packetData = this.current708Packet.data;
          var extended = isExtended ? 4096 : 0;
          var currentByte = packetData[i];
          var nextByte = packetData[i + 1];
          var win = service.currentWindow;
          var char;
          var charCodeArray;
          if (service.textDecoder_ && !isExtended) {
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
        };
        Cea708Stream.prototype.multiByteCharacter = function(i, service) {
          var packetData = this.current708Packet.data;
          var firstByte = packetData[i + 1];
          var secondByte = packetData[i + 2];
          if (within708TextBlock(firstByte) && within708TextBlock(secondByte)) {
            i = this.handleText(++i, service, {
              isMultiByte: true
            });
          }
          return i;
        };
        Cea708Stream.prototype.setCurrentWindow = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[i];
          var windowNum = b & 7;
          service.setCurrentWindow(windowNum);
          return i;
        };
        Cea708Stream.prototype.defineWindow = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[i];
          var windowNum = b & 7;
          service.setCurrentWindow(windowNum);
          var win = service.currentWindow;
          b = packetData[++i];
          win.visible = (b & 32) >> 5;
          win.rowLock = (b & 16) >> 4;
          win.columnLock = (b & 8) >> 3;
          win.priority = b & 7;
          b = packetData[++i];
          win.relativePositioning = (b & 128) >> 7;
          win.anchorVertical = b & 127;
          b = packetData[++i];
          win.anchorHorizontal = b;
          b = packetData[++i];
          win.anchorPoint = (b & 240) >> 4;
          win.rowCount = b & 15;
          b = packetData[++i];
          win.columnCount = b & 63;
          b = packetData[++i];
          win.windowStyle = (b & 56) >> 3;
          win.penStyle = b & 7;
          win.virtualRowCount = win.rowCount + 1;
          return i;
        };
        Cea708Stream.prototype.setWindowAttributes = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[i];
          var winAttr = service.currentWindow.winAttr;
          b = packetData[++i];
          winAttr.fillOpacity = (b & 192) >> 6;
          winAttr.fillRed = (b & 48) >> 4;
          winAttr.fillGreen = (b & 12) >> 2;
          winAttr.fillBlue = b & 3;
          b = packetData[++i];
          winAttr.borderType = (b & 192) >> 6;
          winAttr.borderRed = (b & 48) >> 4;
          winAttr.borderGreen = (b & 12) >> 2;
          winAttr.borderBlue = b & 3;
          b = packetData[++i];
          winAttr.borderType += (b & 128) >> 5;
          winAttr.wordWrap = (b & 64) >> 6;
          winAttr.printDirection = (b & 48) >> 4;
          winAttr.scrollDirection = (b & 12) >> 2;
          winAttr.justify = b & 3;
          b = packetData[++i];
          winAttr.effectSpeed = (b & 240) >> 4;
          winAttr.effectDirection = (b & 12) >> 2;
          winAttr.displayEffect = b & 3;
          return i;
        };
        Cea708Stream.prototype.flushDisplayed = function(pts, service) {
          var displayedText = [];
          for (var winId = 0; winId < 8; winId++) {
            if (service.windows[winId].visible && !service.windows[winId].isEmpty()) {
              displayedText.push(service.windows[winId].getText());
            }
          }
          service.endPts = pts;
          service.text = displayedText.join("\n\n");
          this.pushCaption(service);
          service.startPts = pts;
        };
        Cea708Stream.prototype.pushCaption = function(service) {
          if (service.text !== "") {
            this.trigger("data", {
              startPts: service.startPts,
              endPts: service.endPts,
              text: service.text,
              stream: "cc708_" + service.serviceNum
            });
            service.text = "";
            service.startPts = service.endPts;
          }
        };
        Cea708Stream.prototype.displayWindows = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[++i];
          var pts = this.getPts(i);
          this.flushDisplayed(pts, service);
          for (var winId = 0; winId < 8; winId++) {
            if (b & 1 << winId) {
              service.windows[winId].visible = 1;
            }
          }
          return i;
        };
        Cea708Stream.prototype.hideWindows = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[++i];
          var pts = this.getPts(i);
          this.flushDisplayed(pts, service);
          for (var winId = 0; winId < 8; winId++) {
            if (b & 1 << winId) {
              service.windows[winId].visible = 0;
            }
          }
          return i;
        };
        Cea708Stream.prototype.toggleWindows = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[++i];
          var pts = this.getPts(i);
          this.flushDisplayed(pts, service);
          for (var winId = 0; winId < 8; winId++) {
            if (b & 1 << winId) {
              service.windows[winId].visible ^= 1;
            }
          }
          return i;
        };
        Cea708Stream.prototype.clearWindows = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[++i];
          var pts = this.getPts(i);
          this.flushDisplayed(pts, service);
          for (var winId = 0; winId < 8; winId++) {
            if (b & 1 << winId) {
              service.windows[winId].clearText();
            }
          }
          return i;
        };
        Cea708Stream.prototype.deleteWindows = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[++i];
          var pts = this.getPts(i);
          this.flushDisplayed(pts, service);
          for (var winId = 0; winId < 8; winId++) {
            if (b & 1 << winId) {
              service.windows[winId].reset();
            }
          }
          return i;
        };
        Cea708Stream.prototype.setPenAttributes = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[i];
          var penAttr = service.currentWindow.penAttr;
          b = packetData[++i];
          penAttr.textTag = (b & 240) >> 4;
          penAttr.offset = (b & 12) >> 2;
          penAttr.penSize = b & 3;
          b = packetData[++i];
          penAttr.italics = (b & 128) >> 7;
          penAttr.underline = (b & 64) >> 6;
          penAttr.edgeType = (b & 56) >> 3;
          penAttr.fontStyle = b & 7;
          return i;
        };
        Cea708Stream.prototype.setPenColor = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[i];
          var penColor = service.currentWindow.penColor;
          b = packetData[++i];
          penColor.fgOpacity = (b & 192) >> 6;
          penColor.fgRed = (b & 48) >> 4;
          penColor.fgGreen = (b & 12) >> 2;
          penColor.fgBlue = b & 3;
          b = packetData[++i];
          penColor.bgOpacity = (b & 192) >> 6;
          penColor.bgRed = (b & 48) >> 4;
          penColor.bgGreen = (b & 12) >> 2;
          penColor.bgBlue = b & 3;
          b = packetData[++i];
          penColor.edgeRed = (b & 48) >> 4;
          penColor.edgeGreen = (b & 12) >> 2;
          penColor.edgeBlue = b & 3;
          return i;
        };
        Cea708Stream.prototype.setPenLocation = function(i, service) {
          var packetData = this.current708Packet.data;
          var b = packetData[i];
          var penLoc = service.currentWindow.penLoc;
          service.currentWindow.pendingNewLine = true;
          b = packetData[++i];
          penLoc.row = b & 15;
          b = packetData[++i];
          penLoc.column = b & 63;
          return i;
        };
        Cea708Stream.prototype.reset = function(i, service) {
          var pts = this.getPts(i);
          this.flushDisplayed(pts, service);
          return this.initService(service.serviceNum, i);
        };
        var CHARACTER_TRANSLATION = {
          42: 225,
          92: 233,
          94: 237,
          95: 243,
          96: 250,
          123: 231,
          124: 247,
          125: 209,
          126: 241,
          127: 9608,
          304: 174,
          305: 176,
          306: 189,
          307: 191,
          308: 8482,
          309: 162,
          310: 163,
          311: 9834,
          312: 224,
          313: 160,
          314: 232,
          315: 226,
          316: 234,
          317: 238,
          318: 244,
          319: 251,
          544: 193,
          545: 201,
          546: 211,
          547: 218,
          548: 220,
          549: 252,
          550: 8216,
          551: 161,
          552: 42,
          553: 39,
          554: 8212,
          555: 169,
          556: 8480,
          557: 8226,
          558: 8220,
          559: 8221,
          560: 192,
          561: 194,
          562: 199,
          563: 200,
          564: 202,
          565: 203,
          566: 235,
          567: 206,
          568: 207,
          569: 239,
          570: 212,
          571: 217,
          572: 249,
          573: 219,
          574: 171,
          575: 187,
          800: 195,
          801: 227,
          802: 205,
          803: 204,
          804: 236,
          805: 210,
          806: 242,
          807: 213,
          808: 245,
          809: 123,
          810: 125,
          811: 92,
          812: 94,
          813: 95,
          814: 124,
          815: 126,
          816: 196,
          817: 228,
          818: 214,
          819: 246,
          820: 223,
          821: 165,
          822: 164,
          823: 9474,
          824: 197,
          825: 229,
          826: 216,
          827: 248,
          828: 9484,
          829: 9488,
          830: 9492,
          831: 9496
        };
        var getCharFromCode = function getCharFromCode2(code) {
          if (code === null) {
            return "";
          }
          code = CHARACTER_TRANSLATION[code] || code;
          return String.fromCharCode(code);
        };
        var BOTTOM_ROW = 14;
        var ROWS = [4352, 4384, 4608, 4640, 5376, 5408, 5632, 5664, 5888, 5920, 4096, 4864, 4896, 5120, 5152];
        var createDisplayBuffer = function createDisplayBuffer2() {
          var result = [], i = BOTTOM_ROW + 1;
          while (i--) {
            result.push("");
          }
          return result;
        };
        var Cea608Stream = function Cea608Stream2(field, dataChannel) {
          Cea608Stream2.prototype.init.call(this);
          this.field_ = field || 0;
          this.dataChannel_ = dataChannel || 0;
          this.name_ = "CC" + ((this.field_ << 1 | this.dataChannel_) + 1);
          this.setConstants();
          this.reset();
          this.push = function(packet) {
            var data, swap, char0, char1, text;
            data = packet.ccData & 32639;
            if (data === this.lastControlCode_) {
              this.lastControlCode_ = null;
              return;
            }
            if ((data & 61440) === 4096) {
              this.lastControlCode_ = data;
            } else if (data !== this.PADDING_) {
              this.lastControlCode_ = null;
            }
            char0 = data >>> 8;
            char1 = data & 255;
            if (data === this.PADDING_) {
              return;
            } else if (data === this.RESUME_CAPTION_LOADING_) {
              this.mode_ = "popOn";
            } else if (data === this.END_OF_CAPTION_) {
              this.mode_ = "popOn";
              this.clearFormatting(packet.pts);
              this.flushDisplayed(packet.pts);
              swap = this.displayed_;
              this.displayed_ = this.nonDisplayed_;
              this.nonDisplayed_ = swap;
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
                this.nonDisplayed_[this.row_] = this.nonDisplayed_[this.row_].slice(0, -1);
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
                this.flushDisplayed(packet.pts);
                this.displayed_ = createDisplayBuffer();
              }
              this.mode_ = "paintOn";
              this.startPts_ = packet.pts;
            } else if (this.isSpecialCharacter(char0, char1)) {
              char0 = (char0 & 3) << 8;
              text = getCharFromCode(char0 | char1);
              this[this.mode_](packet.pts, text);
              this.column_++;
            } else if (this.isExtCharacter(char0, char1)) {
              if (this.mode_ === "popOn") {
                this.nonDisplayed_[this.row_] = this.nonDisplayed_[this.row_].slice(0, -1);
              } else {
                this.displayed_[this.row_] = this.displayed_[this.row_].slice(0, -1);
              }
              char0 = (char0 & 3) << 8;
              text = getCharFromCode(char0 | char1);
              this[this.mode_](packet.pts, text);
              this.column_++;
            } else if (this.isMidRowCode(char0, char1)) {
              this.clearFormatting(packet.pts);
              this[this.mode_](packet.pts, " ");
              this.column_++;
              if ((char1 & 14) === 14) {
                this.addFormatting(packet.pts, ["i"]);
              }
              if ((char1 & 1) === 1) {
                this.addFormatting(packet.pts, ["u"]);
              }
            } else if (this.isOffsetControlCode(char0, char1)) {
              this.column_ += char1 & 3;
            } else if (this.isPAC(char0, char1)) {
              var row = ROWS.indexOf(data & 7968);
              if (this.mode_ === "rollUp") {
                if (row - this.rollUpRows_ + 1 < 0) {
                  row = this.rollUpRows_ - 1;
                }
                this.setRollUp(packet.pts, row);
              }
              if (row !== this.row_) {
                this.clearFormatting(packet.pts);
                this.row_ = row;
              }
              if (char1 & 1 && this.formatting_.indexOf("u") === -1) {
                this.addFormatting(packet.pts, ["u"]);
              }
              if ((data & 16) === 16) {
                this.column_ = ((data & 14) >> 1) * 4;
              }
              if (this.isColorPAC(char1)) {
                if ((char1 & 14) === 14) {
                  this.addFormatting(packet.pts, ["i"]);
                }
              }
            } else if (this.isNormalChar(char0)) {
              if (char1 === 0) {
                char1 = null;
              }
              text = getCharFromCode(char0);
              text += getCharFromCode(char1);
              this[this.mode_](packet.pts, text);
              this.column_ += text.length;
            }
          };
        };
        Cea608Stream.prototype = new stream();
        Cea608Stream.prototype.flushDisplayed = function(pts) {
          var content = this.displayed_.map(function(row, index) {
            try {
              return row.trim();
            } catch (e) {
              this.trigger("log", {
                level: "warn",
                message: "Skipping a malformed 608 caption at index " + index + "."
              });
              return "";
            }
          }, this).join("\n").replace(/^\n+|\n+$/g, "");
          if (content.length) {
            this.trigger("data", {
              startPts: this.startPts_,
              endPts: pts,
              text: content,
              stream: this.name_
            });
          }
        };
        Cea608Stream.prototype.reset = function() {
          this.mode_ = "popOn";
          this.topRow_ = 0;
          this.startPts_ = 0;
          this.displayed_ = createDisplayBuffer();
          this.nonDisplayed_ = createDisplayBuffer();
          this.lastControlCode_ = null;
          this.column_ = 0;
          this.row_ = BOTTOM_ROW;
          this.rollUpRows_ = 2;
          this.formatting_ = [];
        };
        Cea608Stream.prototype.setConstants = function() {
          if (this.dataChannel_ === 0) {
            this.BASE_ = 16;
            this.EXT_ = 17;
            this.CONTROL_ = (20 | this.field_) << 8;
            this.OFFSET_ = 23;
          } else if (this.dataChannel_ === 1) {
            this.BASE_ = 24;
            this.EXT_ = 25;
            this.CONTROL_ = (28 | this.field_) << 8;
            this.OFFSET_ = 31;
          }
          this.PADDING_ = 0;
          this.RESUME_CAPTION_LOADING_ = this.CONTROL_ | 32;
          this.END_OF_CAPTION_ = this.CONTROL_ | 47;
          this.ROLL_UP_2_ROWS_ = this.CONTROL_ | 37;
          this.ROLL_UP_3_ROWS_ = this.CONTROL_ | 38;
          this.ROLL_UP_4_ROWS_ = this.CONTROL_ | 39;
          this.CARRIAGE_RETURN_ = this.CONTROL_ | 45;
          this.RESUME_DIRECT_CAPTIONING_ = this.CONTROL_ | 41;
          this.BACKSPACE_ = this.CONTROL_ | 33;
          this.ERASE_DISPLAYED_MEMORY_ = this.CONTROL_ | 44;
          this.ERASE_NON_DISPLAYED_MEMORY_ = this.CONTROL_ | 46;
        };
        Cea608Stream.prototype.isSpecialCharacter = function(char0, char1) {
          return char0 === this.EXT_ && char1 >= 48 && char1 <= 63;
        };
        Cea608Stream.prototype.isExtCharacter = function(char0, char1) {
          return (char0 === this.EXT_ + 1 || char0 === this.EXT_ + 2) && char1 >= 32 && char1 <= 63;
        };
        Cea608Stream.prototype.isMidRowCode = function(char0, char1) {
          return char0 === this.EXT_ && char1 >= 32 && char1 <= 47;
        };
        Cea608Stream.prototype.isOffsetControlCode = function(char0, char1) {
          return char0 === this.OFFSET_ && char1 >= 33 && char1 <= 35;
        };
        Cea608Stream.prototype.isPAC = function(char0, char1) {
          return char0 >= this.BASE_ && char0 < this.BASE_ + 8 && char1 >= 64 && char1 <= 127;
        };
        Cea608Stream.prototype.isColorPAC = function(char1) {
          return char1 >= 64 && char1 <= 79 || char1 >= 96 && char1 <= 127;
        };
        Cea608Stream.prototype.isNormalChar = function(char) {
          return char >= 32 && char <= 127;
        };
        Cea608Stream.prototype.setRollUp = function(pts, newBaseRow) {
          if (this.mode_ !== "rollUp") {
            this.row_ = BOTTOM_ROW;
            this.mode_ = "rollUp";
            this.flushDisplayed(pts);
            this.nonDisplayed_ = createDisplayBuffer();
            this.displayed_ = createDisplayBuffer();
          }
          if (newBaseRow !== void 0 && newBaseRow !== this.row_) {
            for (var i = 0; i < this.rollUpRows_; i++) {
              this.displayed_[newBaseRow - i] = this.displayed_[this.row_ - i];
              this.displayed_[this.row_ - i] = "";
            }
          }
          if (newBaseRow === void 0) {
            newBaseRow = this.row_;
          }
          this.topRow_ = newBaseRow - this.rollUpRows_ + 1;
        };
        Cea608Stream.prototype.addFormatting = function(pts, format) {
          this.formatting_ = this.formatting_.concat(format);
          var text = format.reduce(function(text2, format2) {
            return text2 + "<" + format2 + ">";
          }, "");
          this[this.mode_](pts, text);
        };
        Cea608Stream.prototype.clearFormatting = function(pts) {
          if (!this.formatting_.length) {
            return;
          }
          var text = this.formatting_.reverse().reduce(function(text2, format) {
            return text2 + "</" + format + ">";
          }, "");
          this.formatting_ = [];
          this[this.mode_](pts, text);
        };
        Cea608Stream.prototype.popOn = function(pts, text) {
          var baseRow = this.nonDisplayed_[this.row_];
          baseRow += text;
          this.nonDisplayed_[this.row_] = baseRow;
        };
        Cea608Stream.prototype.rollUp = function(pts, text) {
          var baseRow = this.displayed_[this.row_];
          baseRow += text;
          this.displayed_[this.row_] = baseRow;
        };
        Cea608Stream.prototype.shiftRowsUp_ = function() {
          var i;
          for (i = 0; i < this.topRow_; i++) {
            this.displayed_[i] = "";
          }
          for (i = this.row_ + 1; i < BOTTOM_ROW + 1; i++) {
            this.displayed_[i] = "";
          }
          for (i = this.topRow_; i < this.row_; i++) {
            this.displayed_[i] = this.displayed_[i + 1];
          }
          this.displayed_[this.row_] = "";
        };
        Cea608Stream.prototype.paintOn = function(pts, text) {
          var baseRow = this.displayed_[this.row_];
          baseRow += text;
          this.displayed_[this.row_] = baseRow;
        };
        var captionStream = {
          CaptionStream: CaptionStream$1,
          Cea608Stream,
          Cea708Stream
        };
        var streamTypes = {
          H264_STREAM_TYPE: 27,
          ADTS_STREAM_TYPE: 15,
          METADATA_STREAM_TYPE: 21
        };
        var MAX_TS = 8589934592;
        var RO_THRESH = 4294967296;
        var TYPE_SHARED = "shared";
        var handleRollover$1 = function handleRollover2(value, reference) {
          var direction = 1;
          if (value > reference) {
            direction = -1;
          }
          while (Math.abs(reference - value) > RO_THRESH) {
            value += direction * MAX_TS;
          }
          return value;
        };
        var TimestampRolloverStream$1 = function TimestampRolloverStream2(type2) {
          var lastDTS, referenceDTS;
          TimestampRolloverStream2.prototype.init.call(this);
          this.type_ = type2 || TYPE_SHARED;
          this.push = function(data) {
            if (this.type_ !== TYPE_SHARED && data.type !== this.type_) {
              return;
            }
            if (referenceDTS === void 0) {
              referenceDTS = data.dts;
            }
            data.dts = handleRollover$1(data.dts, referenceDTS);
            data.pts = handleRollover$1(data.pts, referenceDTS);
            lastDTS = data.dts;
            this.trigger("data", data);
          };
          this.flush = function() {
            referenceDTS = lastDTS;
            this.trigger("done");
          };
          this.endTimeline = function() {
            this.flush();
            this.trigger("endedtimeline");
          };
          this.discontinuity = function() {
            referenceDTS = void 0;
            lastDTS = void 0;
          };
          this.reset = function() {
            this.discontinuity();
            this.trigger("reset");
          };
        };
        TimestampRolloverStream$1.prototype = new stream();
        var timestampRolloverStream = {
          TimestampRolloverStream: TimestampRolloverStream$1,
          handleRollover: handleRollover$1
        };
        var percentEncode$1 = function percentEncode2(bytes, start, end) {
          var i, result = "";
          for (i = start; i < end; i++) {
            result += "%" + ("00" + bytes[i].toString(16)).slice(-2);
          }
          return result;
        }, parseUtf8 = function parseUtf82(bytes, start, end) {
          return decodeURIComponent(percentEncode$1(bytes, start, end));
        }, parseIso88591$1 = function parseIso885912(bytes, start, end) {
          return unescape(percentEncode$1(bytes, start, end));
        }, parseSyncSafeInteger$1 = function parseSyncSafeInteger2(data) {
          return data[0] << 21 | data[1] << 14 | data[2] << 7 | data[3];
        }, tagParsers = {
          TXXX: function TXXX(tag) {
            var i;
            if (tag.data[0] !== 3) {
              return;
            }
            for (i = 1; i < tag.data.length; i++) {
              if (tag.data[i] === 0) {
                tag.description = parseUtf8(tag.data, 1, i);
                tag.value = parseUtf8(tag.data, i + 1, tag.data.length).replace(/\0*$/, "");
                break;
              }
            }
            tag.data = tag.value;
          },
          WXXX: function WXXX(tag) {
            var i;
            if (tag.data[0] !== 3) {
              return;
            }
            for (i = 1; i < tag.data.length; i++) {
              if (tag.data[i] === 0) {
                tag.description = parseUtf8(tag.data, 1, i);
                tag.url = parseUtf8(tag.data, i + 1, tag.data.length);
                break;
              }
            }
          },
          PRIV: function PRIV(tag) {
            var i;
            for (i = 0; i < tag.data.length; i++) {
              if (tag.data[i] === 0) {
                tag.owner = parseIso88591$1(tag.data, 0, i);
                break;
              }
            }
            tag.privateData = tag.data.subarray(i + 1);
            tag.data = tag.privateData;
          }
        }, _MetadataStream;
        _MetadataStream = function MetadataStream(options) {
          var settings = {
            descriptor: options && options.descriptor
          }, tagSize = 0, buffer = [], bufferSize = 0, i;
          _MetadataStream.prototype.init.call(this);
          this.dispatchType = streamTypes.METADATA_STREAM_TYPE.toString(16);
          if (settings.descriptor) {
            for (i = 0; i < settings.descriptor.length; i++) {
              this.dispatchType += ("00" + settings.descriptor[i].toString(16)).slice(-2);
            }
          }
          this.push = function(chunk) {
            var tag, frameStart, frameSize, frame, i2, frameHeader;
            if (chunk.type !== "timed-metadata") {
              return;
            }
            if (chunk.dataAlignmentIndicator) {
              bufferSize = 0;
              buffer.length = 0;
            }
            if (buffer.length === 0 && (chunk.data.length < 10 || chunk.data[0] !== "I".charCodeAt(0) || chunk.data[1] !== "D".charCodeAt(0) || chunk.data[2] !== "3".charCodeAt(0))) {
              this.trigger("log", {
                level: "warn",
                message: "Skipping unrecognized metadata packet"
              });
              return;
            }
            buffer.push(chunk);
            bufferSize += chunk.data.byteLength;
            if (buffer.length === 1) {
              tagSize = parseSyncSafeInteger$1(chunk.data.subarray(6, 10));
              tagSize += 10;
            }
            if (bufferSize < tagSize) {
              return;
            }
            tag = {
              data: new Uint8Array(tagSize),
              frames: [],
              pts: buffer[0].pts,
              dts: buffer[0].dts
            };
            for (i2 = 0; i2 < tagSize; ) {
              tag.data.set(buffer[0].data.subarray(0, tagSize - i2), i2);
              i2 += buffer[0].data.byteLength;
              bufferSize -= buffer[0].data.byteLength;
              buffer.shift();
            }
            frameStart = 10;
            if (tag.data[5] & 64) {
              frameStart += 4;
              frameStart += parseSyncSafeInteger$1(tag.data.subarray(10, 14));
              tagSize -= parseSyncSafeInteger$1(tag.data.subarray(16, 20));
            }
            do {
              frameSize = parseSyncSafeInteger$1(tag.data.subarray(frameStart + 4, frameStart + 8));
              if (frameSize < 1) {
                this.trigger("log", {
                  level: "warn",
                  message: "Malformed ID3 frame encountered. Skipping remaining metadata parsing."
                });
                break;
              }
              frameHeader = String.fromCharCode(tag.data[frameStart], tag.data[frameStart + 1], tag.data[frameStart + 2], tag.data[frameStart + 3]);
              frame = {
                id: frameHeader,
                data: tag.data.subarray(frameStart + 10, frameStart + frameSize + 10)
              };
              frame.key = frame.id;
              if (tagParsers[frame.id]) {
                tagParsers[frame.id](frame);
                if (frame.owner === "com.apple.streaming.transportStreamTimestamp") {
                  var d = frame.data, size = (d[3] & 1) << 30 | d[4] << 22 | d[5] << 14 | d[6] << 6 | d[7] >>> 2;
                  size *= 4;
                  size += d[7] & 3;
                  frame.timeStamp = size;
                  if (tag.pts === void 0 && tag.dts === void 0) {
                    tag.pts = frame.timeStamp;
                    tag.dts = frame.timeStamp;
                  }
                  this.trigger("timestamp", frame);
                }
              }
              tag.frames.push(frame);
              frameStart += 10;
              frameStart += frameSize;
            } while (frameStart < tagSize);
            this.trigger("data", tag);
          };
        };
        _MetadataStream.prototype = new stream();
        var metadataStream = _MetadataStream;
        var TimestampRolloverStream = timestampRolloverStream.TimestampRolloverStream;
        var _TransportPacketStream, _TransportParseStream, _ElementaryStream;
        var MP2T_PACKET_LENGTH$1 = 188, SYNC_BYTE$1 = 71;
        _TransportPacketStream = function TransportPacketStream() {
          var buffer = new Uint8Array(MP2T_PACKET_LENGTH$1), bytesInBuffer = 0;
          _TransportPacketStream.prototype.init.call(this);
          this.push = function(bytes) {
            var startIndex = 0, endIndex = MP2T_PACKET_LENGTH$1, everything;
            if (bytesInBuffer) {
              everything = new Uint8Array(bytes.byteLength + bytesInBuffer);
              everything.set(buffer.subarray(0, bytesInBuffer));
              everything.set(bytes, bytesInBuffer);
              bytesInBuffer = 0;
            } else {
              everything = bytes;
            }
            while (endIndex < everything.byteLength) {
              if (everything[startIndex] === SYNC_BYTE$1 && everything[endIndex] === SYNC_BYTE$1) {
                this.trigger("data", everything.subarray(startIndex, endIndex));
                startIndex += MP2T_PACKET_LENGTH$1;
                endIndex += MP2T_PACKET_LENGTH$1;
                continue;
              }
              startIndex++;
              endIndex++;
            }
            if (startIndex < everything.byteLength) {
              buffer.set(everything.subarray(startIndex), 0);
              bytesInBuffer = everything.byteLength - startIndex;
            }
          };
          this.flush = function() {
            if (bytesInBuffer === MP2T_PACKET_LENGTH$1 && buffer[0] === SYNC_BYTE$1) {
              this.trigger("data", buffer);
              bytesInBuffer = 0;
            }
            this.trigger("done");
          };
          this.endTimeline = function() {
            this.flush();
            this.trigger("endedtimeline");
          };
          this.reset = function() {
            bytesInBuffer = 0;
            this.trigger("reset");
          };
        };
        _TransportPacketStream.prototype = new stream();
        _TransportParseStream = function TransportParseStream() {
          var parsePsi, parsePat2, parsePmt2, self2;
          _TransportParseStream.prototype.init.call(this);
          self2 = this;
          this.packetsWaitingForPmt = [];
          this.programMapTable = void 0;
          parsePsi = function parsePsi2(payload, psi) {
            var offset = 0;
            if (psi.payloadUnitStartIndicator) {
              offset += payload[offset] + 1;
            }
            if (psi.type === "pat") {
              parsePat2(payload.subarray(offset), psi);
            } else {
              parsePmt2(payload.subarray(offset), psi);
            }
          };
          parsePat2 = function parsePat3(payload, pat) {
            pat.section_number = payload[7];
            pat.last_section_number = payload[8];
            self2.pmtPid = (payload[10] & 31) << 8 | payload[11];
            pat.pmtPid = self2.pmtPid;
          };
          parsePmt2 = function parsePmt3(payload, pmt) {
            var sectionLength, tableEnd, programInfoLength, offset;
            if (!(payload[5] & 1)) {
              return;
            }
            self2.programMapTable = {
              video: null,
              audio: null,
              "timed-metadata": {}
            };
            sectionLength = (payload[1] & 15) << 8 | payload[2];
            tableEnd = 3 + sectionLength - 4;
            programInfoLength = (payload[10] & 15) << 8 | payload[11];
            offset = 12 + programInfoLength;
            while (offset < tableEnd) {
              var streamType = payload[offset];
              var pid = (payload[offset + 1] & 31) << 8 | payload[offset + 2];
              if (streamType === streamTypes.H264_STREAM_TYPE && self2.programMapTable.video === null) {
                self2.programMapTable.video = pid;
              } else if (streamType === streamTypes.ADTS_STREAM_TYPE && self2.programMapTable.audio === null) {
                self2.programMapTable.audio = pid;
              } else if (streamType === streamTypes.METADATA_STREAM_TYPE) {
                self2.programMapTable["timed-metadata"][pid] = streamType;
              }
              offset += ((payload[offset + 3] & 15) << 8 | payload[offset + 4]) + 5;
            }
            pmt.programMapTable = self2.programMapTable;
          };
          this.push = function(packet) {
            var result = {}, offset = 4;
            result.payloadUnitStartIndicator = !!(packet[1] & 64);
            result.pid = packet[1] & 31;
            result.pid <<= 8;
            result.pid |= packet[2];
            if ((packet[3] & 48) >>> 4 > 1) {
              offset += packet[offset] + 1;
            }
            if (result.pid === 0) {
              result.type = "pat";
              parsePsi(packet.subarray(offset), result);
              this.trigger("data", result);
            } else if (result.pid === this.pmtPid) {
              result.type = "pmt";
              parsePsi(packet.subarray(offset), result);
              this.trigger("data", result);
              while (this.packetsWaitingForPmt.length) {
                this.processPes_.apply(this, this.packetsWaitingForPmt.shift());
              }
            } else if (this.programMapTable === void 0) {
              this.packetsWaitingForPmt.push([packet, offset, result]);
            } else {
              this.processPes_(packet, offset, result);
            }
          };
          this.processPes_ = function(packet, offset, result) {
            if (result.pid === this.programMapTable.video) {
              result.streamType = streamTypes.H264_STREAM_TYPE;
            } else if (result.pid === this.programMapTable.audio) {
              result.streamType = streamTypes.ADTS_STREAM_TYPE;
            } else {
              result.streamType = this.programMapTable["timed-metadata"][result.pid];
            }
            result.type = "pes";
            result.data = packet.subarray(offset);
            this.trigger("data", result);
          };
        };
        _TransportParseStream.prototype = new stream();
        _TransportParseStream.STREAM_TYPES = {
          h264: 27,
          adts: 15
        };
        _ElementaryStream = function ElementaryStream() {
          var self2 = this, segmentHadPmt = false, video = {
            data: [],
            size: 0
          }, audio = {
            data: [],
            size: 0
          }, timedMetadata = {
            data: [],
            size: 0
          }, programMapTable, parsePes = function parsePes2(payload, pes) {
            var ptsDtsFlags;
            var startPrefix = payload[0] << 16 | payload[1] << 8 | payload[2];
            pes.data = new Uint8Array();
            if (startPrefix !== 1) {
              return;
            }
            pes.packetLength = 6 + (payload[4] << 8 | payload[5]);
            pes.dataAlignmentIndicator = (payload[6] & 4) !== 0;
            ptsDtsFlags = payload[7];
            if (ptsDtsFlags & 192) {
              pes.pts = (payload[9] & 14) << 27 | (payload[10] & 255) << 20 | (payload[11] & 254) << 12 | (payload[12] & 255) << 5 | (payload[13] & 254) >>> 3;
              pes.pts *= 4;
              pes.pts += (payload[13] & 6) >>> 1;
              pes.dts = pes.pts;
              if (ptsDtsFlags & 64) {
                pes.dts = (payload[14] & 14) << 27 | (payload[15] & 255) << 20 | (payload[16] & 254) << 12 | (payload[17] & 255) << 5 | (payload[18] & 254) >>> 3;
                pes.dts *= 4;
                pes.dts += (payload[18] & 6) >>> 1;
              }
            }
            pes.data = payload.subarray(9 + payload[8]);
          }, flushStream = function flushStream2(stream2, type2, forceFlush) {
            var packetData = new Uint8Array(stream2.size), event = {
              type: type2
            }, i = 0, offset = 0, packetFlushable = false, fragment;
            if (!stream2.data.length || stream2.size < 9) {
              return;
            }
            event.trackId = stream2.data[0].pid;
            for (i = 0; i < stream2.data.length; i++) {
              fragment = stream2.data[i];
              packetData.set(fragment.data, offset);
              offset += fragment.data.byteLength;
            }
            parsePes(packetData, event);
            packetFlushable = type2 === "video" || event.packetLength <= stream2.size;
            if (forceFlush || packetFlushable) {
              stream2.size = 0;
              stream2.data.length = 0;
            }
            if (packetFlushable) {
              self2.trigger("data", event);
            }
          };
          _ElementaryStream.prototype.init.call(this);
          this.push = function(data) {
            ({
              pat: function pat() {
              },
              pes: function pes() {
                var stream2, streamType;
                switch (data.streamType) {
                  case streamTypes.H264_STREAM_TYPE:
                    stream2 = video;
                    streamType = "video";
                    break;
                  case streamTypes.ADTS_STREAM_TYPE:
                    stream2 = audio;
                    streamType = "audio";
                    break;
                  case streamTypes.METADATA_STREAM_TYPE:
                    stream2 = timedMetadata;
                    streamType = "timed-metadata";
                    break;
                  default:
                    return;
                }
                if (data.payloadUnitStartIndicator) {
                  flushStream(stream2, streamType, true);
                }
                stream2.data.push(data);
                stream2.size += data.data.byteLength;
              },
              pmt: function pmt() {
                var event = {
                  type: "metadata",
                  tracks: []
                };
                programMapTable = data.programMapTable;
                if (programMapTable.video !== null) {
                  event.tracks.push({
                    timelineStartInfo: {
                      baseMediaDecodeTime: 0
                    },
                    id: +programMapTable.video,
                    codec: "avc",
                    type: "video"
                  });
                }
                if (programMapTable.audio !== null) {
                  event.tracks.push({
                    timelineStartInfo: {
                      baseMediaDecodeTime: 0
                    },
                    id: +programMapTable.audio,
                    codec: "adts",
                    type: "audio"
                  });
                }
                segmentHadPmt = true;
                self2.trigger("data", event);
              }
            })[data.type]();
          };
          this.reset = function() {
            video.size = 0;
            video.data.length = 0;
            audio.size = 0;
            audio.data.length = 0;
            this.trigger("reset");
          };
          this.flushStreams_ = function() {
            flushStream(video, "video");
            flushStream(audio, "audio");
            flushStream(timedMetadata, "timed-metadata");
          };
          this.flush = function() {
            if (!segmentHadPmt && programMapTable) {
              var pmt = {
                type: "metadata",
                tracks: []
              };
              if (programMapTable.video !== null) {
                pmt.tracks.push({
                  timelineStartInfo: {
                    baseMediaDecodeTime: 0
                  },
                  id: +programMapTable.video,
                  codec: "avc",
                  type: "video"
                });
              }
              if (programMapTable.audio !== null) {
                pmt.tracks.push({
                  timelineStartInfo: {
                    baseMediaDecodeTime: 0
                  },
                  id: +programMapTable.audio,
                  codec: "adts",
                  type: "audio"
                });
              }
              self2.trigger("data", pmt);
            }
            segmentHadPmt = false;
            this.flushStreams_();
            this.trigger("done");
          };
        };
        _ElementaryStream.prototype = new stream();
        var m2ts$1 = {
          PAT_PID: 0,
          MP2T_PACKET_LENGTH: MP2T_PACKET_LENGTH$1,
          TransportPacketStream: _TransportPacketStream,
          TransportParseStream: _TransportParseStream,
          ElementaryStream: _ElementaryStream,
          TimestampRolloverStream,
          CaptionStream: captionStream.CaptionStream,
          Cea608Stream: captionStream.Cea608Stream,
          Cea708Stream: captionStream.Cea708Stream,
          MetadataStream: metadataStream
        };
        for (var type in streamTypes) {
          if (streamTypes.hasOwnProperty(type)) {
            m2ts$1[type] = streamTypes[type];
          }
        }
        var m2ts_1 = m2ts$1;
        var ADTS_SAMPLING_FREQUENCIES = [96e3, 88200, 64e3, 48e3, 44100, 32e3, 24e3, 22050, 16e3, 12e3, 11025, 8e3, 7350];
        var parseId3TagSize = function parseId3TagSize2(header, byteIndex) {
          var returnSize = header[byteIndex + 6] << 21 | header[byteIndex + 7] << 14 | header[byteIndex + 8] << 7 | header[byteIndex + 9], flags = header[byteIndex + 5], footerPresent = (flags & 16) >> 4;
          returnSize = returnSize >= 0 ? returnSize : 0;
          if (footerPresent) {
            return returnSize + 20;
          }
          return returnSize + 10;
        };
        var getId3Offset = function getId3Offset2(data, offset) {
          if (data.length - offset < 10 || data[offset] !== "I".charCodeAt(0) || data[offset + 1] !== "D".charCodeAt(0) || data[offset + 2] !== "3".charCodeAt(0)) {
            return offset;
          }
          offset += parseId3TagSize(data, offset);
          return getId3Offset2(data, offset);
        };
        var isLikelyAacData$2 = function isLikelyAacData2(data) {
          var offset = getId3Offset(data, 0);
          return data.length >= offset + 2 && (data[offset] & 255) === 255 && (data[offset + 1] & 240) === 240 && (data[offset + 1] & 22) === 16;
        };
        var parseSyncSafeInteger = function parseSyncSafeInteger2(data) {
          return data[0] << 21 | data[1] << 14 | data[2] << 7 | data[3];
        };
        var percentEncode = function percentEncode2(bytes, start, end) {
          var i, result = "";
          for (i = start; i < end; i++) {
            result += "%" + ("00" + bytes[i].toString(16)).slice(-2);
          }
          return result;
        };
        var parseIso88591 = function parseIso885912(bytes, start, end) {
          return unescape(percentEncode(bytes, start, end));
        };
        var parseAdtsSize = function parseAdtsSize2(header, byteIndex) {
          var lowThree = (header[byteIndex + 5] & 224) >> 5, middle = header[byteIndex + 4] << 3, highTwo = header[byteIndex + 3] & 3 << 11;
          return highTwo | middle | lowThree;
        };
        var parseType$1 = function parseType2(header, byteIndex) {
          if (header[byteIndex] === "I".charCodeAt(0) && header[byteIndex + 1] === "D".charCodeAt(0) && header[byteIndex + 2] === "3".charCodeAt(0)) {
            return "timed-metadata";
          } else if (header[byteIndex] & true && (header[byteIndex + 1] & 240) === 240) {
            return "audio";
          }
          return null;
        };
        var parseSampleRate = function parseSampleRate2(packet) {
          var i = 0;
          while (i + 5 < packet.length) {
            if (packet[i] !== 255 || (packet[i + 1] & 246) !== 240) {
              i++;
              continue;
            }
            return ADTS_SAMPLING_FREQUENCIES[(packet[i + 2] & 60) >>> 2];
          }
          return null;
        };
        var parseAacTimestamp = function parseAacTimestamp2(packet) {
          var frameStart, frameSize, frame, frameHeader;
          frameStart = 10;
          if (packet[5] & 64) {
            frameStart += 4;
            frameStart += parseSyncSafeInteger(packet.subarray(10, 14));
          }
          do {
            frameSize = parseSyncSafeInteger(packet.subarray(frameStart + 4, frameStart + 8));
            if (frameSize < 1) {
              return null;
            }
            frameHeader = String.fromCharCode(packet[frameStart], packet[frameStart + 1], packet[frameStart + 2], packet[frameStart + 3]);
            if (frameHeader === "PRIV") {
              frame = packet.subarray(frameStart + 10, frameStart + frameSize + 10);
              for (var i = 0; i < frame.byteLength; i++) {
                if (frame[i] === 0) {
                  var owner = parseIso88591(frame, 0, i);
                  if (owner === "com.apple.streaming.transportStreamTimestamp") {
                    var d = frame.subarray(i + 1);
                    var size = (d[3] & 1) << 30 | d[4] << 22 | d[5] << 14 | d[6] << 6 | d[7] >>> 2;
                    size *= 4;
                    size += d[7] & 3;
                    return size;
                  }
                  break;
                }
              }
            }
            frameStart += 10;
            frameStart += frameSize;
          } while (frameStart < packet.byteLength);
          return null;
        };
        var utils = {
          isLikelyAacData: isLikelyAacData$2,
          parseId3TagSize,
          parseAdtsSize,
          parseType: parseType$1,
          parseSampleRate,
          parseAacTimestamp
        };
        var _AacStream;
        _AacStream = function AacStream() {
          var everything = new Uint8Array(), timeStamp = 0;
          _AacStream.prototype.init.call(this);
          this.setTimestamp = function(timestamp) {
            timeStamp = timestamp;
          };
          this.push = function(bytes) {
            var frameSize = 0, byteIndex = 0, bytesLeft, chunk, packet, tempLength;
            if (everything.length) {
              tempLength = everything.length;
              everything = new Uint8Array(bytes.byteLength + tempLength);
              everything.set(everything.subarray(0, tempLength));
              everything.set(bytes, tempLength);
            } else {
              everything = bytes;
            }
            while (everything.length - byteIndex >= 3) {
              if (everything[byteIndex] === "I".charCodeAt(0) && everything[byteIndex + 1] === "D".charCodeAt(0) && everything[byteIndex + 2] === "3".charCodeAt(0)) {
                if (everything.length - byteIndex < 10) {
                  break;
                }
                frameSize = utils.parseId3TagSize(everything, byteIndex);
                if (byteIndex + frameSize > everything.length) {
                  break;
                }
                chunk = {
                  type: "timed-metadata",
                  data: everything.subarray(byteIndex, byteIndex + frameSize)
                };
                this.trigger("data", chunk);
                byteIndex += frameSize;
                continue;
              } else if ((everything[byteIndex] & 255) === 255 && (everything[byteIndex + 1] & 240) === 240) {
                if (everything.length - byteIndex < 7) {
                  break;
                }
                frameSize = utils.parseAdtsSize(everything, byteIndex);
                if (byteIndex + frameSize > everything.length) {
                  break;
                }
                packet = {
                  type: "audio",
                  data: everything.subarray(byteIndex, byteIndex + frameSize),
                  pts: timeStamp,
                  dts: timeStamp
                };
                this.trigger("data", packet);
                byteIndex += frameSize;
                continue;
              }
              byteIndex++;
            }
            bytesLeft = everything.length - byteIndex;
            if (bytesLeft > 0) {
              everything = everything.subarray(byteIndex);
            } else {
              everything = new Uint8Array();
            }
          };
          this.reset = function() {
            everything = new Uint8Array();
            this.trigger("reset");
          };
          this.endTimeline = function() {
            everything = new Uint8Array();
            this.trigger("endedtimeline");
          };
        };
        _AacStream.prototype = new stream();
        var aac = _AacStream;
        var AUDIO_PROPERTIES = ["audioobjecttype", "channelcount", "samplerate", "samplingfrequencyindex", "samplesize"];
        var audioProperties = AUDIO_PROPERTIES;
        var VIDEO_PROPERTIES = ["width", "height", "profileIdc", "levelIdc", "profileCompatibility", "sarRatio"];
        var videoProperties = VIDEO_PROPERTIES;
        var H264Stream$1 = h264.H264Stream;
        var isLikelyAacData$1 = utils.isLikelyAacData;
        var ONE_SECOND_IN_TS$2 = clock.ONE_SECOND_IN_TS;
        var _VideoSegmentStream$1, _AudioSegmentStream$1, _Transmuxer$1, _CoalesceStream;
        var retriggerForStream = function retriggerForStream2(key, event) {
          event.stream = key;
          this.trigger("log", event);
        };
        var addPipelineLogRetriggers = function addPipelineLogRetriggers2(transmuxer3, pipeline) {
          var keys = Object.keys(pipeline);
          for (var i = 0; i < keys.length; i++) {
            var key = keys[i];
            if (key === "headOfPipeline" || !pipeline[key].on) {
              continue;
            }
            pipeline[key].on("log", retriggerForStream.bind(transmuxer3, key));
          }
        };
        var arrayEquals = function arrayEquals2(a, b) {
          var i;
          if (a.length !== b.length) {
            return false;
          }
          for (i = 0; i < a.length; i++) {
            if (a[i] !== b[i]) {
              return false;
            }
          }
          return true;
        };
        var generateSegmentTimingInfo = function generateSegmentTimingInfo2(baseMediaDecodeTime, startDts, startPts, endDts, endPts, prependedContentDuration) {
          var ptsOffsetFromDts = startPts - startDts, decodeDuration = endDts - startDts, presentationDuration = endPts - startPts;
          return {
            start: {
              dts: baseMediaDecodeTime,
              pts: baseMediaDecodeTime + ptsOffsetFromDts
            },
            end: {
              dts: baseMediaDecodeTime + decodeDuration,
              pts: baseMediaDecodeTime + presentationDuration
            },
            prependedContentDuration,
            baseMediaDecodeTime
          };
        };
        _AudioSegmentStream$1 = function AudioSegmentStream2(track, options) {
          var adtsFrames = [], sequenceNumber, earliestAllowedDts = 0, audioAppendStartTs = 0, videoBaseMediaDecodeTime = Infinity;
          options = options || {};
          sequenceNumber = options.firstSequenceNumber || 0;
          _AudioSegmentStream$1.prototype.init.call(this);
          this.push = function(data) {
            trackDecodeInfo.collectDtsInfo(track, data);
            if (track) {
              audioProperties.forEach(function(prop) {
                track[prop] = data[prop];
              });
            }
            adtsFrames.push(data);
          };
          this.setEarliestDts = function(earliestDts) {
            earliestAllowedDts = earliestDts;
          };
          this.setVideoBaseMediaDecodeTime = function(baseMediaDecodeTime) {
            videoBaseMediaDecodeTime = baseMediaDecodeTime;
          };
          this.setAudioAppendStart = function(timestamp) {
            audioAppendStartTs = timestamp;
          };
          this.flush = function() {
            var frames, moof2, mdat2, boxes, frameDuration, segmentDuration, videoClockCyclesOfSilencePrefixed;
            if (adtsFrames.length === 0) {
              this.trigger("done", "AudioSegmentStream");
              return;
            }
            frames = audioFrameUtils.trimAdtsFramesByEarliestDts(adtsFrames, track, earliestAllowedDts);
            track.baseMediaDecodeTime = trackDecodeInfo.calculateTrackBaseMediaDecodeTime(track, options.keepOriginalTimestamps);
            videoClockCyclesOfSilencePrefixed = audioFrameUtils.prefixWithSilence(track, frames, audioAppendStartTs, videoBaseMediaDecodeTime);
            track.samples = audioFrameUtils.generateSampleTable(frames);
            mdat2 = mp4Generator.mdat(audioFrameUtils.concatenateFrameData(frames));
            adtsFrames = [];
            moof2 = mp4Generator.moof(sequenceNumber, [track]);
            boxes = new Uint8Array(moof2.byteLength + mdat2.byteLength);
            sequenceNumber++;
            boxes.set(moof2);
            boxes.set(mdat2, moof2.byteLength);
            trackDecodeInfo.clearDtsInfo(track);
            frameDuration = Math.ceil(ONE_SECOND_IN_TS$2 * 1024 / track.samplerate);
            if (frames.length) {
              segmentDuration = frames.length * frameDuration;
              this.trigger("segmentTimingInfo", generateSegmentTimingInfo(clock.audioTsToVideoTs(track.baseMediaDecodeTime, track.samplerate), frames[0].dts, frames[0].pts, frames[0].dts + segmentDuration, frames[0].pts + segmentDuration, videoClockCyclesOfSilencePrefixed || 0));
              this.trigger("timingInfo", {
                start: frames[0].pts,
                end: frames[0].pts + segmentDuration
              });
            }
            this.trigger("data", {
              track,
              boxes
            });
            this.trigger("done", "AudioSegmentStream");
          };
          this.reset = function() {
            trackDecodeInfo.clearDtsInfo(track);
            adtsFrames = [];
            this.trigger("reset");
          };
        };
        _AudioSegmentStream$1.prototype = new stream();
        _VideoSegmentStream$1 = function VideoSegmentStream2(track, options) {
          var sequenceNumber, nalUnits = [], gopsToAlignWith = [], config, pps;
          options = options || {};
          sequenceNumber = options.firstSequenceNumber || 0;
          _VideoSegmentStream$1.prototype.init.call(this);
          delete track.minPTS;
          this.gopCache_ = [];
          this.push = function(nalUnit) {
            trackDecodeInfo.collectDtsInfo(track, nalUnit);
            if (nalUnit.nalUnitType === "seq_parameter_set_rbsp" && !config) {
              config = nalUnit.config;
              track.sps = [nalUnit.data];
              videoProperties.forEach(function(prop) {
                track[prop] = config[prop];
              }, this);
            }
            if (nalUnit.nalUnitType === "pic_parameter_set_rbsp" && !pps) {
              pps = nalUnit.data;
              track.pps = [nalUnit.data];
            }
            nalUnits.push(nalUnit);
          };
          this.flush = function() {
            var frames, gopForFusion, gops, moof2, mdat2, boxes, prependedContentDuration = 0, firstGop, lastGop;
            while (nalUnits.length) {
              if (nalUnits[0].nalUnitType === "access_unit_delimiter_rbsp") {
                break;
              }
              nalUnits.shift();
            }
            if (nalUnits.length === 0) {
              this.resetStream_();
              this.trigger("done", "VideoSegmentStream");
              return;
            }
            frames = frameUtils.groupNalsIntoFrames(nalUnits);
            gops = frameUtils.groupFramesIntoGops(frames);
            if (!gops[0][0].keyFrame) {
              gopForFusion = this.getGopForFusion_(nalUnits[0], track);
              if (gopForFusion) {
                prependedContentDuration = gopForFusion.duration;
                gops.unshift(gopForFusion);
                gops.byteLength += gopForFusion.byteLength;
                gops.nalCount += gopForFusion.nalCount;
                gops.pts = gopForFusion.pts;
                gops.dts = gopForFusion.dts;
                gops.duration += gopForFusion.duration;
              } else {
                gops = frameUtils.extendFirstKeyFrame(gops);
              }
            }
            if (gopsToAlignWith.length) {
              var alignedGops;
              if (options.alignGopsAtEnd) {
                alignedGops = this.alignGopsAtEnd_(gops);
              } else {
                alignedGops = this.alignGopsAtStart_(gops);
              }
              if (!alignedGops) {
                this.gopCache_.unshift({
                  gop: gops.pop(),
                  pps: track.pps,
                  sps: track.sps
                });
                this.gopCache_.length = Math.min(6, this.gopCache_.length);
                nalUnits = [];
                this.resetStream_();
                this.trigger("done", "VideoSegmentStream");
                return;
              }
              trackDecodeInfo.clearDtsInfo(track);
              gops = alignedGops;
            }
            trackDecodeInfo.collectDtsInfo(track, gops);
            track.samples = frameUtils.generateSampleTable(gops);
            mdat2 = mp4Generator.mdat(frameUtils.concatenateNalData(gops));
            track.baseMediaDecodeTime = trackDecodeInfo.calculateTrackBaseMediaDecodeTime(track, options.keepOriginalTimestamps);
            this.trigger("processedGopsInfo", gops.map(function(gop) {
              return {
                pts: gop.pts,
                dts: gop.dts,
                byteLength: gop.byteLength
              };
            }));
            firstGop = gops[0];
            lastGop = gops[gops.length - 1];
            this.trigger("segmentTimingInfo", generateSegmentTimingInfo(track.baseMediaDecodeTime, firstGop.dts, firstGop.pts, lastGop.dts + lastGop.duration, lastGop.pts + lastGop.duration, prependedContentDuration));
            this.trigger("timingInfo", {
              start: gops[0].pts,
              end: gops[gops.length - 1].pts + gops[gops.length - 1].duration
            });
            this.gopCache_.unshift({
              gop: gops.pop(),
              pps: track.pps,
              sps: track.sps
            });
            this.gopCache_.length = Math.min(6, this.gopCache_.length);
            nalUnits = [];
            this.trigger("baseMediaDecodeTime", track.baseMediaDecodeTime);
            this.trigger("timelineStartInfo", track.timelineStartInfo);
            moof2 = mp4Generator.moof(sequenceNumber, [track]);
            boxes = new Uint8Array(moof2.byteLength + mdat2.byteLength);
            sequenceNumber++;
            boxes.set(moof2);
            boxes.set(mdat2, moof2.byteLength);
            this.trigger("data", {
              track,
              boxes
            });
            this.resetStream_();
            this.trigger("done", "VideoSegmentStream");
          };
          this.reset = function() {
            this.resetStream_();
            nalUnits = [];
            this.gopCache_.length = 0;
            gopsToAlignWith.length = 0;
            this.trigger("reset");
          };
          this.resetStream_ = function() {
            trackDecodeInfo.clearDtsInfo(track);
            config = void 0;
            pps = void 0;
          };
          this.getGopForFusion_ = function(nalUnit) {
            var halfSecond = 45e3, allowableOverlap = 1e4, nearestDistance = Infinity, dtsDistance, nearestGopObj, currentGop, currentGopObj, i;
            for (i = 0; i < this.gopCache_.length; i++) {
              currentGopObj = this.gopCache_[i];
              currentGop = currentGopObj.gop;
              if (!(track.pps && arrayEquals(track.pps[0], currentGopObj.pps[0])) || !(track.sps && arrayEquals(track.sps[0], currentGopObj.sps[0]))) {
                continue;
              }
              if (currentGop.dts < track.timelineStartInfo.dts) {
                continue;
              }
              dtsDistance = nalUnit.dts - currentGop.dts - currentGop.duration;
              if (dtsDistance >= -allowableOverlap && dtsDistance <= halfSecond) {
                if (!nearestGopObj || nearestDistance > dtsDistance) {
                  nearestGopObj = currentGopObj;
                  nearestDistance = dtsDistance;
                }
              }
            }
            if (nearestGopObj) {
              return nearestGopObj.gop;
            }
            return null;
          };
          this.alignGopsAtStart_ = function(gops) {
            var alignIndex, gopIndex, align, gop, byteLength, nalCount, duration, alignedGops;
            byteLength = gops.byteLength;
            nalCount = gops.nalCount;
            duration = gops.duration;
            alignIndex = gopIndex = 0;
            while (alignIndex < gopsToAlignWith.length && gopIndex < gops.length) {
              align = gopsToAlignWith[alignIndex];
              gop = gops[gopIndex];
              if (align.pts === gop.pts) {
                break;
              }
              if (gop.pts > align.pts) {
                alignIndex++;
                continue;
              }
              gopIndex++;
              byteLength -= gop.byteLength;
              nalCount -= gop.nalCount;
              duration -= gop.duration;
            }
            if (gopIndex === 0) {
              return gops;
            }
            if (gopIndex === gops.length) {
              return null;
            }
            alignedGops = gops.slice(gopIndex);
            alignedGops.byteLength = byteLength;
            alignedGops.duration = duration;
            alignedGops.nalCount = nalCount;
            alignedGops.pts = alignedGops[0].pts;
            alignedGops.dts = alignedGops[0].dts;
            return alignedGops;
          };
          this.alignGopsAtEnd_ = function(gops) {
            var alignIndex, gopIndex, align, gop, alignEndIndex, matchFound;
            alignIndex = gopsToAlignWith.length - 1;
            gopIndex = gops.length - 1;
            alignEndIndex = null;
            matchFound = false;
            while (alignIndex >= 0 && gopIndex >= 0) {
              align = gopsToAlignWith[alignIndex];
              gop = gops[gopIndex];
              if (align.pts === gop.pts) {
                matchFound = true;
                break;
              }
              if (align.pts > gop.pts) {
                alignIndex--;
                continue;
              }
              if (alignIndex === gopsToAlignWith.length - 1) {
                alignEndIndex = gopIndex;
              }
              gopIndex--;
            }
            if (!matchFound && alignEndIndex === null) {
              return null;
            }
            var trimIndex;
            if (matchFound) {
              trimIndex = gopIndex;
            } else {
              trimIndex = alignEndIndex;
            }
            if (trimIndex === 0) {
              return gops;
            }
            var alignedGops = gops.slice(trimIndex);
            var metadata = alignedGops.reduce(function(total, gop2) {
              total.byteLength += gop2.byteLength;
              total.duration += gop2.duration;
              total.nalCount += gop2.nalCount;
              return total;
            }, {
              byteLength: 0,
              duration: 0,
              nalCount: 0
            });
            alignedGops.byteLength = metadata.byteLength;
            alignedGops.duration = metadata.duration;
            alignedGops.nalCount = metadata.nalCount;
            alignedGops.pts = alignedGops[0].pts;
            alignedGops.dts = alignedGops[0].dts;
            return alignedGops;
          };
          this.alignGopsWith = function(newGopsToAlignWith) {
            gopsToAlignWith = newGopsToAlignWith;
          };
        };
        _VideoSegmentStream$1.prototype = new stream();
        _CoalesceStream = function CoalesceStream2(options, metadataStream2) {
          this.numberOfTracks = 0;
          this.metadataStream = metadataStream2;
          options = options || {};
          if (typeof options.remux !== "undefined") {
            this.remuxTracks = !!options.remux;
          } else {
            this.remuxTracks = true;
          }
          if (typeof options.keepOriginalTimestamps === "boolean") {
            this.keepOriginalTimestamps = options.keepOriginalTimestamps;
          } else {
            this.keepOriginalTimestamps = false;
          }
          this.pendingTracks = [];
          this.videoTrack = null;
          this.pendingBoxes = [];
          this.pendingCaptions = [];
          this.pendingMetadata = [];
          this.pendingBytes = 0;
          this.emittedTracks = 0;
          _CoalesceStream.prototype.init.call(this);
          this.push = function(output) {
            if (output.text) {
              return this.pendingCaptions.push(output);
            }
            if (output.frames) {
              return this.pendingMetadata.push(output);
            }
            this.pendingTracks.push(output.track);
            this.pendingBytes += output.boxes.byteLength;
            if (output.track.type === "video") {
              this.videoTrack = output.track;
              this.pendingBoxes.push(output.boxes);
            }
            if (output.track.type === "audio") {
              this.audioTrack = output.track;
              this.pendingBoxes.unshift(output.boxes);
            }
          };
        };
        _CoalesceStream.prototype = new stream();
        _CoalesceStream.prototype.flush = function(flushSource) {
          var offset = 0, event = {
            captions: [],
            captionStreams: {},
            metadata: [],
            info: {}
          }, caption, id3, initSegment, timelineStartPts = 0, i;
          if (this.pendingTracks.length < this.numberOfTracks) {
            if (flushSource !== "VideoSegmentStream" && flushSource !== "AudioSegmentStream") {
              return;
            } else if (this.remuxTracks) {
              return;
            } else if (this.pendingTracks.length === 0) {
              this.emittedTracks++;
              if (this.emittedTracks >= this.numberOfTracks) {
                this.trigger("done");
                this.emittedTracks = 0;
              }
              return;
            }
          }
          if (this.videoTrack) {
            timelineStartPts = this.videoTrack.timelineStartInfo.pts;
            videoProperties.forEach(function(prop) {
              event.info[prop] = this.videoTrack[prop];
            }, this);
          } else if (this.audioTrack) {
            timelineStartPts = this.audioTrack.timelineStartInfo.pts;
            audioProperties.forEach(function(prop) {
              event.info[prop] = this.audioTrack[prop];
            }, this);
          }
          if (this.videoTrack || this.audioTrack) {
            if (this.pendingTracks.length === 1) {
              event.type = this.pendingTracks[0].type;
            } else {
              event.type = "combined";
            }
            this.emittedTracks += this.pendingTracks.length;
            initSegment = mp4Generator.initSegment(this.pendingTracks);
            event.initSegment = new Uint8Array(initSegment.byteLength);
            event.initSegment.set(initSegment);
            event.data = new Uint8Array(this.pendingBytes);
            for (i = 0; i < this.pendingBoxes.length; i++) {
              event.data.set(this.pendingBoxes[i], offset);
              offset += this.pendingBoxes[i].byteLength;
            }
            for (i = 0; i < this.pendingCaptions.length; i++) {
              caption = this.pendingCaptions[i];
              caption.startTime = clock.metadataTsToSeconds(caption.startPts, timelineStartPts, this.keepOriginalTimestamps);
              caption.endTime = clock.metadataTsToSeconds(caption.endPts, timelineStartPts, this.keepOriginalTimestamps);
              event.captionStreams[caption.stream] = true;
              event.captions.push(caption);
            }
            for (i = 0; i < this.pendingMetadata.length; i++) {
              id3 = this.pendingMetadata[i];
              id3.cueTime = clock.metadataTsToSeconds(id3.pts, timelineStartPts, this.keepOriginalTimestamps);
              event.metadata.push(id3);
            }
            event.metadata.dispatchType = this.metadataStream.dispatchType;
            this.pendingTracks.length = 0;
            this.videoTrack = null;
            this.pendingBoxes.length = 0;
            this.pendingCaptions.length = 0;
            this.pendingBytes = 0;
            this.pendingMetadata.length = 0;
            this.trigger("data", event);
            for (i = 0; i < event.captions.length; i++) {
              caption = event.captions[i];
              this.trigger("caption", caption);
            }
            for (i = 0; i < event.metadata.length; i++) {
              id3 = event.metadata[i];
              this.trigger("id3Frame", id3);
            }
          }
          if (this.emittedTracks >= this.numberOfTracks) {
            this.trigger("done");
            this.emittedTracks = 0;
          }
        };
        _CoalesceStream.prototype.setRemux = function(val) {
          this.remuxTracks = val;
        };
        _Transmuxer$1 = function Transmuxer2(options) {
          var self2 = this, hasFlushed = true, videoTrack, audioTrack;
          _Transmuxer$1.prototype.init.call(this);
          options = options || {};
          this.baseMediaDecodeTime = options.baseMediaDecodeTime || 0;
          this.transmuxPipeline_ = {};
          this.setupAacPipeline = function() {
            var pipeline = {};
            this.transmuxPipeline_ = pipeline;
            pipeline.type = "aac";
            pipeline.metadataStream = new m2ts_1.MetadataStream();
            pipeline.aacStream = new aac();
            pipeline.audioTimestampRolloverStream = new m2ts_1.TimestampRolloverStream("audio");
            pipeline.timedMetadataTimestampRolloverStream = new m2ts_1.TimestampRolloverStream("timed-metadata");
            pipeline.adtsStream = new adts();
            pipeline.coalesceStream = new _CoalesceStream(options, pipeline.metadataStream);
            pipeline.headOfPipeline = pipeline.aacStream;
            pipeline.aacStream.pipe(pipeline.audioTimestampRolloverStream).pipe(pipeline.adtsStream);
            pipeline.aacStream.pipe(pipeline.timedMetadataTimestampRolloverStream).pipe(pipeline.metadataStream).pipe(pipeline.coalesceStream);
            pipeline.metadataStream.on("timestamp", function(frame) {
              pipeline.aacStream.setTimestamp(frame.timeStamp);
            });
            pipeline.aacStream.on("data", function(data) {
              if (data.type !== "timed-metadata" && data.type !== "audio" || pipeline.audioSegmentStream) {
                return;
              }
              audioTrack = audioTrack || {
                timelineStartInfo: {
                  baseMediaDecodeTime: self2.baseMediaDecodeTime
                },
                codec: "adts",
                type: "audio"
              };
              pipeline.coalesceStream.numberOfTracks++;
              pipeline.audioSegmentStream = new _AudioSegmentStream$1(audioTrack, options);
              pipeline.audioSegmentStream.on("log", self2.getLogTrigger_("audioSegmentStream"));
              pipeline.audioSegmentStream.on("timingInfo", self2.trigger.bind(self2, "audioTimingInfo"));
              pipeline.adtsStream.pipe(pipeline.audioSegmentStream).pipe(pipeline.coalesceStream);
              self2.trigger("trackinfo", {
                hasAudio: !!audioTrack,
                hasVideo: !!videoTrack
              });
            });
            pipeline.coalesceStream.on("data", this.trigger.bind(this, "data"));
            pipeline.coalesceStream.on("done", this.trigger.bind(this, "done"));
            addPipelineLogRetriggers(this, pipeline);
          };
          this.setupTsPipeline = function() {
            var pipeline = {};
            this.transmuxPipeline_ = pipeline;
            pipeline.type = "ts";
            pipeline.metadataStream = new m2ts_1.MetadataStream();
            pipeline.packetStream = new m2ts_1.TransportPacketStream();
            pipeline.parseStream = new m2ts_1.TransportParseStream();
            pipeline.elementaryStream = new m2ts_1.ElementaryStream();
            pipeline.timestampRolloverStream = new m2ts_1.TimestampRolloverStream();
            pipeline.adtsStream = new adts();
            pipeline.h264Stream = new H264Stream$1();
            pipeline.captionStream = new m2ts_1.CaptionStream(options);
            pipeline.coalesceStream = new _CoalesceStream(options, pipeline.metadataStream);
            pipeline.headOfPipeline = pipeline.packetStream;
            pipeline.packetStream.pipe(pipeline.parseStream).pipe(pipeline.elementaryStream).pipe(pipeline.timestampRolloverStream);
            pipeline.timestampRolloverStream.pipe(pipeline.h264Stream);
            pipeline.timestampRolloverStream.pipe(pipeline.adtsStream);
            pipeline.timestampRolloverStream.pipe(pipeline.metadataStream).pipe(pipeline.coalesceStream);
            pipeline.h264Stream.pipe(pipeline.captionStream).pipe(pipeline.coalesceStream);
            pipeline.elementaryStream.on("data", function(data) {
              var i;
              if (data.type === "metadata") {
                i = data.tracks.length;
                while (i--) {
                  if (!videoTrack && data.tracks[i].type === "video") {
                    videoTrack = data.tracks[i];
                    videoTrack.timelineStartInfo.baseMediaDecodeTime = self2.baseMediaDecodeTime;
                  } else if (!audioTrack && data.tracks[i].type === "audio") {
                    audioTrack = data.tracks[i];
                    audioTrack.timelineStartInfo.baseMediaDecodeTime = self2.baseMediaDecodeTime;
                  }
                }
                if (videoTrack && !pipeline.videoSegmentStream) {
                  pipeline.coalesceStream.numberOfTracks++;
                  pipeline.videoSegmentStream = new _VideoSegmentStream$1(videoTrack, options);
                  pipeline.videoSegmentStream.on("log", self2.getLogTrigger_("videoSegmentStream"));
                  pipeline.videoSegmentStream.on("timelineStartInfo", function(timelineStartInfo) {
                    if (audioTrack && !options.keepOriginalTimestamps) {
                      audioTrack.timelineStartInfo = timelineStartInfo;
                      pipeline.audioSegmentStream.setEarliestDts(timelineStartInfo.dts - self2.baseMediaDecodeTime);
                    }
                  });
                  pipeline.videoSegmentStream.on("processedGopsInfo", self2.trigger.bind(self2, "gopInfo"));
                  pipeline.videoSegmentStream.on("segmentTimingInfo", self2.trigger.bind(self2, "videoSegmentTimingInfo"));
                  pipeline.videoSegmentStream.on("baseMediaDecodeTime", function(baseMediaDecodeTime) {
                    if (audioTrack) {
                      pipeline.audioSegmentStream.setVideoBaseMediaDecodeTime(baseMediaDecodeTime);
                    }
                  });
                  pipeline.videoSegmentStream.on("timingInfo", self2.trigger.bind(self2, "videoTimingInfo"));
                  pipeline.h264Stream.pipe(pipeline.videoSegmentStream).pipe(pipeline.coalesceStream);
                }
                if (audioTrack && !pipeline.audioSegmentStream) {
                  pipeline.coalesceStream.numberOfTracks++;
                  pipeline.audioSegmentStream = new _AudioSegmentStream$1(audioTrack, options);
                  pipeline.audioSegmentStream.on("log", self2.getLogTrigger_("audioSegmentStream"));
                  pipeline.audioSegmentStream.on("timingInfo", self2.trigger.bind(self2, "audioTimingInfo"));
                  pipeline.audioSegmentStream.on("segmentTimingInfo", self2.trigger.bind(self2, "audioSegmentTimingInfo"));
                  pipeline.adtsStream.pipe(pipeline.audioSegmentStream).pipe(pipeline.coalesceStream);
                }
                self2.trigger("trackinfo", {
                  hasAudio: !!audioTrack,
                  hasVideo: !!videoTrack
                });
              }
            });
            pipeline.coalesceStream.on("data", this.trigger.bind(this, "data"));
            pipeline.coalesceStream.on("id3Frame", function(id3Frame) {
              id3Frame.dispatchType = pipeline.metadataStream.dispatchType;
              self2.trigger("id3Frame", id3Frame);
            });
            pipeline.coalesceStream.on("caption", this.trigger.bind(this, "caption"));
            pipeline.coalesceStream.on("done", this.trigger.bind(this, "done"));
            addPipelineLogRetriggers(this, pipeline);
          };
          this.setBaseMediaDecodeTime = function(baseMediaDecodeTime) {
            var pipeline = this.transmuxPipeline_;
            if (!options.keepOriginalTimestamps) {
              this.baseMediaDecodeTime = baseMediaDecodeTime;
            }
            if (audioTrack) {
              audioTrack.timelineStartInfo.dts = void 0;
              audioTrack.timelineStartInfo.pts = void 0;
              trackDecodeInfo.clearDtsInfo(audioTrack);
              if (pipeline.audioTimestampRolloverStream) {
                pipeline.audioTimestampRolloverStream.discontinuity();
              }
            }
            if (videoTrack) {
              if (pipeline.videoSegmentStream) {
                pipeline.videoSegmentStream.gopCache_ = [];
              }
              videoTrack.timelineStartInfo.dts = void 0;
              videoTrack.timelineStartInfo.pts = void 0;
              trackDecodeInfo.clearDtsInfo(videoTrack);
              pipeline.captionStream.reset();
            }
            if (pipeline.timestampRolloverStream) {
              pipeline.timestampRolloverStream.discontinuity();
            }
          };
          this.setAudioAppendStart = function(timestamp) {
            if (audioTrack) {
              this.transmuxPipeline_.audioSegmentStream.setAudioAppendStart(timestamp);
            }
          };
          this.setRemux = function(val) {
            var pipeline = this.transmuxPipeline_;
            options.remux = val;
            if (pipeline && pipeline.coalesceStream) {
              pipeline.coalesceStream.setRemux(val);
            }
          };
          this.alignGopsWith = function(gopsToAlignWith) {
            if (videoTrack && this.transmuxPipeline_.videoSegmentStream) {
              this.transmuxPipeline_.videoSegmentStream.alignGopsWith(gopsToAlignWith);
            }
          };
          this.getLogTrigger_ = function(key) {
            var self3 = this;
            return function(event) {
              event.stream = key;
              self3.trigger("log", event);
            };
          };
          this.push = function(data) {
            if (hasFlushed) {
              var isAac = isLikelyAacData$1(data);
              if (isAac && this.transmuxPipeline_.type !== "aac") {
                this.setupAacPipeline();
              } else if (!isAac && this.transmuxPipeline_.type !== "ts") {
                this.setupTsPipeline();
              }
              hasFlushed = false;
            }
            this.transmuxPipeline_.headOfPipeline.push(data);
          };
          this.flush = function() {
            hasFlushed = true;
            this.transmuxPipeline_.headOfPipeline.flush();
          };
          this.endTimeline = function() {
            this.transmuxPipeline_.headOfPipeline.endTimeline();
          };
          this.reset = function() {
            if (this.transmuxPipeline_.headOfPipeline) {
              this.transmuxPipeline_.headOfPipeline.reset();
            }
          };
          this.resetCaptions = function() {
            if (this.transmuxPipeline_.captionStream) {
              this.transmuxPipeline_.captionStream.reset();
            }
          };
        };
        _Transmuxer$1.prototype = new stream();
        var transmuxer$2 = {
          Transmuxer: _Transmuxer$1,
          VideoSegmentStream: _VideoSegmentStream$1,
          AudioSegmentStream: _AudioSegmentStream$1,
          AUDIO_PROPERTIES: audioProperties,
          VIDEO_PROPERTIES: videoProperties,
          generateSegmentTimingInfo
        };
        var discardEmulationPreventionBytes = captionPacketParser.discardEmulationPreventionBytes;
        var CaptionStream = captionStream.CaptionStream;
        var mapToSample = function mapToSample2(offset, samples) {
          var approximateOffset = offset;
          for (var i = 0; i < samples.length; i++) {
            var sample = samples[i];
            if (approximateOffset < sample.size) {
              return sample;
            }
            approximateOffset -= sample.size;
          }
          return null;
        };
        var findSeiNals = function findSeiNals2(avcStream, samples, trackId) {
          var avcView = new DataView(avcStream.buffer, avcStream.byteOffset, avcStream.byteLength), result = {
            logs: [],
            seiNals: []
          }, seiNal, i, length, lastMatchedSample;
          for (i = 0; i + 4 < avcStream.length; i += length) {
            length = avcView.getUint32(i);
            i += 4;
            if (length <= 0) {
              continue;
            }
            switch (avcStream[i] & 31) {
              case 6:
                var data = avcStream.subarray(i + 1, i + 1 + length);
                var matchingSample = mapToSample(i, samples);
                seiNal = {
                  nalUnitType: "sei_rbsp",
                  size: length,
                  data,
                  escapedRBSP: discardEmulationPreventionBytes(data),
                  trackId
                };
                if (matchingSample) {
                  seiNal.pts = matchingSample.pts;
                  seiNal.dts = matchingSample.dts;
                  lastMatchedSample = matchingSample;
                } else if (lastMatchedSample) {
                  seiNal.pts = lastMatchedSample.pts;
                  seiNal.dts = lastMatchedSample.dts;
                } else {
                  result.logs.push({
                    level: "warn",
                    message: "We've encountered a nal unit without data at " + i + " for trackId " + trackId + ". See mux.js#223."
                  });
                  break;
                }
                result.seiNals.push(seiNal);
                break;
            }
          }
          return result;
        };
        var parseSamples = function parseSamples2(truns, baseMediaDecodeTime, tfhd2) {
          var currentDts = baseMediaDecodeTime;
          var defaultSampleDuration = tfhd2.defaultSampleDuration || 0;
          var defaultSampleSize = tfhd2.defaultSampleSize || 0;
          var trackId = tfhd2.trackId;
          var allSamples = [];
          truns.forEach(function(trun2) {
            var trackRun = parseTrun(trun2);
            var samples = trackRun.samples;
            samples.forEach(function(sample) {
              if (sample.duration === void 0) {
                sample.duration = defaultSampleDuration;
              }
              if (sample.size === void 0) {
                sample.size = defaultSampleSize;
              }
              sample.trackId = trackId;
              sample.dts = currentDts;
              if (sample.compositionTimeOffset === void 0) {
                sample.compositionTimeOffset = 0;
              }
              if (typeof currentDts === "bigint") {
                sample.pts = currentDts + window__default["default"].BigInt(sample.compositionTimeOffset);
                currentDts += window__default["default"].BigInt(sample.duration);
              } else {
                sample.pts = currentDts + sample.compositionTimeOffset;
                currentDts += sample.duration;
              }
            });
            allSamples = allSamples.concat(samples);
          });
          return allSamples;
        };
        var parseCaptionNals = function parseCaptionNals2(segment, videoTrackId) {
          var trafs = findBox_1(segment, ["moof", "traf"]);
          var mdats = findBox_1(segment, ["mdat"]);
          var captionNals = {};
          var mdatTrafPairs = [];
          mdats.forEach(function(mdat2, index) {
            var matchingTraf = trafs[index];
            mdatTrafPairs.push({
              mdat: mdat2,
              traf: matchingTraf
            });
          });
          mdatTrafPairs.forEach(function(pair) {
            var mdat2 = pair.mdat;
            var traf2 = pair.traf;
            var tfhd2 = findBox_1(traf2, ["tfhd"]);
            var headerInfo = parseTfhd(tfhd2[0]);
            var trackId = headerInfo.trackId;
            var tfdt2 = findBox_1(traf2, ["tfdt"]);
            var baseMediaDecodeTime = tfdt2.length > 0 ? parseTfdt(tfdt2[0]).baseMediaDecodeTime : 0;
            var truns = findBox_1(traf2, ["trun"]);
            var samples;
            var result;
            if (videoTrackId === trackId && truns.length > 0) {
              samples = parseSamples(truns, baseMediaDecodeTime, headerInfo);
              result = findSeiNals(mdat2, samples, trackId);
              if (!captionNals[trackId]) {
                captionNals[trackId] = {
                  seiNals: [],
                  logs: []
                };
              }
              captionNals[trackId].seiNals = captionNals[trackId].seiNals.concat(result.seiNals);
              captionNals[trackId].logs = captionNals[trackId].logs.concat(result.logs);
            }
          });
          return captionNals;
        };
        var parseEmbeddedCaptions = function parseEmbeddedCaptions2(segment, trackId, timescale2) {
          var captionNals;
          if (trackId === null) {
            return null;
          }
          captionNals = parseCaptionNals(segment, trackId);
          var trackNals = captionNals[trackId] || {};
          return {
            seiNals: trackNals.seiNals,
            logs: trackNals.logs,
            timescale: timescale2
          };
        };
        var CaptionParser = function CaptionParser2() {
          var isInitialized = false;
          var captionStream2;
          var segmentCache;
          var trackId;
          var timescale2;
          var parsedCaptions;
          var parsingPartial;
          this.isInitialized = function() {
            return isInitialized;
          };
          this.init = function(options) {
            captionStream2 = new CaptionStream();
            isInitialized = true;
            parsingPartial = options ? options.isPartial : false;
            captionStream2.on("data", function(event) {
              event.startTime = event.startPts / timescale2;
              event.endTime = event.endPts / timescale2;
              parsedCaptions.captions.push(event);
              parsedCaptions.captionStreams[event.stream] = true;
            });
            captionStream2.on("log", function(log2) {
              parsedCaptions.logs.push(log2);
            });
          };
          this.isNewInit = function(videoTrackIds, timescales) {
            if (videoTrackIds && videoTrackIds.length === 0 || timescales && typeof timescales === "object" && Object.keys(timescales).length === 0) {
              return false;
            }
            return trackId !== videoTrackIds[0] || timescale2 !== timescales[trackId];
          };
          this.parse = function(segment, videoTrackIds, timescales) {
            var parsedData;
            if (!this.isInitialized()) {
              return null;
            } else if (!videoTrackIds || !timescales) {
              return null;
            } else if (this.isNewInit(videoTrackIds, timescales)) {
              trackId = videoTrackIds[0];
              timescale2 = timescales[trackId];
            } else if (trackId === null || !timescale2) {
              segmentCache.push(segment);
              return null;
            }
            while (segmentCache.length > 0) {
              var cachedSegment = segmentCache.shift();
              this.parse(cachedSegment, videoTrackIds, timescales);
            }
            parsedData = parseEmbeddedCaptions(segment, trackId, timescale2);
            if (parsedData && parsedData.logs) {
              parsedCaptions.logs = parsedCaptions.logs.concat(parsedData.logs);
            }
            if (parsedData === null || !parsedData.seiNals) {
              if (parsedCaptions.logs.length) {
                return {
                  logs: parsedCaptions.logs,
                  captions: [],
                  captionStreams: []
                };
              }
              return null;
            }
            this.pushNals(parsedData.seiNals);
            this.flushStream();
            return parsedCaptions;
          };
          this.pushNals = function(nals) {
            if (!this.isInitialized() || !nals || nals.length === 0) {
              return null;
            }
            nals.forEach(function(nal) {
              captionStream2.push(nal);
            });
          };
          this.flushStream = function() {
            if (!this.isInitialized()) {
              return null;
            }
            if (!parsingPartial) {
              captionStream2.flush();
            } else {
              captionStream2.partialFlush();
            }
          };
          this.clearParsedCaptions = function() {
            parsedCaptions.captions = [];
            parsedCaptions.captionStreams = {};
            parsedCaptions.logs = [];
          };
          this.resetCaptionStream = function() {
            if (!this.isInitialized()) {
              return null;
            }
            captionStream2.reset();
          };
          this.clearAllCaptions = function() {
            this.clearParsedCaptions();
            this.resetCaptionStream();
          };
          this.reset = function() {
            segmentCache = [];
            trackId = null;
            timescale2 = null;
            if (!parsedCaptions) {
              parsedCaptions = {
                captions: [],
                captionStreams: {},
                logs: []
              };
            } else {
              this.clearParsedCaptions();
            }
            this.resetCaptionStream();
          };
          this.reset();
        };
        var captionParser = CaptionParser;
        var mp4 = {
          generator: mp4Generator,
          probe: probe$2,
          Transmuxer: transmuxer$2.Transmuxer,
          AudioSegmentStream: transmuxer$2.AudioSegmentStream,
          VideoSegmentStream: transmuxer$2.VideoSegmentStream,
          CaptionParser: captionParser
        };
        var _FlvTag;
        _FlvTag = function FlvTag(type2, extraData) {
          var adHoc = 0, bufferStartSize = 16384, prepareWrite = function prepareWrite2(flv2, count) {
            var bytes, minLength = flv2.position + count;
            if (minLength < flv2.bytes.byteLength) {
              return;
            }
            bytes = new Uint8Array(minLength * 2);
            bytes.set(flv2.bytes.subarray(0, flv2.position), 0);
            flv2.bytes = bytes;
            flv2.view = new DataView(flv2.bytes.buffer);
          }, widthBytes = _FlvTag.widthBytes || new Uint8Array("width".length), heightBytes = _FlvTag.heightBytes || new Uint8Array("height".length), videocodecidBytes = _FlvTag.videocodecidBytes || new Uint8Array("videocodecid".length), i;
          if (!_FlvTag.widthBytes) {
            for (i = 0; i < "width".length; i++) {
              widthBytes[i] = "width".charCodeAt(i);
            }
            for (i = 0; i < "height".length; i++) {
              heightBytes[i] = "height".charCodeAt(i);
            }
            for (i = 0; i < "videocodecid".length; i++) {
              videocodecidBytes[i] = "videocodecid".charCodeAt(i);
            }
            _FlvTag.widthBytes = widthBytes;
            _FlvTag.heightBytes = heightBytes;
            _FlvTag.videocodecidBytes = videocodecidBytes;
          }
          this.keyFrame = false;
          switch (type2) {
            case _FlvTag.VIDEO_TAG:
              this.length = 16;
              bufferStartSize *= 6;
              break;
            case _FlvTag.AUDIO_TAG:
              this.length = 13;
              this.keyFrame = true;
              break;
            case _FlvTag.METADATA_TAG:
              this.length = 29;
              this.keyFrame = true;
              break;
            default:
              throw new Error("Unknown FLV tag type");
          }
          this.bytes = new Uint8Array(bufferStartSize);
          this.view = new DataView(this.bytes.buffer);
          this.bytes[0] = type2;
          this.position = this.length;
          this.keyFrame = extraData;
          this.pts = 0;
          this.dts = 0;
          this.writeBytes = function(bytes, offset, length) {
            var start = offset || 0, end;
            length = length || bytes.byteLength;
            end = start + length;
            prepareWrite(this, length);
            this.bytes.set(bytes.subarray(start, end), this.position);
            this.position += length;
            this.length = Math.max(this.length, this.position);
          };
          this.writeByte = function(byte) {
            prepareWrite(this, 1);
            this.bytes[this.position] = byte;
            this.position++;
            this.length = Math.max(this.length, this.position);
          };
          this.writeShort = function(short) {
            prepareWrite(this, 2);
            this.view.setUint16(this.position, short);
            this.position += 2;
            this.length = Math.max(this.length, this.position);
          };
          this.negIndex = function(pos) {
            return this.bytes[this.length - pos];
          };
          this.nalUnitSize = function() {
            if (adHoc === 0) {
              return 0;
            }
            return this.length - (adHoc + 4);
          };
          this.startNalUnit = function() {
            if (adHoc > 0) {
              throw new Error("Attempted to create new NAL wihout closing the old one");
            }
            adHoc = this.length;
            this.length += 4;
            this.position = this.length;
          };
          this.endNalUnit = function(nalContainer) {
            var nalStart, nalLength;
            if (this.length === adHoc + 4) {
              this.length -= 4;
            } else if (adHoc > 0) {
              nalStart = adHoc + 4;
              nalLength = this.length - nalStart;
              this.position = adHoc;
              this.view.setUint32(this.position, nalLength);
              this.position = this.length;
              if (nalContainer) {
                nalContainer.push(this.bytes.subarray(nalStart, nalStart + nalLength));
              }
            }
            adHoc = 0;
          };
          this.writeMetaDataDouble = function(key, val) {
            var i2;
            prepareWrite(this, 2 + key.length + 9);
            this.view.setUint16(this.position, key.length);
            this.position += 2;
            if (key === "width") {
              this.bytes.set(widthBytes, this.position);
              this.position += 5;
            } else if (key === "height") {
              this.bytes.set(heightBytes, this.position);
              this.position += 6;
            } else if (key === "videocodecid") {
              this.bytes.set(videocodecidBytes, this.position);
              this.position += 12;
            } else {
              for (i2 = 0; i2 < key.length; i2++) {
                this.bytes[this.position] = key.charCodeAt(i2);
                this.position++;
              }
            }
            this.position++;
            this.view.setFloat64(this.position, val);
            this.position += 8;
            this.length = Math.max(this.length, this.position);
            ++adHoc;
          };
          this.writeMetaDataBoolean = function(key, val) {
            var i2;
            prepareWrite(this, 2);
            this.view.setUint16(this.position, key.length);
            this.position += 2;
            for (i2 = 0; i2 < key.length; i2++) {
              prepareWrite(this, 1);
              this.bytes[this.position] = key.charCodeAt(i2);
              this.position++;
            }
            prepareWrite(this, 2);
            this.view.setUint8(this.position, 1);
            this.position++;
            this.view.setUint8(this.position, val ? 1 : 0);
            this.position++;
            this.length = Math.max(this.length, this.position);
            ++adHoc;
          };
          this.finalize = function() {
            var dtsDelta, len;
            switch (this.bytes[0]) {
              case _FlvTag.VIDEO_TAG:
                this.bytes[11] = (this.keyFrame || extraData ? 16 : 32) | 7;
                this.bytes[12] = extraData ? 0 : 1;
                dtsDelta = this.pts - this.dts;
                this.bytes[13] = (dtsDelta & 16711680) >>> 16;
                this.bytes[14] = (dtsDelta & 65280) >>> 8;
                this.bytes[15] = (dtsDelta & 255) >>> 0;
                break;
              case _FlvTag.AUDIO_TAG:
                this.bytes[11] = 175;
                this.bytes[12] = extraData ? 0 : 1;
                break;
              case _FlvTag.METADATA_TAG:
                this.position = 11;
                this.view.setUint8(this.position, 2);
                this.position++;
                this.view.setUint16(this.position, 10);
                this.position += 2;
                this.bytes.set([111, 110, 77, 101, 116, 97, 68, 97, 116, 97], this.position);
                this.position += 10;
                this.bytes[this.position] = 8;
                this.position++;
                this.view.setUint32(this.position, adHoc);
                this.position = this.length;
                this.bytes.set([0, 0, 9], this.position);
                this.position += 3;
                this.length = this.position;
                break;
            }
            len = this.length - 11;
            this.bytes[1] = (len & 16711680) >>> 16;
            this.bytes[2] = (len & 65280) >>> 8;
            this.bytes[3] = (len & 255) >>> 0;
            this.bytes[4] = (this.dts & 16711680) >>> 16;
            this.bytes[5] = (this.dts & 65280) >>> 8;
            this.bytes[6] = (this.dts & 255) >>> 0;
            this.bytes[7] = (this.dts & 4278190080) >>> 24;
            this.bytes[8] = 0;
            this.bytes[9] = 0;
            this.bytes[10] = 0;
            prepareWrite(this, 4);
            this.view.setUint32(this.length, this.length);
            this.length += 4;
            this.position += 4;
            this.bytes = this.bytes.subarray(0, this.length);
            this.frameTime = _FlvTag.frameTime(this.bytes);
            return this;
          };
        };
        _FlvTag.AUDIO_TAG = 8;
        _FlvTag.VIDEO_TAG = 9;
        _FlvTag.METADATA_TAG = 18;
        _FlvTag.isAudioFrame = function(tag) {
          return _FlvTag.AUDIO_TAG === tag[0];
        };
        _FlvTag.isVideoFrame = function(tag) {
          return _FlvTag.VIDEO_TAG === tag[0];
        };
        _FlvTag.isMetaData = function(tag) {
          return _FlvTag.METADATA_TAG === tag[0];
        };
        _FlvTag.isKeyFrame = function(tag) {
          if (_FlvTag.isVideoFrame(tag)) {
            return tag[11] === 23;
          }
          if (_FlvTag.isAudioFrame(tag)) {
            return true;
          }
          if (_FlvTag.isMetaData(tag)) {
            return true;
          }
          return false;
        };
        _FlvTag.frameTime = function(tag) {
          var pts = tag[4] << 16;
          pts |= tag[5] << 8;
          pts |= tag[6] << 0;
          pts |= tag[7] << 24;
          return pts;
        };
        var flvTag = _FlvTag;
        var CoalesceStream = function CoalesceStream2(options) {
          this.numberOfTracks = 0;
          this.metadataStream = options.metadataStream;
          this.videoTags = [];
          this.audioTags = [];
          this.videoTrack = null;
          this.audioTrack = null;
          this.pendingCaptions = [];
          this.pendingMetadata = [];
          this.pendingTracks = 0;
          this.processedTracks = 0;
          CoalesceStream2.prototype.init.call(this);
          this.push = function(output) {
            if (output.text) {
              return this.pendingCaptions.push(output);
            }
            if (output.frames) {
              return this.pendingMetadata.push(output);
            }
            if (output.track.type === "video") {
              this.videoTrack = output.track;
              this.videoTags = output.tags;
              this.pendingTracks++;
            }
            if (output.track.type === "audio") {
              this.audioTrack = output.track;
              this.audioTags = output.tags;
              this.pendingTracks++;
            }
          };
        };
        CoalesceStream.prototype = new stream();
        CoalesceStream.prototype.flush = function(flushSource) {
          var id3, caption, i, timelineStartPts, event = {
            tags: {},
            captions: [],
            captionStreams: {},
            metadata: []
          };
          if (this.pendingTracks < this.numberOfTracks) {
            if (flushSource !== "VideoSegmentStream" && flushSource !== "AudioSegmentStream") {
              return;
            } else if (this.pendingTracks === 0) {
              this.processedTracks++;
              if (this.processedTracks < this.numberOfTracks) {
                return;
              }
            }
          }
          this.processedTracks += this.pendingTracks;
          this.pendingTracks = 0;
          if (this.processedTracks < this.numberOfTracks) {
            return;
          }
          if (this.videoTrack) {
            timelineStartPts = this.videoTrack.timelineStartInfo.pts;
          } else if (this.audioTrack) {
            timelineStartPts = this.audioTrack.timelineStartInfo.pts;
          }
          event.tags.videoTags = this.videoTags;
          event.tags.audioTags = this.audioTags;
          for (i = 0; i < this.pendingCaptions.length; i++) {
            caption = this.pendingCaptions[i];
            caption.startTime = caption.startPts - timelineStartPts;
            caption.startTime /= 9e4;
            caption.endTime = caption.endPts - timelineStartPts;
            caption.endTime /= 9e4;
            event.captionStreams[caption.stream] = true;
            event.captions.push(caption);
          }
          for (i = 0; i < this.pendingMetadata.length; i++) {
            id3 = this.pendingMetadata[i];
            id3.cueTime = id3.pts - timelineStartPts;
            id3.cueTime /= 9e4;
            event.metadata.push(id3);
          }
          event.metadata.dispatchType = this.metadataStream.dispatchType;
          this.videoTrack = null;
          this.audioTrack = null;
          this.videoTags = [];
          this.audioTags = [];
          this.pendingCaptions.length = 0;
          this.pendingMetadata.length = 0;
          this.pendingTracks = 0;
          this.processedTracks = 0;
          this.trigger("data", event);
          this.trigger("done");
        };
        var coalesceStream = CoalesceStream;
        var TagList = function TagList2() {
          var self2 = this;
          this.list = [];
          this.push = function(tag) {
            this.list.push({
              bytes: tag.bytes,
              dts: tag.dts,
              pts: tag.pts,
              keyFrame: tag.keyFrame,
              metaDataTag: tag.metaDataTag
            });
          };
          Object.defineProperty(this, "length", {
            get: function get() {
              return self2.list.length;
            }
          });
        };
        var tagList = TagList;
        var H264Stream = h264.H264Stream;
        var _Transmuxer, _VideoSegmentStream, _AudioSegmentStream, collectTimelineInfo, metaDataTag, extraDataTag;
        collectTimelineInfo = function collectTimelineInfo2(track, data) {
          if (typeof data.pts === "number") {
            if (track.timelineStartInfo.pts === void 0) {
              track.timelineStartInfo.pts = data.pts;
            } else {
              track.timelineStartInfo.pts = Math.min(track.timelineStartInfo.pts, data.pts);
            }
          }
          if (typeof data.dts === "number") {
            if (track.timelineStartInfo.dts === void 0) {
              track.timelineStartInfo.dts = data.dts;
            } else {
              track.timelineStartInfo.dts = Math.min(track.timelineStartInfo.dts, data.dts);
            }
          }
        };
        metaDataTag = function metaDataTag2(track, pts) {
          var tag = new flvTag(flvTag.METADATA_TAG);
          tag.dts = pts;
          tag.pts = pts;
          tag.writeMetaDataDouble("videocodecid", 7);
          tag.writeMetaDataDouble("width", track.width);
          tag.writeMetaDataDouble("height", track.height);
          return tag;
        };
        extraDataTag = function extraDataTag2(track, pts) {
          var i, tag = new flvTag(flvTag.VIDEO_TAG, true);
          tag.dts = pts;
          tag.pts = pts;
          tag.writeByte(1);
          tag.writeByte(track.profileIdc);
          tag.writeByte(track.profileCompatibility);
          tag.writeByte(track.levelIdc);
          tag.writeByte(252 | 3);
          tag.writeByte(224 | 1);
          tag.writeShort(track.sps[0].length);
          tag.writeBytes(track.sps[0]);
          tag.writeByte(track.pps.length);
          for (i = 0; i < track.pps.length; ++i) {
            tag.writeShort(track.pps[i].length);
            tag.writeBytes(track.pps[i]);
          }
          return tag;
        };
        _AudioSegmentStream = function AudioSegmentStream2(track) {
          var adtsFrames = [], videoKeyFrames = [], oldExtraData;
          _AudioSegmentStream.prototype.init.call(this);
          this.push = function(data) {
            collectTimelineInfo(track, data);
            if (track) {
              track.audioobjecttype = data.audioobjecttype;
              track.channelcount = data.channelcount;
              track.samplerate = data.samplerate;
              track.samplingfrequencyindex = data.samplingfrequencyindex;
              track.samplesize = data.samplesize;
              track.extraData = track.audioobjecttype << 11 | track.samplingfrequencyindex << 7 | track.channelcount << 3;
            }
            data.pts = Math.round(data.pts / 90);
            data.dts = Math.round(data.dts / 90);
            adtsFrames.push(data);
          };
          this.flush = function() {
            var currentFrame, adtsFrame, lastMetaPts, tags = new tagList();
            if (adtsFrames.length === 0) {
              this.trigger("done", "AudioSegmentStream");
              return;
            }
            lastMetaPts = -Infinity;
            while (adtsFrames.length) {
              currentFrame = adtsFrames.shift();
              if (videoKeyFrames.length && currentFrame.pts >= videoKeyFrames[0]) {
                lastMetaPts = videoKeyFrames.shift();
                this.writeMetaDataTags(tags, lastMetaPts);
              }
              if (track.extraData !== oldExtraData || currentFrame.pts - lastMetaPts >= 1e3) {
                this.writeMetaDataTags(tags, currentFrame.pts);
                oldExtraData = track.extraData;
                lastMetaPts = currentFrame.pts;
              }
              adtsFrame = new flvTag(flvTag.AUDIO_TAG);
              adtsFrame.pts = currentFrame.pts;
              adtsFrame.dts = currentFrame.dts;
              adtsFrame.writeBytes(currentFrame.data);
              tags.push(adtsFrame.finalize());
            }
            videoKeyFrames.length = 0;
            oldExtraData = null;
            this.trigger("data", {
              track,
              tags: tags.list
            });
            this.trigger("done", "AudioSegmentStream");
          };
          this.writeMetaDataTags = function(tags, pts) {
            var adtsFrame;
            adtsFrame = new flvTag(flvTag.METADATA_TAG);
            adtsFrame.pts = pts;
            adtsFrame.dts = pts;
            adtsFrame.writeMetaDataDouble("audiocodecid", 10);
            adtsFrame.writeMetaDataBoolean("stereo", track.channelcount === 2);
            adtsFrame.writeMetaDataDouble("audiosamplerate", track.samplerate);
            adtsFrame.writeMetaDataDouble("audiosamplesize", 16);
            tags.push(adtsFrame.finalize());
            adtsFrame = new flvTag(flvTag.AUDIO_TAG, true);
            adtsFrame.pts = pts;
            adtsFrame.dts = pts;
            adtsFrame.view.setUint16(adtsFrame.position, track.extraData);
            adtsFrame.position += 2;
            adtsFrame.length = Math.max(adtsFrame.length, adtsFrame.position);
            tags.push(adtsFrame.finalize());
          };
          this.onVideoKeyFrame = function(pts) {
            videoKeyFrames.push(pts);
          };
        };
        _AudioSegmentStream.prototype = new stream();
        _VideoSegmentStream = function VideoSegmentStream2(track) {
          var nalUnits = [], config, h264Frame;
          _VideoSegmentStream.prototype.init.call(this);
          this.finishFrame = function(tags, frame) {
            if (!frame) {
              return;
            }
            if (config && track && track.newMetadata && (frame.keyFrame || tags.length === 0)) {
              var metaTag = metaDataTag(config, frame.dts).finalize();
              var extraTag = extraDataTag(track, frame.dts).finalize();
              metaTag.metaDataTag = extraTag.metaDataTag = true;
              tags.push(metaTag);
              tags.push(extraTag);
              track.newMetadata = false;
              this.trigger("keyframe", frame.dts);
            }
            frame.endNalUnit();
            tags.push(frame.finalize());
            h264Frame = null;
          };
          this.push = function(data) {
            collectTimelineInfo(track, data);
            data.pts = Math.round(data.pts / 90);
            data.dts = Math.round(data.dts / 90);
            nalUnits.push(data);
          };
          this.flush = function() {
            var currentNal, tags = new tagList();
            while (nalUnits.length) {
              if (nalUnits[0].nalUnitType === "access_unit_delimiter_rbsp") {
                break;
              }
              nalUnits.shift();
            }
            if (nalUnits.length === 0) {
              this.trigger("done", "VideoSegmentStream");
              return;
            }
            while (nalUnits.length) {
              currentNal = nalUnits.shift();
              if (currentNal.nalUnitType === "seq_parameter_set_rbsp") {
                track.newMetadata = true;
                config = currentNal.config;
                track.width = config.width;
                track.height = config.height;
                track.sps = [currentNal.data];
                track.profileIdc = config.profileIdc;
                track.levelIdc = config.levelIdc;
                track.profileCompatibility = config.profileCompatibility;
                h264Frame.endNalUnit();
              } else if (currentNal.nalUnitType === "pic_parameter_set_rbsp") {
                track.newMetadata = true;
                track.pps = [currentNal.data];
                h264Frame.endNalUnit();
              } else if (currentNal.nalUnitType === "access_unit_delimiter_rbsp") {
                if (h264Frame) {
                  this.finishFrame(tags, h264Frame);
                }
                h264Frame = new flvTag(flvTag.VIDEO_TAG);
                h264Frame.pts = currentNal.pts;
                h264Frame.dts = currentNal.dts;
              } else {
                if (currentNal.nalUnitType === "slice_layer_without_partitioning_rbsp_idr") {
                  h264Frame.keyFrame = true;
                }
                h264Frame.endNalUnit();
              }
              h264Frame.startNalUnit();
              h264Frame.writeBytes(currentNal.data);
            }
            if (h264Frame) {
              this.finishFrame(tags, h264Frame);
            }
            this.trigger("data", {
              track,
              tags: tags.list
            });
            this.trigger("done", "VideoSegmentStream");
          };
        };
        _VideoSegmentStream.prototype = new stream();
        _Transmuxer = function Transmuxer2(options) {
          var self2 = this, packetStream, parseStream, elementaryStream, videoTimestampRolloverStream, audioTimestampRolloverStream, timedMetadataTimestampRolloverStream, adtsStream, h264Stream, videoSegmentStream2, audioSegmentStream2, captionStream2, coalesceStream$1;
          _Transmuxer.prototype.init.call(this);
          options = options || {};
          this.metadataStream = new m2ts_1.MetadataStream();
          options.metadataStream = this.metadataStream;
          packetStream = new m2ts_1.TransportPacketStream();
          parseStream = new m2ts_1.TransportParseStream();
          elementaryStream = new m2ts_1.ElementaryStream();
          videoTimestampRolloverStream = new m2ts_1.TimestampRolloverStream("video");
          audioTimestampRolloverStream = new m2ts_1.TimestampRolloverStream("audio");
          timedMetadataTimestampRolloverStream = new m2ts_1.TimestampRolloverStream("timed-metadata");
          adtsStream = new adts();
          h264Stream = new H264Stream();
          coalesceStream$1 = new coalesceStream(options);
          packetStream.pipe(parseStream).pipe(elementaryStream);
          elementaryStream.pipe(videoTimestampRolloverStream).pipe(h264Stream);
          elementaryStream.pipe(audioTimestampRolloverStream).pipe(adtsStream);
          elementaryStream.pipe(timedMetadataTimestampRolloverStream).pipe(this.metadataStream).pipe(coalesceStream$1);
          captionStream2 = new m2ts_1.CaptionStream(options);
          h264Stream.pipe(captionStream2).pipe(coalesceStream$1);
          elementaryStream.on("data", function(data) {
            var i, videoTrack, audioTrack;
            if (data.type === "metadata") {
              i = data.tracks.length;
              while (i--) {
                if (data.tracks[i].type === "video") {
                  videoTrack = data.tracks[i];
                } else if (data.tracks[i].type === "audio") {
                  audioTrack = data.tracks[i];
                }
              }
              if (videoTrack && !videoSegmentStream2) {
                coalesceStream$1.numberOfTracks++;
                videoSegmentStream2 = new _VideoSegmentStream(videoTrack);
                h264Stream.pipe(videoSegmentStream2).pipe(coalesceStream$1);
              }
              if (audioTrack && !audioSegmentStream2) {
                coalesceStream$1.numberOfTracks++;
                audioSegmentStream2 = new _AudioSegmentStream(audioTrack);
                adtsStream.pipe(audioSegmentStream2).pipe(coalesceStream$1);
                if (videoSegmentStream2) {
                  videoSegmentStream2.on("keyframe", audioSegmentStream2.onVideoKeyFrame);
                }
              }
            }
          });
          this.push = function(data) {
            packetStream.push(data);
          };
          this.flush = function() {
            packetStream.flush();
          };
          this.resetCaptions = function() {
            captionStream2.reset();
          };
          coalesceStream$1.on("data", function(event) {
            self2.trigger("data", event);
          });
          coalesceStream$1.on("done", function() {
            self2.trigger("done");
          });
        };
        _Transmuxer.prototype = new stream();
        var transmuxer$1 = _Transmuxer;
        var getFlvHeader = function getFlvHeader2(duration, audio, video) {
          var headBytes = new Uint8Array(3 + 1 + 1 + 4), head = new DataView(headBytes.buffer), metadata, result, metadataLength;
          duration = duration || 0;
          audio = audio === void 0 ? true : audio;
          video = video === void 0 ? true : video;
          head.setUint8(0, 70);
          head.setUint8(1, 76);
          head.setUint8(2, 86);
          head.setUint8(3, 1);
          head.setUint8(4, (audio ? 4 : 0) | (video ? 1 : 0));
          head.setUint32(5, headBytes.byteLength);
          if (duration <= 0) {
            result = new Uint8Array(headBytes.byteLength + 4);
            result.set(headBytes);
            result.set([0, 0, 0, 0], headBytes.byteLength);
            return result;
          }
          metadata = new flvTag(flvTag.METADATA_TAG);
          metadata.pts = metadata.dts = 0;
          metadata.writeMetaDataDouble("duration", duration);
          metadataLength = metadata.finalize().length;
          result = new Uint8Array(headBytes.byteLength + metadataLength);
          result.set(headBytes);
          result.set(head.byteLength, metadataLength);
          return result;
        };
        var flvHeader = getFlvHeader;
        var flv = {
          tag: flvTag,
          Transmuxer: transmuxer$1,
          getFlvHeader: flvHeader
        };
        var m2ts = m2ts_1;
        var ONE_SECOND_IN_TS$1 = clock.ONE_SECOND_IN_TS;
        var AudioSegmentStream = function AudioSegmentStream2(track, options) {
          var adtsFrames = [], sequenceNumber = 0, earliestAllowedDts = 0, audioAppendStartTs = 0, videoBaseMediaDecodeTime = Infinity, segmentStartPts = null, segmentEndPts = null;
          options = options || {};
          AudioSegmentStream2.prototype.init.call(this);
          this.push = function(data) {
            trackDecodeInfo.collectDtsInfo(track, data);
            if (track) {
              audioProperties.forEach(function(prop) {
                track[prop] = data[prop];
              });
            }
            adtsFrames.push(data);
          };
          this.setEarliestDts = function(earliestDts) {
            earliestAllowedDts = earliestDts;
          };
          this.setVideoBaseMediaDecodeTime = function(baseMediaDecodeTime) {
            videoBaseMediaDecodeTime = baseMediaDecodeTime;
          };
          this.setAudioAppendStart = function(timestamp) {
            audioAppendStartTs = timestamp;
          };
          this.processFrames_ = function() {
            var frames, moof2, mdat2, boxes, timingInfo;
            if (adtsFrames.length === 0) {
              return;
            }
            frames = audioFrameUtils.trimAdtsFramesByEarliestDts(adtsFrames, track, earliestAllowedDts);
            if (frames.length === 0) {
              return;
            }
            track.baseMediaDecodeTime = trackDecodeInfo.calculateTrackBaseMediaDecodeTime(track, options.keepOriginalTimestamps);
            audioFrameUtils.prefixWithSilence(track, frames, audioAppendStartTs, videoBaseMediaDecodeTime);
            track.samples = audioFrameUtils.generateSampleTable(frames);
            mdat2 = mp4Generator.mdat(audioFrameUtils.concatenateFrameData(frames));
            adtsFrames = [];
            moof2 = mp4Generator.moof(sequenceNumber, [track]);
            sequenceNumber++;
            track.initSegment = mp4Generator.initSegment([track]);
            boxes = new Uint8Array(moof2.byteLength + mdat2.byteLength);
            boxes.set(moof2);
            boxes.set(mdat2, moof2.byteLength);
            trackDecodeInfo.clearDtsInfo(track);
            if (segmentStartPts === null) {
              segmentEndPts = segmentStartPts = frames[0].pts;
            }
            segmentEndPts += frames.length * (ONE_SECOND_IN_TS$1 * 1024 / track.samplerate);
            timingInfo = {
              start: segmentStartPts
            };
            this.trigger("timingInfo", timingInfo);
            this.trigger("data", {
              track,
              boxes
            });
          };
          this.flush = function() {
            this.processFrames_();
            this.trigger("timingInfo", {
              start: segmentStartPts,
              end: segmentEndPts
            });
            this.resetTiming_();
            this.trigger("done", "AudioSegmentStream");
          };
          this.partialFlush = function() {
            this.processFrames_();
            this.trigger("partialdone", "AudioSegmentStream");
          };
          this.endTimeline = function() {
            this.flush();
            this.trigger("endedtimeline", "AudioSegmentStream");
          };
          this.resetTiming_ = function() {
            trackDecodeInfo.clearDtsInfo(track);
            segmentStartPts = null;
            segmentEndPts = null;
          };
          this.reset = function() {
            this.resetTiming_();
            adtsFrames = [];
            this.trigger("reset");
          };
        };
        AudioSegmentStream.prototype = new stream();
        var audioSegmentStream = AudioSegmentStream;
        var VideoSegmentStream = function VideoSegmentStream2(track, options) {
          var sequenceNumber = 0, nalUnits = [], frameCache = [], config, pps, segmentStartPts = null, segmentEndPts = null, gops, ensureNextFrameIsKeyFrame = true;
          options = options || {};
          VideoSegmentStream2.prototype.init.call(this);
          this.push = function(nalUnit) {
            trackDecodeInfo.collectDtsInfo(track, nalUnit);
            if (typeof track.timelineStartInfo.dts === "undefined") {
              track.timelineStartInfo.dts = nalUnit.dts;
            }
            if (nalUnit.nalUnitType === "seq_parameter_set_rbsp" && !config) {
              config = nalUnit.config;
              track.sps = [nalUnit.data];
              videoProperties.forEach(function(prop) {
                track[prop] = config[prop];
              }, this);
            }
            if (nalUnit.nalUnitType === "pic_parameter_set_rbsp" && !pps) {
              pps = nalUnit.data;
              track.pps = [nalUnit.data];
            }
            nalUnits.push(nalUnit);
          };
          this.processNals_ = function(cacheLastFrame) {
            var i;
            nalUnits = frameCache.concat(nalUnits);
            while (nalUnits.length) {
              if (nalUnits[0].nalUnitType === "access_unit_delimiter_rbsp") {
                break;
              }
              nalUnits.shift();
            }
            if (nalUnits.length === 0) {
              return;
            }
            var frames = frameUtils.groupNalsIntoFrames(nalUnits);
            if (!frames.length) {
              return;
            }
            frameCache = frames[frames.length - 1];
            if (cacheLastFrame) {
              frames.pop();
              frames.duration -= frameCache.duration;
              frames.nalCount -= frameCache.length;
              frames.byteLength -= frameCache.byteLength;
            }
            if (!frames.length) {
              nalUnits = [];
              return;
            }
            this.trigger("timelineStartInfo", track.timelineStartInfo);
            if (ensureNextFrameIsKeyFrame) {
              gops = frameUtils.groupFramesIntoGops(frames);
              if (!gops[0][0].keyFrame) {
                gops = frameUtils.extendFirstKeyFrame(gops);
                if (!gops[0][0].keyFrame) {
                  nalUnits = [].concat.apply([], frames).concat(frameCache);
                  frameCache = [];
                  return;
                }
                frames = [].concat.apply([], gops);
                frames.duration = gops.duration;
              }
              ensureNextFrameIsKeyFrame = false;
            }
            if (segmentStartPts === null) {
              segmentStartPts = frames[0].pts;
              segmentEndPts = segmentStartPts;
            }
            segmentEndPts += frames.duration;
            this.trigger("timingInfo", {
              start: segmentStartPts,
              end: segmentEndPts
            });
            for (i = 0; i < frames.length; i++) {
              var frame = frames[i];
              track.samples = frameUtils.generateSampleTableForFrame(frame);
              var mdat2 = mp4Generator.mdat(frameUtils.concatenateNalDataForFrame(frame));
              trackDecodeInfo.clearDtsInfo(track);
              trackDecodeInfo.collectDtsInfo(track, frame);
              track.baseMediaDecodeTime = trackDecodeInfo.calculateTrackBaseMediaDecodeTime(track, options.keepOriginalTimestamps);
              var moof2 = mp4Generator.moof(sequenceNumber, [track]);
              sequenceNumber++;
              track.initSegment = mp4Generator.initSegment([track]);
              var boxes = new Uint8Array(moof2.byteLength + mdat2.byteLength);
              boxes.set(moof2);
              boxes.set(mdat2, moof2.byteLength);
              this.trigger("data", {
                track,
                boxes,
                sequence: sequenceNumber,
                videoFrameDts: frame.dts,
                videoFramePts: frame.pts
              });
            }
            nalUnits = [];
          };
          this.resetTimingAndConfig_ = function() {
            config = void 0;
            pps = void 0;
            segmentStartPts = null;
            segmentEndPts = null;
          };
          this.partialFlush = function() {
            this.processNals_(true);
            this.trigger("partialdone", "VideoSegmentStream");
          };
          this.flush = function() {
            this.processNals_(false);
            this.resetTimingAndConfig_();
            this.trigger("done", "VideoSegmentStream");
          };
          this.endTimeline = function() {
            this.flush();
            this.trigger("endedtimeline", "VideoSegmentStream");
          };
          this.reset = function() {
            this.resetTimingAndConfig_();
            frameCache = [];
            nalUnits = [];
            ensureNextFrameIsKeyFrame = true;
            this.trigger("reset");
          };
        };
        VideoSegmentStream.prototype = new stream();
        var videoSegmentStream = VideoSegmentStream;
        var isLikelyAacData = utils.isLikelyAacData;
        var createPipeline = function createPipeline2(object) {
          object.prototype = new stream();
          object.prototype.init.call(object);
          return object;
        };
        var tsPipeline = function tsPipeline2(options) {
          var pipeline = {
            type: "ts",
            tracks: {
              audio: null,
              video: null
            },
            packet: new m2ts_1.TransportPacketStream(),
            parse: new m2ts_1.TransportParseStream(),
            elementary: new m2ts_1.ElementaryStream(),
            timestampRollover: new m2ts_1.TimestampRolloverStream(),
            adts: new codecs.Adts(),
            h264: new codecs.h264.H264Stream(),
            captionStream: new m2ts_1.CaptionStream(options),
            metadataStream: new m2ts_1.MetadataStream()
          };
          pipeline.headOfPipeline = pipeline.packet;
          pipeline.packet.pipe(pipeline.parse).pipe(pipeline.elementary).pipe(pipeline.timestampRollover);
          pipeline.timestampRollover.pipe(pipeline.h264);
          pipeline.h264.pipe(pipeline.captionStream);
          pipeline.timestampRollover.pipe(pipeline.metadataStream);
          pipeline.timestampRollover.pipe(pipeline.adts);
          pipeline.elementary.on("data", function(data) {
            if (data.type !== "metadata") {
              return;
            }
            for (var i = 0; i < data.tracks.length; i++) {
              if (!pipeline.tracks[data.tracks[i].type]) {
                pipeline.tracks[data.tracks[i].type] = data.tracks[i];
                pipeline.tracks[data.tracks[i].type].timelineStartInfo.baseMediaDecodeTime = options.baseMediaDecodeTime;
              }
            }
            if (pipeline.tracks.video && !pipeline.videoSegmentStream) {
              pipeline.videoSegmentStream = new videoSegmentStream(pipeline.tracks.video, options);
              pipeline.videoSegmentStream.on("timelineStartInfo", function(timelineStartInfo) {
                if (pipeline.tracks.audio && !options.keepOriginalTimestamps) {
                  pipeline.audioSegmentStream.setEarliestDts(timelineStartInfo.dts - options.baseMediaDecodeTime);
                }
              });
              pipeline.videoSegmentStream.on("timingInfo", pipeline.trigger.bind(pipeline, "videoTimingInfo"));
              pipeline.videoSegmentStream.on("data", function(data2) {
                pipeline.trigger("data", {
                  type: "video",
                  data: data2
                });
              });
              pipeline.videoSegmentStream.on("done", pipeline.trigger.bind(pipeline, "done"));
              pipeline.videoSegmentStream.on("partialdone", pipeline.trigger.bind(pipeline, "partialdone"));
              pipeline.videoSegmentStream.on("endedtimeline", pipeline.trigger.bind(pipeline, "endedtimeline"));
              pipeline.h264.pipe(pipeline.videoSegmentStream);
            }
            if (pipeline.tracks.audio && !pipeline.audioSegmentStream) {
              pipeline.audioSegmentStream = new audioSegmentStream(pipeline.tracks.audio, options);
              pipeline.audioSegmentStream.on("data", function(data2) {
                pipeline.trigger("data", {
                  type: "audio",
                  data: data2
                });
              });
              pipeline.audioSegmentStream.on("done", pipeline.trigger.bind(pipeline, "done"));
              pipeline.audioSegmentStream.on("partialdone", pipeline.trigger.bind(pipeline, "partialdone"));
              pipeline.audioSegmentStream.on("endedtimeline", pipeline.trigger.bind(pipeline, "endedtimeline"));
              pipeline.audioSegmentStream.on("timingInfo", pipeline.trigger.bind(pipeline, "audioTimingInfo"));
              pipeline.adts.pipe(pipeline.audioSegmentStream);
            }
            pipeline.trigger("trackinfo", {
              hasAudio: !!pipeline.tracks.audio,
              hasVideo: !!pipeline.tracks.video
            });
          });
          pipeline.captionStream.on("data", function(caption) {
            var timelineStartPts;
            if (pipeline.tracks.video) {
              timelineStartPts = pipeline.tracks.video.timelineStartInfo.pts || 0;
            } else {
              timelineStartPts = 0;
            }
            caption.startTime = clock.metadataTsToSeconds(caption.startPts, timelineStartPts, options.keepOriginalTimestamps);
            caption.endTime = clock.metadataTsToSeconds(caption.endPts, timelineStartPts, options.keepOriginalTimestamps);
            pipeline.trigger("caption", caption);
          });
          pipeline = createPipeline(pipeline);
          pipeline.metadataStream.on("data", pipeline.trigger.bind(pipeline, "id3Frame"));
          return pipeline;
        };
        var aacPipeline = function aacPipeline2(options) {
          var pipeline = {
            type: "aac",
            tracks: {
              audio: null
            },
            metadataStream: new m2ts_1.MetadataStream(),
            aacStream: new aac(),
            audioRollover: new m2ts_1.TimestampRolloverStream("audio"),
            timedMetadataRollover: new m2ts_1.TimestampRolloverStream("timed-metadata"),
            adtsStream: new adts(true)
          };
          pipeline.headOfPipeline = pipeline.aacStream;
          pipeline.aacStream.pipe(pipeline.audioRollover).pipe(pipeline.adtsStream);
          pipeline.aacStream.pipe(pipeline.timedMetadataRollover).pipe(pipeline.metadataStream);
          pipeline.metadataStream.on("timestamp", function(frame) {
            pipeline.aacStream.setTimestamp(frame.timeStamp);
          });
          pipeline.aacStream.on("data", function(data) {
            if (data.type !== "timed-metadata" && data.type !== "audio" || pipeline.audioSegmentStream) {
              return;
            }
            pipeline.tracks.audio = pipeline.tracks.audio || {
              timelineStartInfo: {
                baseMediaDecodeTime: options.baseMediaDecodeTime
              },
              codec: "adts",
              type: "audio"
            };
            pipeline.audioSegmentStream = new audioSegmentStream(pipeline.tracks.audio, options);
            pipeline.audioSegmentStream.on("data", function(data2) {
              pipeline.trigger("data", {
                type: "audio",
                data: data2
              });
            });
            pipeline.audioSegmentStream.on("partialdone", pipeline.trigger.bind(pipeline, "partialdone"));
            pipeline.audioSegmentStream.on("done", pipeline.trigger.bind(pipeline, "done"));
            pipeline.audioSegmentStream.on("endedtimeline", pipeline.trigger.bind(pipeline, "endedtimeline"));
            pipeline.audioSegmentStream.on("timingInfo", pipeline.trigger.bind(pipeline, "audioTimingInfo"));
            pipeline.adtsStream.pipe(pipeline.audioSegmentStream);
            pipeline.trigger("trackinfo", {
              hasAudio: !!pipeline.tracks.audio,
              hasVideo: !!pipeline.tracks.video
            });
          });
          pipeline = createPipeline(pipeline);
          pipeline.metadataStream.on("data", pipeline.trigger.bind(pipeline, "id3Frame"));
          return pipeline;
        };
        var setupPipelineListeners = function setupPipelineListeners2(pipeline, transmuxer3) {
          pipeline.on("data", transmuxer3.trigger.bind(transmuxer3, "data"));
          pipeline.on("done", transmuxer3.trigger.bind(transmuxer3, "done"));
          pipeline.on("partialdone", transmuxer3.trigger.bind(transmuxer3, "partialdone"));
          pipeline.on("endedtimeline", transmuxer3.trigger.bind(transmuxer3, "endedtimeline"));
          pipeline.on("audioTimingInfo", transmuxer3.trigger.bind(transmuxer3, "audioTimingInfo"));
          pipeline.on("videoTimingInfo", transmuxer3.trigger.bind(transmuxer3, "videoTimingInfo"));
          pipeline.on("trackinfo", transmuxer3.trigger.bind(transmuxer3, "trackinfo"));
          pipeline.on("id3Frame", function(event) {
            event.dispatchType = pipeline.metadataStream.dispatchType;
            event.cueTime = clock.videoTsToSeconds(event.pts);
            transmuxer3.trigger("id3Frame", event);
          });
          pipeline.on("caption", function(event) {
            transmuxer3.trigger("caption", event);
          });
        };
        var Transmuxer = function Transmuxer2(options) {
          var pipeline = null, hasFlushed = true;
          options = options || {};
          Transmuxer2.prototype.init.call(this);
          options.baseMediaDecodeTime = options.baseMediaDecodeTime || 0;
          this.push = function(bytes) {
            if (hasFlushed) {
              var isAac = isLikelyAacData(bytes);
              if (isAac && (!pipeline || pipeline.type !== "aac")) {
                pipeline = aacPipeline(options);
                setupPipelineListeners(pipeline, this);
              } else if (!isAac && (!pipeline || pipeline.type !== "ts")) {
                pipeline = tsPipeline(options);
                setupPipelineListeners(pipeline, this);
              }
              hasFlushed = false;
            }
            pipeline.headOfPipeline.push(bytes);
          };
          this.flush = function() {
            if (!pipeline) {
              return;
            }
            hasFlushed = true;
            pipeline.headOfPipeline.flush();
          };
          this.partialFlush = function() {
            if (!pipeline) {
              return;
            }
            pipeline.headOfPipeline.partialFlush();
          };
          this.endTimeline = function() {
            if (!pipeline) {
              return;
            }
            pipeline.headOfPipeline.endTimeline();
          };
          this.reset = function() {
            if (!pipeline) {
              return;
            }
            pipeline.headOfPipeline.reset();
          };
          this.setBaseMediaDecodeTime = function(baseMediaDecodeTime) {
            if (!options.keepOriginalTimestamps) {
              options.baseMediaDecodeTime = baseMediaDecodeTime;
            }
            if (!pipeline) {
              return;
            }
            if (pipeline.tracks.audio) {
              pipeline.tracks.audio.timelineStartInfo.dts = void 0;
              pipeline.tracks.audio.timelineStartInfo.pts = void 0;
              trackDecodeInfo.clearDtsInfo(pipeline.tracks.audio);
              if (pipeline.audioRollover) {
                pipeline.audioRollover.discontinuity();
              }
            }
            if (pipeline.tracks.video) {
              if (pipeline.videoSegmentStream) {
                pipeline.videoSegmentStream.gopCache_ = [];
              }
              pipeline.tracks.video.timelineStartInfo.dts = void 0;
              pipeline.tracks.video.timelineStartInfo.pts = void 0;
              trackDecodeInfo.clearDtsInfo(pipeline.tracks.video);
            }
            if (pipeline.timestampRollover) {
              pipeline.timestampRollover.discontinuity();
            }
          };
          this.setRemux = function(val) {
            options.remux = val;
            if (pipeline && pipeline.coalesceStream) {
              pipeline.coalesceStream.setRemux(val);
            }
          };
          this.setAudioAppendStart = function(audioAppendStart) {
            if (!pipeline || !pipeline.tracks.audio || !pipeline.audioSegmentStream) {
              return;
            }
            pipeline.audioSegmentStream.setAudioAppendStart(audioAppendStart);
          };
          this.alignGopsWith = function(gopsToAlignWith) {
            return;
          };
        };
        Transmuxer.prototype = new stream();
        var transmuxer2 = Transmuxer;
        var partial = {
          Transmuxer: transmuxer2
        };
        var getUint64$1 = numbers.getUint64;
        var parseSidx = function parseSidx2(data) {
          var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
            version: data[0],
            flags: new Uint8Array(data.subarray(1, 4)),
            references: [],
            referenceId: view.getUint32(4),
            timescale: view.getUint32(8)
          }, i = 12;
          if (result.version === 0) {
            result.earliestPresentationTime = view.getUint32(i);
            result.firstOffset = view.getUint32(i + 4);
            i += 8;
          } else {
            result.earliestPresentationTime = getUint64$1(data.subarray(i));
            result.firstOffset = getUint64$1(data.subarray(i + 8));
            i += 16;
          }
          i += 2;
          var referenceCount = view.getUint16(i);
          i += 2;
          for (; referenceCount > 0; i += 12, referenceCount--) {
            result.references.push({
              referenceType: (data[i] & 128) >>> 7,
              referencedSize: view.getUint32(i) & 2147483647,
              subsegmentDuration: view.getUint32(i + 4),
              startsWithSap: !!(data[i + 8] & 128),
              sapType: (data[i + 8] & 112) >>> 4,
              sapDeltaTime: view.getUint32(i + 8) & 268435455
            });
          }
          return result;
        };
        var parseSidx_1 = parseSidx;
        var getUint64 = numbers.getUint64;
        var inspectMp4, _textifyMp, parseMp4Date = function parseMp4Date2(seconds) {
          return new Date(seconds * 1e3 - 20828448e5);
        }, nalParse = function nalParse2(avcStream) {
          var avcView = new DataView(avcStream.buffer, avcStream.byteOffset, avcStream.byteLength), result = [], i, length;
          for (i = 0; i + 4 < avcStream.length; i += length) {
            length = avcView.getUint32(i);
            i += 4;
            if (length <= 0) {
              result.push("<span style='color:red;'>MALFORMED DATA</span>");
              continue;
            }
            switch (avcStream[i] & 31) {
              case 1:
                result.push("slice_layer_without_partitioning_rbsp");
                break;
              case 5:
                result.push("slice_layer_without_partitioning_rbsp_idr");
                break;
              case 6:
                result.push("sei_rbsp");
                break;
              case 7:
                result.push("seq_parameter_set_rbsp");
                break;
              case 8:
                result.push("pic_parameter_set_rbsp");
                break;
              case 9:
                result.push("access_unit_delimiter_rbsp");
                break;
              default:
                result.push("UNKNOWN NAL - " + avcStream[i] & 31);
                break;
            }
          }
          return result;
        }, parse = {
          avc1: function avc1(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength);
            return {
              dataReferenceIndex: view.getUint16(6),
              width: view.getUint16(24),
              height: view.getUint16(26),
              horizresolution: view.getUint16(28) + view.getUint16(30) / 16,
              vertresolution: view.getUint16(32) + view.getUint16(34) / 16,
              frameCount: view.getUint16(40),
              depth: view.getUint16(74),
              config: inspectMp4(data.subarray(78, data.byteLength))
            };
          },
          avcC: function avcC(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              configurationVersion: data[0],
              avcProfileIndication: data[1],
              profileCompatibility: data[2],
              avcLevelIndication: data[3],
              lengthSizeMinusOne: data[4] & 3,
              sps: [],
              pps: []
            }, numOfSequenceParameterSets = data[5] & 31, numOfPictureParameterSets, nalSize, offset, i;
            offset = 6;
            for (i = 0; i < numOfSequenceParameterSets; i++) {
              nalSize = view.getUint16(offset);
              offset += 2;
              result.sps.push(new Uint8Array(data.subarray(offset, offset + nalSize)));
              offset += nalSize;
            }
            numOfPictureParameterSets = data[offset];
            offset++;
            for (i = 0; i < numOfPictureParameterSets; i++) {
              nalSize = view.getUint16(offset);
              offset += 2;
              result.pps.push(new Uint8Array(data.subarray(offset, offset + nalSize)));
              offset += nalSize;
            }
            return result;
          },
          btrt: function btrt(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength);
            return {
              bufferSizeDB: view.getUint32(0),
              maxBitrate: view.getUint32(4),
              avgBitrate: view.getUint32(8)
            };
          },
          edts: function edts(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          elst: function elst(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4)),
              edits: []
            }, entryCount = view.getUint32(4), i;
            for (i = 8; entryCount; entryCount--) {
              if (result.version === 0) {
                result.edits.push({
                  segmentDuration: view.getUint32(i),
                  mediaTime: view.getInt32(i + 4),
                  mediaRate: view.getUint16(i + 8) + view.getUint16(i + 10) / (256 * 256)
                });
                i += 12;
              } else {
                result.edits.push({
                  segmentDuration: getUint64(data.subarray(i)),
                  mediaTime: getUint64(data.subarray(i + 8)),
                  mediaRate: view.getUint16(i + 16) + view.getUint16(i + 18) / (256 * 256)
                });
                i += 20;
              }
            }
            return result;
          },
          esds: function esds2(data) {
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              esId: data[6] << 8 | data[7],
              streamPriority: data[8] & 31,
              decoderConfig: {
                objectProfileIndication: data[11],
                streamType: data[12] >>> 2 & 63,
                bufferSize: data[13] << 16 | data[14] << 8 | data[15],
                maxBitrate: data[16] << 24 | data[17] << 16 | data[18] << 8 | data[19],
                avgBitrate: data[20] << 24 | data[21] << 16 | data[22] << 8 | data[23],
                decoderConfigDescriptor: {
                  tag: data[24],
                  length: data[25],
                  audioObjectType: data[26] >>> 3 & 31,
                  samplingFrequencyIndex: (data[26] & 7) << 1 | data[27] >>> 7 & 1,
                  channelConfiguration: data[27] >>> 3 & 15
                }
              }
            };
          },
          ftyp: function ftyp2(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              majorBrand: parseType_1(data.subarray(0, 4)),
              minorVersion: view.getUint32(4),
              compatibleBrands: []
            }, i = 8;
            while (i < data.byteLength) {
              result.compatibleBrands.push(parseType_1(data.subarray(i, i + 4)));
              i += 4;
            }
            return result;
          },
          dinf: function dinf2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          dref: function dref(data) {
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              dataReferences: inspectMp4(data.subarray(8))
            };
          },
          hdlr: function hdlr2(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4)),
              handlerType: parseType_1(data.subarray(8, 12)),
              name: ""
            }, i = 8;
            for (i = 24; i < data.byteLength; i++) {
              if (data[i] === 0) {
                i++;
                break;
              }
              result.name += String.fromCharCode(data[i]);
            }
            result.name = decodeURIComponent(escape(result.name));
            return result;
          },
          mdat: function mdat2(data) {
            return {
              byteLength: data.byteLength,
              nals: nalParse(data)
            };
          },
          mdhd: function mdhd2(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), i = 4, language, result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4)),
              language: ""
            };
            if (result.version === 1) {
              i += 4;
              result.creationTime = parseMp4Date(view.getUint32(i));
              i += 8;
              result.modificationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.timescale = view.getUint32(i);
              i += 8;
              result.duration = view.getUint32(i);
            } else {
              result.creationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.modificationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.timescale = view.getUint32(i);
              i += 4;
              result.duration = view.getUint32(i);
            }
            i += 4;
            language = view.getUint16(i);
            result.language += String.fromCharCode((language >> 10) + 96);
            result.language += String.fromCharCode(((language & 992) >> 5) + 96);
            result.language += String.fromCharCode((language & 31) + 96);
            return result;
          },
          mdia: function mdia2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          mfhd: function mfhd2(data) {
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              sequenceNumber: data[4] << 24 | data[5] << 16 | data[6] << 8 | data[7]
            };
          },
          minf: function minf2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          mp4a: function mp4a(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              dataReferenceIndex: view.getUint16(6),
              channelcount: view.getUint16(16),
              samplesize: view.getUint16(18),
              samplerate: view.getUint16(24) + view.getUint16(26) / 65536
            };
            if (data.byteLength > 28) {
              result.streamDescriptor = inspectMp4(data.subarray(28))[0];
            }
            return result;
          },
          moof: function moof2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          moov: function moov2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          mvex: function mvex2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          mvhd: function mvhd2(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), i = 4, result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4))
            };
            if (result.version === 1) {
              i += 4;
              result.creationTime = parseMp4Date(view.getUint32(i));
              i += 8;
              result.modificationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.timescale = view.getUint32(i);
              i += 8;
              result.duration = view.getUint32(i);
            } else {
              result.creationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.modificationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.timescale = view.getUint32(i);
              i += 4;
              result.duration = view.getUint32(i);
            }
            i += 4;
            result.rate = view.getUint16(i) + view.getUint16(i + 2) / 16;
            i += 4;
            result.volume = view.getUint8(i) + view.getUint8(i + 1) / 8;
            i += 2;
            i += 2;
            i += 2 * 4;
            result.matrix = new Uint32Array(data.subarray(i, i + 9 * 4));
            i += 9 * 4;
            i += 6 * 4;
            result.nextTrackId = view.getUint32(i);
            return result;
          },
          pdin: function pdin(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength);
            return {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4)),
              rate: view.getUint32(4),
              initialDelay: view.getUint32(8)
            };
          },
          sdtp: function sdtp2(data) {
            var result = {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              samples: []
            }, i;
            for (i = 4; i < data.byteLength; i++) {
              result.samples.push({
                dependsOn: (data[i] & 48) >> 4,
                isDependedOn: (data[i] & 12) >> 2,
                hasRedundancy: data[i] & 3
              });
            }
            return result;
          },
          sidx: parseSidx_1,
          smhd: function smhd(data) {
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              balance: data[4] + data[5] / 256
            };
          },
          stbl: function stbl2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          ctts: function ctts(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4)),
              compositionOffsets: []
            }, entryCount = view.getUint32(4), i;
            for (i = 8; entryCount; i += 8, entryCount--) {
              result.compositionOffsets.push({
                sampleCount: view.getUint32(i),
                sampleOffset: view[result.version === 0 ? "getUint32" : "getInt32"](i + 4)
              });
            }
            return result;
          },
          stss: function stss(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4)),
              syncSamples: []
            }, entryCount = view.getUint32(4), i;
            for (i = 8; entryCount; i += 4, entryCount--) {
              result.syncSamples.push(view.getUint32(i));
            }
            return result;
          },
          stco: function stco(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              chunkOffsets: []
            }, entryCount = view.getUint32(4), i;
            for (i = 8; entryCount; i += 4, entryCount--) {
              result.chunkOffsets.push(view.getUint32(i));
            }
            return result;
          },
          stsc: function stsc(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), entryCount = view.getUint32(4), result = {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              sampleToChunks: []
            }, i;
            for (i = 8; entryCount; i += 12, entryCount--) {
              result.sampleToChunks.push({
                firstChunk: view.getUint32(i),
                samplesPerChunk: view.getUint32(i + 4),
                sampleDescriptionIndex: view.getUint32(i + 8)
              });
            }
            return result;
          },
          stsd: function stsd2(data) {
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              sampleDescriptions: inspectMp4(data.subarray(8))
            };
          },
          stsz: function stsz(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              sampleSize: view.getUint32(4),
              entries: []
            }, i;
            for (i = 12; i < data.byteLength; i += 4) {
              result.entries.push(view.getUint32(i));
            }
            return result;
          },
          stts: function stts(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), result = {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              timeToSamples: []
            }, entryCount = view.getUint32(4), i;
            for (i = 8; entryCount; i += 8, entryCount--) {
              result.timeToSamples.push({
                sampleCount: view.getUint32(i),
                sampleDelta: view.getUint32(i + 4)
              });
            }
            return result;
          },
          styp: function styp(data) {
            return parse.ftyp(data);
          },
          tfdt: parseTfdt,
          tfhd: parseTfhd,
          tkhd: function tkhd2(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength), i = 4, result = {
              version: view.getUint8(0),
              flags: new Uint8Array(data.subarray(1, 4))
            };
            if (result.version === 1) {
              i += 4;
              result.creationTime = parseMp4Date(view.getUint32(i));
              i += 8;
              result.modificationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.trackId = view.getUint32(i);
              i += 4;
              i += 8;
              result.duration = view.getUint32(i);
            } else {
              result.creationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.modificationTime = parseMp4Date(view.getUint32(i));
              i += 4;
              result.trackId = view.getUint32(i);
              i += 4;
              i += 4;
              result.duration = view.getUint32(i);
            }
            i += 4;
            i += 2 * 4;
            result.layer = view.getUint16(i);
            i += 2;
            result.alternateGroup = view.getUint16(i);
            i += 2;
            result.volume = view.getUint8(i) + view.getUint8(i + 1) / 8;
            i += 2;
            i += 2;
            result.matrix = new Uint32Array(data.subarray(i, i + 9 * 4));
            i += 9 * 4;
            result.width = view.getUint16(i) + view.getUint16(i + 2) / 65536;
            i += 4;
            result.height = view.getUint16(i) + view.getUint16(i + 2) / 65536;
            return result;
          },
          traf: function traf2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          trak: function trak2(data) {
            return {
              boxes: inspectMp4(data)
            };
          },
          trex: function trex2(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength);
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              trackId: view.getUint32(4),
              defaultSampleDescriptionIndex: view.getUint32(8),
              defaultSampleDuration: view.getUint32(12),
              defaultSampleSize: view.getUint32(16),
              sampleDependsOn: data[20] & 3,
              sampleIsDependedOn: (data[21] & 192) >> 6,
              sampleHasRedundancy: (data[21] & 48) >> 4,
              samplePaddingValue: (data[21] & 14) >> 1,
              sampleIsDifferenceSample: !!(data[21] & 1),
              sampleDegradationPriority: view.getUint16(22)
            };
          },
          trun: parseTrun,
          "url ": function url(data) {
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4))
            };
          },
          vmhd: function vmhd(data) {
            var view = new DataView(data.buffer, data.byteOffset, data.byteLength);
            return {
              version: data[0],
              flags: new Uint8Array(data.subarray(1, 4)),
              graphicsmode: view.getUint16(4),
              opcolor: new Uint16Array([view.getUint16(6), view.getUint16(8), view.getUint16(10)])
            };
          }
        };
        inspectMp4 = function inspectMp42(data) {
          var i = 0, result = [], view, size, type2, end, box2;
          var ab = new ArrayBuffer(data.length);
          var v = new Uint8Array(ab);
          for (var z = 0; z < data.length; ++z) {
            v[z] = data[z];
          }
          view = new DataView(ab);
          while (i < data.byteLength) {
            size = view.getUint32(i);
            type2 = parseType_1(data.subarray(i + 4, i + 8));
            end = size > 1 ? i + size : data.byteLength;
            box2 = (parse[type2] || function(data2) {
              return {
                data: data2
              };
            })(data.subarray(i + 8, end));
            box2.size = size;
            box2.type = type2;
            result.push(box2);
            i = end;
          }
          return result;
        };
        _textifyMp = function textifyMp4(inspectedMp4, depth) {
          var indent;
          depth = depth || 0;
          indent = new Array(depth * 2 + 1).join(" ");
          return inspectedMp4.map(function(box2, index) {
            return indent + box2.type + "\n" + Object.keys(box2).filter(function(key) {
              return key !== "type" && key !== "boxes";
            }).map(function(key) {
              var prefix = indent + "  " + key + ": ", value = box2[key];
              if (value instanceof Uint8Array || value instanceof Uint32Array) {
                var bytes = Array.prototype.slice.call(new Uint8Array(value.buffer, value.byteOffset, value.byteLength)).map(function(byte) {
                  return " " + ("00" + byte.toString(16)).slice(-2);
                }).join("").match(/.{1,24}/g);
                if (!bytes) {
                  return prefix + "<>";
                }
                if (bytes.length === 1) {
                  return prefix + "<" + bytes.join("").slice(1) + ">";
                }
                return prefix + "<\n" + bytes.map(function(line) {
                  return indent + "  " + line;
                }).join("\n") + "\n" + indent + "  >";
              }
              return prefix + JSON.stringify(value, null, 2).split("\n").map(function(line, index2) {
                if (index2 === 0) {
                  return line;
                }
                return indent + "  " + line;
              }).join("\n");
            }).join("\n") + (box2.boxes ? "\n" + _textifyMp(box2.boxes, depth + 1) : "");
          }).join("\n");
        };
        var mp4Inspector = {
          inspect: inspectMp4,
          textify: _textifyMp,
          parseType: parseType_1,
          findBox: findBox_1,
          parseTraf: parse.traf,
          parseTfdt: parse.tfdt,
          parseHdlr: parse.hdlr,
          parseTfhd: parse.tfhd,
          parseTrun: parse.trun,
          parseSidx: parse.sidx
        };
        var tagTypes = {
          8: "audio",
          9: "video",
          18: "metadata"
        }, hex = function hex2(val) {
          return "0x" + ("00" + val.toString(16)).slice(-2).toUpperCase();
        }, hexStringList = function hexStringList2(data) {
          var arr = [], i;
          while (data.byteLength > 0) {
            i = 0;
            arr.push(hex(data[i++]));
            data = data.subarray(i);
          }
          return arr.join(" ");
        }, parseAVCTag = function parseAVCTag2(tag, obj) {
          var avcPacketTypes = ["AVC Sequence Header", "AVC NALU", "AVC End-of-Sequence"], compositionTime = tag[1] & parseInt("01111111", 2) << 16 | tag[2] << 8 | tag[3];
          obj = obj || {};
          obj.avcPacketType = avcPacketTypes[tag[0]];
          obj.CompositionTime = tag[1] & parseInt("10000000", 2) ? -compositionTime : compositionTime;
          if (tag[0] === 1) {
            obj.nalUnitTypeRaw = hexStringList(tag.subarray(4, 100));
          } else {
            obj.data = hexStringList(tag.subarray(4));
          }
          return obj;
        }, parseVideoTag = function parseVideoTag2(tag, obj) {
          var frameTypes = ["Unknown", "Keyframe (for AVC, a seekable frame)", "Inter frame (for AVC, a nonseekable frame)", "Disposable inter frame (H.263 only)", "Generated keyframe (reserved for server use only)", "Video info/command frame"], codecID = tag[0] & parseInt("00001111", 2);
          obj = obj || {};
          obj.frameType = frameTypes[(tag[0] & parseInt("11110000", 2)) >>> 4];
          obj.codecID = codecID;
          if (codecID === 7) {
            return parseAVCTag(tag.subarray(1), obj);
          }
          return obj;
        }, parseAACTag = function parseAACTag2(tag, obj) {
          var packetTypes = ["AAC Sequence Header", "AAC Raw"];
          obj = obj || {};
          obj.aacPacketType = packetTypes[tag[0]];
          obj.data = hexStringList(tag.subarray(1));
          return obj;
        }, parseAudioTag = function parseAudioTag2(tag, obj) {
          var formatTable = ["Linear PCM, platform endian", "ADPCM", "MP3", "Linear PCM, little endian", "Nellymoser 16-kHz mono", "Nellymoser 8-kHz mono", "Nellymoser", "G.711 A-law logarithmic PCM", "G.711 mu-law logarithmic PCM", "reserved", "AAC", "Speex", "MP3 8-Khz", "Device-specific sound"], samplingRateTable = ["5.5-kHz", "11-kHz", "22-kHz", "44-kHz"], soundFormat = (tag[0] & parseInt("11110000", 2)) >>> 4;
          obj = obj || {};
          obj.soundFormat = formatTable[soundFormat];
          obj.soundRate = samplingRateTable[(tag[0] & parseInt("00001100", 2)) >>> 2];
          obj.soundSize = (tag[0] & parseInt("00000010", 2)) >>> 1 ? "16-bit" : "8-bit";
          obj.soundType = tag[0] & parseInt("00000001", 2) ? "Stereo" : "Mono";
          if (soundFormat === 10) {
            return parseAACTag(tag.subarray(1), obj);
          }
          return obj;
        }, parseGenericTag = function parseGenericTag2(tag) {
          return {
            tagType: tagTypes[tag[0]],
            dataSize: tag[1] << 16 | tag[2] << 8 | tag[3],
            timestamp: tag[7] << 24 | tag[4] << 16 | tag[5] << 8 | tag[6],
            streamID: tag[8] << 16 | tag[9] << 8 | tag[10]
          };
        }, inspectFlvTag = function inspectFlvTag2(tag) {
          var header = parseGenericTag(tag);
          switch (tag[0]) {
            case 8:
              parseAudioTag(tag.subarray(11), header);
              break;
            case 9:
              parseVideoTag(tag.subarray(11), header);
              break;
          }
          return header;
        }, inspectFlv = function inspectFlv2(bytes) {
          var i = 9, dataSize, parsedResults = [], tag;
          i += 4;
          while (i < bytes.byteLength) {
            dataSize = bytes[i + 1] << 16;
            dataSize |= bytes[i + 2] << 8;
            dataSize |= bytes[i + 3];
            dataSize += 11;
            tag = bytes.subarray(i, i + dataSize);
            parsedResults.push(inspectFlvTag(tag));
            i += dataSize + 4;
          }
          return parsedResults;
        }, textifyFlv = function textifyFlv2(flvTagArray) {
          return JSON.stringify(flvTagArray, null, 2);
        };
        var flvInspector = {
          inspectTag: inspectFlvTag,
          inspect: inspectFlv,
          textify: textifyFlv
        };
        var parsePid = function parsePid2(packet) {
          var pid = packet[1] & 31;
          pid <<= 8;
          pid |= packet[2];
          return pid;
        };
        var parsePayloadUnitStartIndicator = function parsePayloadUnitStartIndicator2(packet) {
          return !!(packet[1] & 64);
        };
        var parseAdaptionField = function parseAdaptionField2(packet) {
          var offset = 0;
          if ((packet[3] & 48) >>> 4 > 1) {
            offset += packet[4] + 1;
          }
          return offset;
        };
        var parseType = function parseType2(packet, pmtPid) {
          var pid = parsePid(packet);
          if (pid === 0) {
            return "pat";
          } else if (pid === pmtPid) {
            return "pmt";
          } else if (pmtPid) {
            return "pes";
          }
          return null;
        };
        var parsePat = function parsePat2(packet) {
          var pusi = parsePayloadUnitStartIndicator(packet);
          var offset = 4 + parseAdaptionField(packet);
          if (pusi) {
            offset += packet[offset] + 1;
          }
          return (packet[offset + 10] & 31) << 8 | packet[offset + 11];
        };
        var parsePmt = function parsePmt2(packet) {
          var programMapTable = {};
          var pusi = parsePayloadUnitStartIndicator(packet);
          var payloadOffset = 4 + parseAdaptionField(packet);
          if (pusi) {
            payloadOffset += packet[payloadOffset] + 1;
          }
          if (!(packet[payloadOffset + 5] & 1)) {
            return;
          }
          var sectionLength, tableEnd, programInfoLength;
          sectionLength = (packet[payloadOffset + 1] & 15) << 8 | packet[payloadOffset + 2];
          tableEnd = 3 + sectionLength - 4;
          programInfoLength = (packet[payloadOffset + 10] & 15) << 8 | packet[payloadOffset + 11];
          var offset = 12 + programInfoLength;
          while (offset < tableEnd) {
            var i = payloadOffset + offset;
            programMapTable[(packet[i + 1] & 31) << 8 | packet[i + 2]] = packet[i];
            offset += ((packet[i + 3] & 15) << 8 | packet[i + 4]) + 5;
          }
          return programMapTable;
        };
        var parsePesType = function parsePesType2(packet, programMapTable) {
          var pid = parsePid(packet);
          var type2 = programMapTable[pid];
          switch (type2) {
            case streamTypes.H264_STREAM_TYPE:
              return "video";
            case streamTypes.ADTS_STREAM_TYPE:
              return "audio";
            case streamTypes.METADATA_STREAM_TYPE:
              return "timed-metadata";
            default:
              return null;
          }
        };
        var parsePesTime = function parsePesTime2(packet) {
          var pusi = parsePayloadUnitStartIndicator(packet);
          if (!pusi) {
            return null;
          }
          var offset = 4 + parseAdaptionField(packet);
          if (offset >= packet.byteLength) {
            return null;
          }
          var pes = null;
          var ptsDtsFlags;
          ptsDtsFlags = packet[offset + 7];
          if (ptsDtsFlags & 192) {
            pes = {};
            pes.pts = (packet[offset + 9] & 14) << 27 | (packet[offset + 10] & 255) << 20 | (packet[offset + 11] & 254) << 12 | (packet[offset + 12] & 255) << 5 | (packet[offset + 13] & 254) >>> 3;
            pes.pts *= 4;
            pes.pts += (packet[offset + 13] & 6) >>> 1;
            pes.dts = pes.pts;
            if (ptsDtsFlags & 64) {
              pes.dts = (packet[offset + 14] & 14) << 27 | (packet[offset + 15] & 255) << 20 | (packet[offset + 16] & 254) << 12 | (packet[offset + 17] & 255) << 5 | (packet[offset + 18] & 254) >>> 3;
              pes.dts *= 4;
              pes.dts += (packet[offset + 18] & 6) >>> 1;
            }
          }
          return pes;
        };
        var parseNalUnitType = function parseNalUnitType2(type2) {
          switch (type2) {
            case 5:
              return "slice_layer_without_partitioning_rbsp_idr";
            case 6:
              return "sei_rbsp";
            case 7:
              return "seq_parameter_set_rbsp";
            case 8:
              return "pic_parameter_set_rbsp";
            case 9:
              return "access_unit_delimiter_rbsp";
            default:
              return null;
          }
        };
        var videoPacketContainsKeyFrame = function videoPacketContainsKeyFrame2(packet) {
          var offset = 4 + parseAdaptionField(packet);
          var frameBuffer = packet.subarray(offset);
          var frameI = 0;
          var frameSyncPoint = 0;
          var foundKeyFrame = false;
          var nalType;
          for (; frameSyncPoint < frameBuffer.byteLength - 3; frameSyncPoint++) {
            if (frameBuffer[frameSyncPoint + 2] === 1) {
              frameI = frameSyncPoint + 5;
              break;
            }
          }
          while (frameI < frameBuffer.byteLength) {
            switch (frameBuffer[frameI]) {
              case 0:
                if (frameBuffer[frameI - 1] !== 0) {
                  frameI += 2;
                  break;
                } else if (frameBuffer[frameI - 2] !== 0) {
                  frameI++;
                  break;
                }
                if (frameSyncPoint + 3 !== frameI - 2) {
                  nalType = parseNalUnitType(frameBuffer[frameSyncPoint + 3] & 31);
                  if (nalType === "slice_layer_without_partitioning_rbsp_idr") {
                    foundKeyFrame = true;
                  }
                }
                do {
                  frameI++;
                } while (frameBuffer[frameI] !== 1 && frameI < frameBuffer.length);
                frameSyncPoint = frameI - 2;
                frameI += 3;
                break;
              case 1:
                if (frameBuffer[frameI - 1] !== 0 || frameBuffer[frameI - 2] !== 0) {
                  frameI += 3;
                  break;
                }
                nalType = parseNalUnitType(frameBuffer[frameSyncPoint + 3] & 31);
                if (nalType === "slice_layer_without_partitioning_rbsp_idr") {
                  foundKeyFrame = true;
                }
                frameSyncPoint = frameI - 2;
                frameI += 3;
                break;
              default:
                frameI += 3;
                break;
            }
          }
          frameBuffer = frameBuffer.subarray(frameSyncPoint);
          frameI -= frameSyncPoint;
          frameSyncPoint = 0;
          if (frameBuffer && frameBuffer.byteLength > 3) {
            nalType = parseNalUnitType(frameBuffer[frameSyncPoint + 3] & 31);
            if (nalType === "slice_layer_without_partitioning_rbsp_idr") {
              foundKeyFrame = true;
            }
          }
          return foundKeyFrame;
        };
        var probe$1 = {
          parseType,
          parsePat,
          parsePmt,
          parsePayloadUnitStartIndicator,
          parsePesType,
          parsePesTime,
          videoPacketContainsKeyFrame
        };
        var handleRollover = timestampRolloverStream.handleRollover;
        var probe = {};
        probe.ts = probe$1;
        probe.aac = utils;
        var ONE_SECOND_IN_TS = clock.ONE_SECOND_IN_TS;
        var MP2T_PACKET_LENGTH = 188, SYNC_BYTE = 71;
        var parsePsi_ = function parsePsi_2(bytes, pmt) {
          var startIndex = 0, endIndex = MP2T_PACKET_LENGTH, packet, type2;
          while (endIndex < bytes.byteLength) {
            if (bytes[startIndex] === SYNC_BYTE && bytes[endIndex] === SYNC_BYTE) {
              packet = bytes.subarray(startIndex, endIndex);
              type2 = probe.ts.parseType(packet, pmt.pid);
              switch (type2) {
                case "pat":
                  pmt.pid = probe.ts.parsePat(packet);
                  break;
                case "pmt":
                  var table = probe.ts.parsePmt(packet);
                  pmt.table = pmt.table || {};
                  Object.keys(table).forEach(function(key) {
                    pmt.table[key] = table[key];
                  });
                  break;
              }
              startIndex += MP2T_PACKET_LENGTH;
              endIndex += MP2T_PACKET_LENGTH;
              continue;
            }
            startIndex++;
            endIndex++;
          }
        };
        var parseAudioPes_ = function parseAudioPes_2(bytes, pmt, result) {
          var startIndex = 0, endIndex = MP2T_PACKET_LENGTH, packet, type2, pesType, pusi, parsed;
          var endLoop = false;
          while (endIndex <= bytes.byteLength) {
            if (bytes[startIndex] === SYNC_BYTE && (bytes[endIndex] === SYNC_BYTE || endIndex === bytes.byteLength)) {
              packet = bytes.subarray(startIndex, endIndex);
              type2 = probe.ts.parseType(packet, pmt.pid);
              switch (type2) {
                case "pes":
                  pesType = probe.ts.parsePesType(packet, pmt.table);
                  pusi = probe.ts.parsePayloadUnitStartIndicator(packet);
                  if (pesType === "audio" && pusi) {
                    parsed = probe.ts.parsePesTime(packet);
                    if (parsed) {
                      parsed.type = "audio";
                      result.audio.push(parsed);
                      endLoop = true;
                    }
                  }
                  break;
              }
              if (endLoop) {
                break;
              }
              startIndex += MP2T_PACKET_LENGTH;
              endIndex += MP2T_PACKET_LENGTH;
              continue;
            }
            startIndex++;
            endIndex++;
          }
          endIndex = bytes.byteLength;
          startIndex = endIndex - MP2T_PACKET_LENGTH;
          endLoop = false;
          while (startIndex >= 0) {
            if (bytes[startIndex] === SYNC_BYTE && (bytes[endIndex] === SYNC_BYTE || endIndex === bytes.byteLength)) {
              packet = bytes.subarray(startIndex, endIndex);
              type2 = probe.ts.parseType(packet, pmt.pid);
              switch (type2) {
                case "pes":
                  pesType = probe.ts.parsePesType(packet, pmt.table);
                  pusi = probe.ts.parsePayloadUnitStartIndicator(packet);
                  if (pesType === "audio" && pusi) {
                    parsed = probe.ts.parsePesTime(packet);
                    if (parsed) {
                      parsed.type = "audio";
                      result.audio.push(parsed);
                      endLoop = true;
                    }
                  }
                  break;
              }
              if (endLoop) {
                break;
              }
              startIndex -= MP2T_PACKET_LENGTH;
              endIndex -= MP2T_PACKET_LENGTH;
              continue;
            }
            startIndex--;
            endIndex--;
          }
        };
        var parseVideoPes_ = function parseVideoPes_2(bytes, pmt, result) {
          var startIndex = 0, endIndex = MP2T_PACKET_LENGTH, packet, type2, pesType, pusi, parsed, frame, i, pes;
          var endLoop = false;
          var currentFrame = {
            data: [],
            size: 0
          };
          while (endIndex < bytes.byteLength) {
            if (bytes[startIndex] === SYNC_BYTE && bytes[endIndex] === SYNC_BYTE) {
              packet = bytes.subarray(startIndex, endIndex);
              type2 = probe.ts.parseType(packet, pmt.pid);
              switch (type2) {
                case "pes":
                  pesType = probe.ts.parsePesType(packet, pmt.table);
                  pusi = probe.ts.parsePayloadUnitStartIndicator(packet);
                  if (pesType === "video") {
                    if (pusi && !endLoop) {
                      parsed = probe.ts.parsePesTime(packet);
                      if (parsed) {
                        parsed.type = "video";
                        result.video.push(parsed);
                        endLoop = true;
                      }
                    }
                    if (!result.firstKeyFrame) {
                      if (pusi) {
                        if (currentFrame.size !== 0) {
                          frame = new Uint8Array(currentFrame.size);
                          i = 0;
                          while (currentFrame.data.length) {
                            pes = currentFrame.data.shift();
                            frame.set(pes, i);
                            i += pes.byteLength;
                          }
                          if (probe.ts.videoPacketContainsKeyFrame(frame)) {
                            var firstKeyFrame = probe.ts.parsePesTime(frame);
                            if (firstKeyFrame) {
                              result.firstKeyFrame = firstKeyFrame;
                              result.firstKeyFrame.type = "video";
                            } else {
                              console.warn("Failed to extract PTS/DTS from PES at first keyframe. This could be an unusual TS segment, or else mux.js did not parse your TS segment correctly. If you know your TS segments do contain PTS/DTS on keyframes please file a bug report! You can try ffprobe to double check for yourself.");
                            }
                          }
                          currentFrame.size = 0;
                        }
                      }
                      currentFrame.data.push(packet);
                      currentFrame.size += packet.byteLength;
                    }
                  }
                  break;
              }
              if (endLoop && result.firstKeyFrame) {
                break;
              }
              startIndex += MP2T_PACKET_LENGTH;
              endIndex += MP2T_PACKET_LENGTH;
              continue;
            }
            startIndex++;
            endIndex++;
          }
          endIndex = bytes.byteLength;
          startIndex = endIndex - MP2T_PACKET_LENGTH;
          endLoop = false;
          while (startIndex >= 0) {
            if (bytes[startIndex] === SYNC_BYTE && bytes[endIndex] === SYNC_BYTE) {
              packet = bytes.subarray(startIndex, endIndex);
              type2 = probe.ts.parseType(packet, pmt.pid);
              switch (type2) {
                case "pes":
                  pesType = probe.ts.parsePesType(packet, pmt.table);
                  pusi = probe.ts.parsePayloadUnitStartIndicator(packet);
                  if (pesType === "video" && pusi) {
                    parsed = probe.ts.parsePesTime(packet);
                    if (parsed) {
                      parsed.type = "video";
                      result.video.push(parsed);
                      endLoop = true;
                    }
                  }
                  break;
              }
              if (endLoop) {
                break;
              }
              startIndex -= MP2T_PACKET_LENGTH;
              endIndex -= MP2T_PACKET_LENGTH;
              continue;
            }
            startIndex--;
            endIndex--;
          }
        };
        var adjustTimestamp_ = function adjustTimestamp_2(segmentInfo, baseTimestamp) {
          if (segmentInfo.audio && segmentInfo.audio.length) {
            var audioBaseTimestamp = baseTimestamp;
            if (typeof audioBaseTimestamp === "undefined" || isNaN(audioBaseTimestamp)) {
              audioBaseTimestamp = segmentInfo.audio[0].dts;
            }
            segmentInfo.audio.forEach(function(info) {
              info.dts = handleRollover(info.dts, audioBaseTimestamp);
              info.pts = handleRollover(info.pts, audioBaseTimestamp);
              info.dtsTime = info.dts / ONE_SECOND_IN_TS;
              info.ptsTime = info.pts / ONE_SECOND_IN_TS;
            });
          }
          if (segmentInfo.video && segmentInfo.video.length) {
            var videoBaseTimestamp = baseTimestamp;
            if (typeof videoBaseTimestamp === "undefined" || isNaN(videoBaseTimestamp)) {
              videoBaseTimestamp = segmentInfo.video[0].dts;
            }
            segmentInfo.video.forEach(function(info) {
              info.dts = handleRollover(info.dts, videoBaseTimestamp);
              info.pts = handleRollover(info.pts, videoBaseTimestamp);
              info.dtsTime = info.dts / ONE_SECOND_IN_TS;
              info.ptsTime = info.pts / ONE_SECOND_IN_TS;
            });
            if (segmentInfo.firstKeyFrame) {
              var frame = segmentInfo.firstKeyFrame;
              frame.dts = handleRollover(frame.dts, videoBaseTimestamp);
              frame.pts = handleRollover(frame.pts, videoBaseTimestamp);
              frame.dtsTime = frame.dts / ONE_SECOND_IN_TS;
              frame.ptsTime = frame.pts / ONE_SECOND_IN_TS;
            }
          }
        };
        var inspectAac_ = function inspectAac_2(bytes) {
          var endLoop = false, audioCount = 0, sampleRate = null, timestamp = null, frameSize = 0, byteIndex = 0, packet;
          while (bytes.length - byteIndex >= 3) {
            var type2 = probe.aac.parseType(bytes, byteIndex);
            switch (type2) {
              case "timed-metadata":
                if (bytes.length - byteIndex < 10) {
                  endLoop = true;
                  break;
                }
                frameSize = probe.aac.parseId3TagSize(bytes, byteIndex);
                if (frameSize > bytes.length) {
                  endLoop = true;
                  break;
                }
                if (timestamp === null) {
                  packet = bytes.subarray(byteIndex, byteIndex + frameSize);
                  timestamp = probe.aac.parseAacTimestamp(packet);
                }
                byteIndex += frameSize;
                break;
              case "audio":
                if (bytes.length - byteIndex < 7) {
                  endLoop = true;
                  break;
                }
                frameSize = probe.aac.parseAdtsSize(bytes, byteIndex);
                if (frameSize > bytes.length) {
                  endLoop = true;
                  break;
                }
                if (sampleRate === null) {
                  packet = bytes.subarray(byteIndex, byteIndex + frameSize);
                  sampleRate = probe.aac.parseSampleRate(packet);
                }
                audioCount++;
                byteIndex += frameSize;
                break;
              default:
                byteIndex++;
                break;
            }
            if (endLoop) {
              return null;
            }
          }
          if (sampleRate === null || timestamp === null) {
            return null;
          }
          var audioTimescale = ONE_SECOND_IN_TS / sampleRate;
          var result = {
            audio: [{
              type: "audio",
              dts: timestamp,
              pts: timestamp
            }, {
              type: "audio",
              dts: timestamp + audioCount * 1024 * audioTimescale,
              pts: timestamp + audioCount * 1024 * audioTimescale
            }]
          };
          return result;
        };
        var inspectTs_ = function inspectTs_2(bytes) {
          var pmt = {
            pid: null,
            table: null
          };
          var result = {};
          parsePsi_(bytes, pmt);
          for (var pid in pmt.table) {
            if (pmt.table.hasOwnProperty(pid)) {
              var type2 = pmt.table[pid];
              switch (type2) {
                case streamTypes.H264_STREAM_TYPE:
                  result.video = [];
                  parseVideoPes_(bytes, pmt, result);
                  if (result.video.length === 0) {
                    delete result.video;
                  }
                  break;
                case streamTypes.ADTS_STREAM_TYPE:
                  result.audio = [];
                  parseAudioPes_(bytes, pmt, result);
                  if (result.audio.length === 0) {
                    delete result.audio;
                  }
                  break;
              }
            }
          }
          return result;
        };
        var inspect = function inspect2(bytes, baseTimestamp) {
          var isAacData = probe.aac.isLikelyAacData(bytes);
          var result;
          if (isAacData) {
            result = inspectAac_(bytes);
          } else {
            result = inspectTs_(bytes);
          }
          if (!result || !result.audio && !result.video) {
            return null;
          }
          adjustTimestamp_(result, baseTimestamp);
          return result;
        };
        var tsInspector = {
          inspect,
          parseAudioPes_
        };
        var muxjs2 = {
          codecs,
          mp4,
          flv,
          mp2t: m2ts,
          partial
        };
        muxjs2.mp4.tools = mp4Inspector;
        muxjs2.flv.tools = flvInspector;
        muxjs2.mp2t.tools = tsInspector;
        var lib = muxjs2;
        return lib;
      });
    }
  });

  // src/wasm/wasp_hls.js
  var import_meta = {};
  var wasm;
  var cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
  cachedTextDecoder.decode();
  var cachedUint8Memory0 = new Uint8Array();
  function getUint8Memory0() {
    if (cachedUint8Memory0.byteLength === 0) {
      cachedUint8Memory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8Memory0;
  }
  function getStringFromWasm0(ptr, len) {
    return cachedTextDecoder.decode(getUint8Memory0().subarray(ptr, ptr + len));
  }
  var WASM_VECTOR_LEN = 0;
  var cachedTextEncoder = new TextEncoder("utf-8");
  var encodeString = typeof cachedTextEncoder.encodeInto === "function" ? function(arg, view) {
    return cachedTextEncoder.encodeInto(arg, view);
  } : function(arg, view) {
    const buf = cachedTextEncoder.encode(arg);
    view.set(buf);
    return {
      read: arg.length,
      written: buf.length
    };
  };
  function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === void 0) {
      const buf = cachedTextEncoder.encode(arg);
      const ptr2 = malloc(buf.length);
      getUint8Memory0().subarray(ptr2, ptr2 + buf.length).set(buf);
      WASM_VECTOR_LEN = buf.length;
      return ptr2;
    }
    let len = arg.length;
    let ptr = malloc(len);
    const mem = getUint8Memory0();
    let offset = 0;
    for (; offset < len; offset++) {
      const code = arg.charCodeAt(offset);
      if (code > 127)
        break;
      mem[ptr + offset] = code;
    }
    if (offset !== len) {
      if (offset !== 0) {
        arg = arg.slice(offset);
      }
      ptr = realloc(ptr, len, len = offset + arg.length * 3);
      const view = getUint8Memory0().subarray(ptr + offset, ptr + len);
      const ret = encodeString(arg, view);
      offset += ret.written;
    }
    WASM_VECTOR_LEN = offset;
    return ptr;
  }
  var cachedInt32Memory0 = new Int32Array();
  function getInt32Memory0() {
    if (cachedInt32Memory0.byteLength === 0) {
      cachedInt32Memory0 = new Int32Array(wasm.memory.buffer);
    }
    return cachedInt32Memory0;
  }
  function getArrayU8FromWasm0(ptr, len) {
    return getUint8Memory0().subarray(ptr / 1, ptr / 1 + len);
  }
  function _assertClass(instance, klass) {
    if (!(instance instanceof klass)) {
      throw new Error(`expected instance of ${klass.name}`);
    }
    return instance.ptr;
  }
  var cachedFloat64Memory0 = new Float64Array();
  function getFloat64Memory0() {
    if (cachedFloat64Memory0.byteLength === 0) {
      cachedFloat64Memory0 = new Float64Array(wasm.memory.buffer);
    }
    return cachedFloat64Memory0;
  }
  function passArrayF64ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 8);
    getFloat64Memory0().set(arg, ptr / 8);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
  }
  function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1);
    getUint8Memory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
  }
  function isLikeNone(x) {
    return x === void 0 || x === null;
  }
  function notDefined(what) {
    return () => {
      throw new Error(`${what} is not defined`);
    };
  }
  var MediaSourceReadyState = Object.freeze({
    Closed: 0,
    "0": "Closed",
    Ended: 1,
    "1": "Ended",
    Open: 2,
    "2": "Open"
  });
  var PlaybackTickReason = Object.freeze({ Init: 0, "0": "Init", Seeking: 1, "1": "Seeking", Seeked: 2, "2": "Seeked", RegularInterval: 3, "3": "RegularInterval", LoadedData: 4, "4": "LoadedData", LoadedMetadata: 5, "5": "LoadedMetadata", CanPlay: 6, "6": "CanPlay", CanPlayThrough: 7, "7": "CanPlayThrough", Ended: 8, "8": "Ended", Pause: 9, "9": "Pause", Play: 10, "10": "Play", RateChange: 11, "11": "RateChange", Stalled: 12, "12": "Stalled" });
  var RemoveMediaSourceErrorCode = Object.freeze({
    NoMediaSourceAttached: 0,
    "0": "NoMediaSourceAttached",
    UnknownError: 1,
    "1": "UnknownError"
  });
  var MediaSourceDurationUpdateErrorCode = Object.freeze({
    NoMediaSourceAttached: 0,
    "0": "NoMediaSourceAttached",
    UnknownError: 1,
    "1": "UnknownError"
  });
  var AttachMediaSourceErrorCode = Object.freeze({
    UnknownError: 0,
    "0": "UnknownError"
  });
  var RemoveBufferErrorCode = Object.freeze({
    SourceBufferNotFound: 0,
    "0": "SourceBufferNotFound",
    UnknownError: 1,
    "1": "UnknownError"
  });
  var EndOfStreamErrorCode = Object.freeze({
    UnknownError: 0,
    "0": "UnknownError"
  });
  var AddSourceBufferErrorCode = Object.freeze({
    NoMediaSourceAttached: 0,
    "0": "NoMediaSourceAttached",
    MediaSourceIsClosed: 1,
    "1": "MediaSourceIsClosed",
    QuotaExceededError: 2,
    "2": "QuotaExceededError",
    TypeNotSupportedError: 3,
    "3": "TypeNotSupportedError",
    EmptyMimeType: 4,
    "4": "EmptyMimeType",
    UnknownError: 5,
    "5": "UnknownError"
  });
  var AppendBufferErrorCode = Object.freeze({
    NoResource: 0,
    "0": "NoResource",
    NoSourceBuffer: 1,
    "1": "NoSourceBuffer",
    TransmuxerError: 2,
    "2": "TransmuxerError",
    UnknownError: 3,
    "3": "UnknownError"
  });
  var PlaybackObservationReason = Object.freeze({
    Init: 0,
    "0": "Init",
    Seeked: 1,
    "1": "Seeked",
    Seeking: 2,
    "2": "Seeking",
    Ended: 3,
    "3": "Ended",
    ReadyStateChanged: 4,
    "4": "ReadyStateChanged",
    RegularInterval: 5,
    "5": "RegularInterval",
    Error: 6,
    "6": "Error"
  });
  var TimerReason = Object.freeze({
    MediaPlaylistRefresh: 0,
    "0": "MediaPlaylistRefresh"
  });
  var LogLevel = Object.freeze({
    Error: 0,
    "0": "Error",
    Warn: 1,
    "1": "Warn",
    Info: 2,
    "2": "Info",
    Debug: 3,
    "3": "Debug"
  });
  var MediaType = Object.freeze({ Audio: 0, "0": "Audio", Video: 1, "1": "Video" });
  var AppendBufferResult = class {
    static __wrap(ptr) {
      const obj = Object.create(AppendBufferResult.prototype);
      obj.ptr = ptr;
      return obj;
    }
    __destroy_into_raw() {
      const ptr = this.ptr;
      this.ptr = 0;
      return ptr;
    }
    free() {
      const ptr = this.__destroy_into_raw();
      wasm.__wbg_appendbufferresult_free(ptr);
    }
    static success(start, duration) {
      const ret = wasm.appendbufferresult_success(!isLikeNone(start), isLikeNone(start) ? 0 : start, !isLikeNone(duration), isLikeNone(duration) ? 0 : duration);
      return AppendBufferResult.__wrap(ret);
    }
    static error(err, desc) {
      var ptr0 = isLikeNone(desc) ? 0 : passStringToWasm0(desc, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      var len0 = WASM_VECTOR_LEN;
      const ret = wasm.appendbufferresult_error(err, ptr0, len0);
      return AppendBufferResult.__wrap(ret);
    }
  };
  var Dispatcher = class {
    static __wrap(ptr) {
      const obj = Object.create(Dispatcher.prototype);
      obj.ptr = ptr;
      return obj;
    }
    __destroy_into_raw() {
      const ptr = this.ptr;
      this.ptr = 0;
      return ptr;
    }
    free() {
      const ptr = this.__destroy_into_raw();
      wasm.__wbg_dispatcher_free(ptr);
    }
    constructor() {
      const ret = wasm.dispatcher_new();
      return Dispatcher.__wrap(ret);
    }
    load_content(content_url) {
      const ptr0 = passStringToWasm0(content_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      wasm.dispatcher_load_content(this.ptr, ptr0, len0);
    }
    get_available_audio_tracks() {
      try {
        const retptr = wasm.__wbindgen_add_to_stack_pointer(-16);
        wasm.dispatcher_get_available_audio_tracks(retptr, this.ptr);
        var r0 = getInt32Memory0()[retptr / 4 + 0];
        var r1 = getInt32Memory0()[retptr / 4 + 1];
        var v0 = getArrayU8FromWasm0(r0, r1).slice();
        wasm.__wbindgen_free(r0, r1 * 1);
        return v0;
      } finally {
        wasm.__wbindgen_add_to_stack_pointer(16);
      }
    }
    stop() {
      wasm.dispatcher_stop(this.ptr);
    }
    static log(level, msg) {
      const ptr0 = passStringToWasm0(msg, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      wasm.dispatcher_log(level, ptr0, len0);
    }
    on_request_finished(request_id, resource_id, resource_size, final_url, duration_ms) {
      const ptr0 = passStringToWasm0(final_url, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len0 = WASM_VECTOR_LEN;
      wasm.dispatcher_on_request_finished(this.ptr, request_id, resource_id, resource_size, ptr0, len0, duration_ms);
    }
    on_request_failed(request_id) {
      wasm.dispatcher_on_request_failed(this.ptr, request_id);
    }
    on_media_source_state_change(state) {
      wasm.dispatcher_on_media_source_state_change(this.ptr, state);
    }
    on_source_buffer_update(source_buffer_id) {
      wasm.dispatcher_on_source_buffer_update(this.ptr, source_buffer_id);
    }
    on_source_buffer_error(source_buffer_id) {
      wasm.dispatcher_on_source_buffer_error(this.ptr, source_buffer_id);
    }
    on_playback_tick(observation) {
      _assertClass(observation, MediaObservation);
      var ptr0 = observation.ptr;
      observation.ptr = 0;
      wasm.dispatcher_on_playback_tick(this.ptr, ptr0);
    }
    on_timer_ended(id, reason) {
      wasm.dispatcher_on_timer_ended(this.ptr, id, reason);
    }
  };
  var MediaObservation = class {
    static __wrap(ptr) {
      const obj = Object.create(MediaObservation.prototype);
      obj.ptr = ptr;
      return obj;
    }
    __destroy_into_raw() {
      const ptr = this.ptr;
      this.ptr = 0;
      return ptr;
    }
    free() {
      const ptr = this.__destroy_into_raw();
      wasm.__wbg_mediaobservation_free(ptr);
    }
    constructor(reason, current_time, ready_state, buffered, paused, seeking) {
      const ptr0 = passArrayF64ToWasm0(buffered, wasm.__wbindgen_malloc);
      const len0 = WASM_VECTOR_LEN;
      const ret = wasm.mediaobservation_new(reason, current_time, ready_state, ptr0, len0, paused, seeking);
      return MediaObservation.__wrap(ret);
    }
  };
  async function load(module, imports) {
    if (typeof Response === "function" && module instanceof Response) {
      if (typeof WebAssembly.instantiateStreaming === "function") {
        try {
          return await WebAssembly.instantiateStreaming(module, imports);
        } catch (e) {
          if (module.headers.get("Content-Type") != "application/wasm") {
            console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
          } else {
            throw e;
          }
        }
      }
      const bytes = await module.arrayBuffer();
      return await WebAssembly.instantiate(bytes, imports);
    } else {
      const instance = await WebAssembly.instantiate(module, imports);
      if (instance instanceof WebAssembly.Instance) {
        return { instance, module };
      } else {
        return instance;
      }
    }
  }
  function getImports() {
    const imports = {};
    imports.wbg = {};
    imports.wbg.__wbg_jsLog_da12411f674ee50f = function(arg0, arg1, arg2) {
      jsLog(arg0 >>> 0, getStringFromWasm0(arg1, arg2));
    };
    imports.wbg.__wbg_jsTimer_13ace9ec5780a1dd = function(arg0, arg1) {
      const ret = jsTimer(arg0, arg1 >>> 0);
      return ret;
    };
    imports.wbg.__wbg_jsGetResourceData_1a5580de201a7896 = function(arg0, arg1) {
      const ret = jsGetResourceData(arg1);
      var ptr0 = isLikeNone(ret) ? 0 : passArray8ToWasm0(ret, wasm.__wbindgen_malloc);
      var len0 = WASM_VECTOR_LEN;
      getInt32Memory0()[arg0 / 4 + 1] = len0;
      getInt32Memory0()[arg0 / 4 + 0] = ptr0;
    };
    imports.wbg.__wbg_jsFetch_1c0faf7eaeeec79c = function(arg0, arg1) {
      const ret = jsFetch(getStringFromWasm0(arg0, arg1));
      return ret;
    };
    imports.wbg.__wbg_jsAbortRequest_3ad497cfce2a4eb6 = typeof jsAbortRequest == "function" ? jsAbortRequest : notDefined("jsAbortRequest");
    imports.wbg.__wbg_jsAttachMediaSource_69fcab1c5cd6f603 = typeof jsAttachMediaSource == "function" ? jsAttachMediaSource : notDefined("jsAttachMediaSource");
    imports.wbg.__wbg_jsRemoveMediaSource_bfd555f14dbd804d = typeof jsRemoveMediaSource == "function" ? jsRemoveMediaSource : notDefined("jsRemoveMediaSource");
    imports.wbg.__wbg_jsSetMediaSourceDuration_4c22750bc2e2b8b6 = typeof jsSetMediaSourceDuration == "function" ? jsSetMediaSourceDuration : notDefined("jsSetMediaSourceDuration");
    imports.wbg.__wbg_jsAddSourceBuffer_f769ac5dd28e087f = function(arg0, arg1, arg2) {
      const ret = jsAddSourceBuffer(arg0 >>> 0, getStringFromWasm0(arg1, arg2));
      return ret;
    };
    imports.wbg.__wbg_jsAppendBuffer_1c43e16b9b4bc73f = function(arg0, arg1, arg2) {
      const ret = jsAppendBuffer(arg0, arg1, arg2 !== 0);
      _assertClass(ret, AppendBufferResult);
      var ptr0 = ret.ptr;
      ret.ptr = 0;
      return ptr0;
    };
    imports.wbg.__wbg_jsEndOfStream_d21dc02fa7e95e93 = typeof jsEndOfStream == "function" ? jsEndOfStream : notDefined("jsEndOfStream");
    imports.wbg.__wbg_jsStartObservingPlayback_39e965722ac389f3 = typeof jsStartObservingPlayback == "function" ? jsStartObservingPlayback : notDefined("jsStartObservingPlayback");
    imports.wbg.__wbg_jsStopObservingPlayback_6ba8aea701a8a72e = typeof jsStopObservingPlayback == "function" ? jsStopObservingPlayback : notDefined("jsStopObservingPlayback");
    imports.wbg.__wbg_jsFreeResource_efd8cc10a164752b = typeof jsFreeResource == "function" ? jsFreeResource : notDefined("jsFreeResource");
    imports.wbg.__wbg_jsSeek_264e96c2a274615f = typeof jsSeek == "function" ? jsSeek : notDefined("jsSeek");
    imports.wbg.__wbindgen_throw = function(arg0, arg1) {
      throw new Error(getStringFromWasm0(arg0, arg1));
    };
    return imports;
  }
  function initMemory(imports, maybe_memory) {
  }
  function finalizeInit(instance, module) {
    wasm = instance.exports;
    init.__wbindgen_wasm_module = module;
    cachedFloat64Memory0 = new Float64Array();
    cachedInt32Memory0 = new Int32Array();
    cachedUint8Memory0 = new Uint8Array();
    return wasm;
  }
  async function init(input) {
    if (typeof input === "undefined") {
      input = new URL("wasp_hls_bg.wasm", import_meta.url);
    }
    const imports = getImports();
    if (typeof input === "string" || typeof Request === "function" && input instanceof Request || typeof URL === "function" && input instanceof URL) {
      input = fetch(input);
    }
    initMemory(imports);
    const { instance, module } = await load(await input, imports);
    return finalizeInit(instance, module);
  }
  var wasp_hls_default = init;

  // src/ts-common/idGenerator.ts
  function idGenerator() {
    let prefix = "";
    let currId = -1;
    return function generateNewId() {
      currId++;
      if (currId >= Number.MAX_SAFE_INTEGER) {
        prefix += "0";
        currId = 0;
      }
      return prefix + String(currId);
    };
  }
  function numberIdGenerator() {
    let currId = -1;
    return function generateNewNumberId() {
      currId++;
      if (currId >= Number.MAX_SAFE_INTEGER) {
        console.warn("Exceeding `numberIdGenerator` limit. Collisions may occur");
        currId = 0;
      }
      return currId;
    };
  }

  // src/ts-common/isobmff-utils.ts
  function getTrackFragmentDecodeTime(buffer) {
    const traf = getTRAF(buffer);
    if (traf === null) {
      return void 0;
    }
    const tfdt = getBoxContent(traf, 1952867444);
    if (tfdt === null) {
      return void 0;
    }
    const version = tfdt[0];
    return version === 1 ? be8toi(tfdt, 4) : version === 0 ? be4toi(tfdt, 4) : void 0;
  }
  function getDurationFromTrun(buffer) {
    const trafs = getTRAFs(buffer);
    if (trafs.length === 0) {
      return void 0;
    }
    let completeDuration = 0;
    for (const traf of trafs) {
      const trun = getBoxContent(traf, 1953658222);
      if (trun === null) {
        return void 0;
      }
      let cursor = 0;
      const version = trun[cursor];
      cursor += 1;
      if (version > 1) {
        return void 0;
      }
      const flags = be3toi(trun, cursor);
      cursor += 3;
      const hasSampleDuration = (flags & 256) > 0;
      let defaultDuration = 0;
      if (!hasSampleDuration) {
        defaultDuration = getDefaultDurationFromTFHDInTRAF(traf);
        if (defaultDuration === void 0) {
          return void 0;
        }
      }
      const hasDataOffset = (flags & 1) > 0;
      const hasFirstSampleFlags = (flags & 4) > 0;
      const hasSampleSize = (flags & 512) > 0;
      const hasSampleFlags = (flags & 1024) > 0;
      const hasSampleCompositionOffset = (flags & 2048) > 0;
      const sampleCounts = be4toi(trun, cursor);
      cursor += 4;
      if (hasDataOffset) {
        cursor += 4;
      }
      if (hasFirstSampleFlags) {
        cursor += 4;
      }
      let i = sampleCounts;
      let duration = 0;
      while (i-- > 0) {
        if (hasSampleDuration) {
          duration += be4toi(trun, cursor);
          cursor += 4;
        } else {
          duration += defaultDuration;
        }
        if (hasSampleSize) {
          cursor += 4;
        }
        if (hasSampleFlags) {
          cursor += 4;
        }
        if (hasSampleCompositionOffset) {
          cursor += 4;
        }
      }
      completeDuration += duration;
    }
    return completeDuration;
  }
  function getMDHDTimescale(buffer) {
    const mdia = getMDIA(buffer);
    if (mdia === null) {
      return void 0;
    }
    const mdhd = getBoxContent(mdia, 1835296868);
    if (mdhd === null) {
      return void 0;
    }
    let cursor = 0;
    const version = mdhd[cursor];
    cursor += 4;
    return version === 1 ? be4toi(mdhd, cursor + 16) : version === 0 ? be4toi(mdhd, cursor + 8) : void 0;
  }
  function getDefaultDurationFromTFHDInTRAF(traf) {
    const tfhd = getBoxContent(traf, 1952868452);
    if (tfhd === null) {
      return void 0;
    }
    let cursor = 1;
    const flags = be3toi(tfhd, cursor);
    cursor += 3;
    const hasBaseDataOffset = (flags & 1) > 0;
    const hasSampleDescriptionIndex = (flags & 2) > 0;
    const hasDefaultSampleDuration = (flags & 8) > 0;
    if (!hasDefaultSampleDuration) {
      return void 0;
    }
    cursor += 4;
    if (hasBaseDataOffset) {
      cursor += 8;
    }
    if (hasSampleDescriptionIndex) {
      cursor += 4;
    }
    const defaultDuration = be4toi(tfhd, cursor);
    return defaultDuration;
  }
  function getTRAF(buffer) {
    const moof = getBoxContent(buffer, 1836019558);
    if (moof === null) {
      return null;
    }
    return getBoxContent(moof, 1953653094);
  }
  function getTRAFs(buffer) {
    const moofs = getBoxesContent(buffer, 1836019558);
    return moofs.reduce((acc, moof) => {
      const traf = getBoxContent(moof, 1953653094);
      if (traf !== null) {
        acc.push(traf);
      }
      return acc;
    }, []);
  }
  function getMDIA(buf) {
    const moov = getBoxContent(buf, 1836019574);
    if (moov === null) {
      return null;
    }
    const trak = getBoxContent(moov, 1953653099);
    if (trak === null) {
      return null;
    }
    return getBoxContent(trak, 1835297121);
  }
  function getBoxContent(buf, boxName) {
    const offsets = getBoxOffsets(buf, boxName);
    return offsets !== null ? buf.subarray(offsets[1], offsets[2]) : null;
  }
  function getBoxesContent(buf, boxName) {
    const ret = [];
    let currentBuf = buf;
    while (true) {
      const offsets = getBoxOffsets(currentBuf, boxName);
      if (offsets === null) {
        return ret;
      }
      if (offsets[2] === 0 || currentBuf.length === 0) {
        throw new Error("Error while parsing ISOBMFF box");
      }
      ret.push(currentBuf.subarray(offsets[1], offsets[2]));
      currentBuf = currentBuf.subarray(offsets[2]);
    }
  }
  function getBoxOffsets(buf, boxName) {
    const len = buf.length;
    let boxBaseOffset = 0;
    let name;
    let lastBoxSize = 0;
    let lastOffset;
    while (boxBaseOffset + 8 <= len) {
      lastOffset = boxBaseOffset;
      lastBoxSize = be4toi(buf, lastOffset);
      lastOffset += 4;
      name = be4toi(buf, lastOffset);
      lastOffset += 4;
      if (lastBoxSize === 0) {
        lastBoxSize = len - boxBaseOffset;
      } else if (lastBoxSize === 1) {
        if (lastOffset + 8 > len) {
          return null;
        }
        lastBoxSize = be8toi(buf, lastOffset);
        lastOffset += 8;
      }
      if (lastBoxSize < 0) {
        throw new Error("ISOBMFF: Size out of range");
      }
      if (name === boxName) {
        if (boxName === 1970628964) {
          lastOffset += 16;
        }
        return [boxBaseOffset, lastOffset, boxBaseOffset + lastBoxSize];
      } else {
        boxBaseOffset += lastBoxSize;
      }
    }
    return null;
  }
  function be3toi(bytes, offset) {
    return bytes[offset + 0] * 65536 + bytes[offset + 1] * 256 + bytes[offset + 2];
  }
  function be4toi(bytes, offset) {
    return bytes[offset + 0] * 16777216 + bytes[offset + 1] * 65536 + bytes[offset + 2] * 256 + bytes[offset + 3];
  }
  function be8toi(bytes, offset) {
    return (bytes[offset + 0] * 16777216 + bytes[offset + 1] * 65536 + bytes[offset + 2] * 256 + bytes[offset + 3]) * 4294967296 + bytes[offset + 4] * 16777216 + bytes[offset + 5] * 65536 + bytes[offset + 6] * 256 + bytes[offset + 7];
  }

  // src/ts-common/QueuedSourceBuffer.ts
  var QueuedSourceBuffer = class {
    constructor(sourceBuffer) {
      this._sourceBuffer = sourceBuffer;
      this._queue = [];
      this._pendingTask = null;
      const intervalId = setInterval(() => {
        this._flush();
      }, 2e3);
      const onError = this._onPendingTaskError.bind(this);
      const _onUpdateEnd = () => {
        this._flush();
      };
      sourceBuffer.addEventListener("error", onError);
      sourceBuffer.addEventListener("updateend", _onUpdateEnd);
      this._dispose = [() => {
        clearInterval(intervalId);
        sourceBuffer.removeEventListener("error", onError);
        sourceBuffer.removeEventListener("updateend", _onUpdateEnd);
      }];
    }
    push(data) {
      console.debug("QSB: receiving order to push data to the SourceBuffer");
      return this._addToQueue({ type: 0 /* Push */, value: data });
    }
    removeBuffer(start, end) {
      console.debug("QSB: receiving order to remove data from the SourceBuffer", start, end);
      return this._addToQueue({
        type: 1 /* Remove */,
        value: { start, end }
      });
    }
    getBufferedRanges() {
      return this._sourceBuffer.buffered;
    }
    dispose() {
      this._dispose.forEach((disposeFn) => disposeFn());
      if (this._pendingTask !== null) {
        this._pendingTask.reject(new Error("QueuedSourceBuffer Cancelled"));
        this._pendingTask = null;
      }
      while (this._queue.length > 0) {
        const nextElement = this._queue.shift();
        if (nextElement !== void 0) {
          nextElement.reject(new Error("QueuedSourceBuffer Cancelled"));
        }
      }
    }
    _onPendingTaskError(err) {
      const error = err instanceof Error ? err : new Error("An unknown error occured when doing operations on the SourceBuffer");
      if (this._pendingTask != null) {
        this._pendingTask.reject(error);
      }
    }
    _addToQueue(operation) {
      return new Promise((resolve, reject) => {
        const shouldRestartQueue = this._queue.length === 0 && this._pendingTask === null;
        const queueItem = { resolve, reject, ...operation };
        this._queue.push(queueItem);
        if (shouldRestartQueue) {
          this._flush();
        }
      });
    }
    _flush() {
      if (this._sourceBuffer.updating) {
        return;
      }
      if (this._pendingTask !== null) {
        const task = this._pendingTask;
        const { resolve } = task;
        this._pendingTask = null;
        resolve();
        return this._flush();
      } else {
        const nextItem = this._queue.shift();
        if (nextItem === void 0) {
          return;
        } else {
          this._pendingTask = nextItem;
        }
      }
      try {
        switch (this._pendingTask.type) {
          case 0 /* Push */:
            const segmentData = this._pendingTask.value;
            if (segmentData === void 0) {
              this._flush();
              return;
            }
            console.debug("QSB: pushing data");
            this._sourceBuffer.appendBuffer(segmentData);
            break;
          case 1 /* Remove */:
            const { start, end } = this._pendingTask.value;
            console.debug("QSB: removing data from SourceBuffer", start, end);
            this._sourceBuffer.remove(start, end);
            break;
          default:
            assertUnreachable(this._pendingTask);
        }
      } catch (e) {
        this._onPendingTaskError(e);
      }
    }
  };
  function assertUnreachable(_) {
    throw new Error("Unreachable path taken");
  }

  // src/ts-worker/globals.ts
  var PlayerInstance = class {
    constructor() {
      this._instanceInfo = null;
      this.hasWorkerMse = void 0;
    }
    start(hasWorkerMse) {
      this.hasWorkerMse = hasWorkerMse;
      this._instanceInfo = {
        dispatcher: new Dispatcher(),
        content: null
      };
    }
    dispose() {
      this._instanceInfo?.dispatcher.free();
      jsMemoryResources.freeEverything();
      requestsStore.freeEverything();
    }
    changeContent(content) {
      if (this._instanceInfo === null) {
        console.error();
        return;
      }
      jsMemoryResources.freeEverything();
      requestsStore.freeEverything();
      this._instanceInfo.content = content;
    }
    getDispatcher() {
      return this._instanceInfo === null ? null : this._instanceInfo.dispatcher;
    }
    getContentInfo() {
      return this._instanceInfo === null ? null : this._instanceInfo.content;
    }
  };
  var GenericStore = class {
    constructor() {
      this._generateId = numberIdGenerator();
      this._store = {};
    }
    create(data) {
      const id = this._generateId();
      this._store[id] = data;
      return id;
    }
    delete(id) {
      delete this._store[id];
    }
    get(id) {
      return this._store[id];
    }
    freeEverything() {
      this._store = {};
    }
  };
  var playerInstance = new PlayerInstance();
  var jsMemoryResources = new GenericStore();
  var requestsStore = new GenericStore();
  function getMediaSourceObj() {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      return void 0;
    }
    const { mediaSourceObj } = contentInfo;
    if (mediaSourceObj === null) {
      return void 0;
    }
    return mediaSourceObj;
  }

  // src/ts-worker/postMessage.ts
  function postMessageToMain(msg, transferables) {
    console.debug("<-- sending to main:", msg.type);
    if (transferables === void 0) {
      postMessage(msg);
    } else {
      postMessage(msg, transferables);
    }
  }

  // src/ts-worker/segment-preparation.ts
  function getTimeInformationFromMp4(segment, initTimescale) {
    const baseDecodeTime = getTrackFragmentDecodeTime(segment);
    if (baseDecodeTime === void 0) {
      return null;
    }
    const trunDuration = getDurationFromTrun(segment);
    return {
      time: baseDecodeTime / initTimescale,
      duration: trunDuration === void 0 ? void 0 : trunDuration / initTimescale
    };
  }

  // src/ts-worker/transmux.ts
  var import_mux = __toESM(require_mux());
  var transmuxer;
  var MPEG_TS_REGEXP = /^[a-z]+\/mp2t;/i;
  function isMpegTsType(typ) {
    return MPEG_TS_REGEXP.test(typ);
  }
  function shouldTransmux(typ) {
    if (!canTransmux(typ)) {
      return false;
    }
    if (typeof MediaSource === "undefined") {
      return true;
    }
    return !MediaSource.isTypeSupported(typ);
  }
  function canTransmux(typ) {
    return isMpegTsType(typ);
  }
  function getTransmuxedType(typ, mediaType) {
    if (!canTransmux(typ)) {
      return typ;
    }
    let mimeType = typ.replace(/mp2t/i, "mp4");
    if (mediaType === MediaType.Audio) {
      mimeType = typ.replace(/video/i, "audio");
    }
    const match = /avc1\.(66|77|100)\.(\d+)/.exec(mimeType);
    if (match) {
      const profile = match[1];
      let newProfile;
      if (profile === "66") {
        newProfile = "4200";
      } else if (profile === "77") {
        newProfile = "4d00";
      } else {
        if (profile !== "100") {
          console.error("Impossible regex catch");
        }
        newProfile = "6400";
      }
      const level = Number(match[2]);
      if (level >= 256) {
        console.error("Invalid legacy avc1 level number.");
      }
      const newLevel = (level >> 4).toString(16) + (level & 15).toString(16);
      mimeType = `avc1.${newProfile}${newLevel}`;
    }
    return mimeType;
  }
  function resetTransmuxer() {
    transmuxer = void 0;
  }
  function transmux(inputSegment) {
    if (transmuxer === void 0) {
      transmuxer = new import_mux.default.mp4.Transmuxer();
    }
    const subSegments = [];
    transmuxer.on("data", function(segment) {
      const transmuxedSegment = new Uint8Array(segment.initSegment.byteLength + segment.data.byteLength);
      transmuxedSegment.set(segment.initSegment, 0);
      transmuxedSegment.set(segment.data, segment.initSegment.byteLength);
      subSegments.push(transmuxedSegment);
    });
    transmuxer.push(inputSegment);
    transmuxer.flush();
    if (subSegments.length === 0) {
      return null;
    } else if (subSegments.length === 1) {
      return subSegments[0];
    } else {
      const segmentSize = subSegments.reduce((acc, s) => {
        return acc + s.byteLength;
      }, 0);
      const fullSegment = new Uint8Array(segmentSize);
      let currOffset = 0;
      for (const subSegment of subSegments) {
        fullSegment.set(subSegment, currOffset);
        currOffset += subSegment.byteLength;
      }
      return fullSegment;
    }
  }

  // src/ts-worker/utils.ts
  function formatErrMessage(err, defaultMsg) {
    return err instanceof Error ? err.name + ": " + err.message : defaultMsg;
  }

  // src/ts-worker/bindings.ts
  var generateMediaSourceId = idGenerator();
  function getResourceData(resourceId) {
    return jsMemoryResources.get(resourceId);
  }
  function log(logLevel, logStr) {
    const now = performance.now().toFixed(2);
    switch (logLevel) {
      case LogLevel.Error:
        console.error(now, logStr);
        break;
      case LogLevel.Warn:
        console.warn(now, logStr);
        break;
      case LogLevel.Info:
        console.info(now, logStr);
        break;
      case LogLevel.Debug:
        console.debug(now, logStr);
        break;
    }
  }
  function timer(duration, reason) {
    const timerId = setTimeout(() => {
      const dispatcher = playerInstance.getDispatcher();
      if (dispatcher === null) {
        return;
      }
      dispatcher.on_timer_ended(timerId, reason);
    }, duration);
    return timerId;
  }
  function clearTimer(id) {
    clearTimeout(id);
  }
  function doFetch(url) {
    const abortController = new AbortController();
    const currentRequestId = requestsStore.create({ abortController });
    const timestampBef = performance.now();
    fetch(url, { signal: abortController.signal }).then(async (res) => {
      const arrRes = await res.arrayBuffer();
      const elapsedMs = performance.now() - timestampBef;
      requestsStore.delete(currentRequestId);
      const dispatcher = playerInstance.getDispatcher();
      if (dispatcher !== null) {
        const segmentArray = new Uint8Array(arrRes);
        const currentResourceId = jsMemoryResources.create(segmentArray);
        dispatcher.on_request_finished(currentRequestId, currentResourceId, segmentArray.byteLength, res.url, elapsedMs);
      }
    }).catch((err) => {
      requestsStore.delete(currentRequestId);
      if (err instanceof Error && err.name === "AbortError") {
        return;
      }
    });
    return currentRequestId;
  }
  function abortRequest(id) {
    const requestObj = requestsStore.get(id);
    if (requestObj !== void 0) {
      requestObj.abortController.abort();
      Promise.resolve().then(() => {
        requestsStore.delete(id);
      });
      return true;
    }
    return false;
  }
  function seek(position) {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null || contentInfo.mediaSourceObj === null) {
      console.error("Attempting to seek when no MediaSource is created");
      return;
    }
    postMessageToMain({
      type: "seek",
      value: {
        mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
        position
      }
    });
  }
  function attachMediaSource() {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      return;
    }
    try {
      let onMediaSourceEnded = function() {
        playerInstance.getDispatcher()?.on_media_source_state_change(MediaSourceReadyState.Ended);
      }, onMediaSourceOpen = function() {
        playerInstance.getDispatcher()?.on_media_source_state_change(MediaSourceReadyState.Open);
      }, onMediaSourceClose = function() {
        playerInstance.getDispatcher()?.on_media_source_state_change(MediaSourceReadyState.Closed);
      };
      if (playerInstance.hasWorkerMse !== true) {
        const mediaSourceId = generateMediaSourceId();
        contentInfo.mediaSourceObj = {
          nextSourceBufferId: 0,
          sourceBuffers: [],
          type: "main",
          mediaSourceId
        };
        postMessageToMain({
          type: "create-media-source",
          value: {
            contentId: contentInfo.contentId,
            mediaSourceId
          }
        });
      } else {
        const mediaSource = new MediaSource();
        mediaSource.addEventListener("sourceclose", onMediaSourceClose);
        mediaSource.addEventListener("sourceended", onMediaSourceEnded);
        mediaSource.addEventListener("sourceopen", onMediaSourceOpen);
        const removeEventListeners = () => {
          mediaSource.removeEventListener("sourceclose", onMediaSourceClose);
          mediaSource.removeEventListener("sourceended", onMediaSourceEnded);
          mediaSource.removeEventListener("sourceopen", onMediaSourceOpen);
        };
        const handle = mediaSource.handle;
        let objectURL;
        if (handle === void 0 || handle === null) {
          objectURL = URL.createObjectURL(mediaSource);
        }
        const mediaSourceId = generateMediaSourceId();
        contentInfo.mediaSourceObj = {
          type: "worker",
          mediaSourceId,
          mediaSource,
          removeEventListeners,
          sourceBuffers: [],
          nextSourceBufferId: 0
        };
        postMessageToMain({
          type: "attach-media-source",
          value: {
            contentId: contentInfo.contentId,
            handle,
            src: objectURL,
            mediaSourceId
          }
        }, handle !== void 0 ? [handle] : []);
      }
    } catch (e) {
      scheduleMicrotask(() => {
      });
    }
  }
  function scheduleMicrotask(fn) {
    if (typeof queueMicrotask === "function") {
      queueMicrotask(fn);
    } else {
      Promise.resolve().then(fn).catch(() => {
      });
    }
  }
  function removeMediaSource() {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      return;
    }
    if (contentInfo.mediaSourceObj === null) {
      return;
    }
    if (contentInfo.mediaSourceObj.type === "worker") {
      const {
        mediaSource,
        removeEventListeners
      } = contentInfo.mediaSourceObj;
      removeEventListeners();
      if (mediaSource !== null && mediaSource.readyState !== "closed") {
        const { readyState, sourceBuffers } = mediaSource;
        for (let i = sourceBuffers.length - 1; i >= 0; i--) {
          const sourceBuffer = sourceBuffers[i];
          if (!sourceBuffer.updating) {
            try {
              if (readyState === "open") {
                sourceBuffer.abort();
              }
              mediaSource.removeSourceBuffer(sourceBuffer);
            } catch (e) {
              const msg = formatErrMessage(e, "Unknown error while removing SourceBuffer");
              Dispatcher.log(LogLevel.Error, "Could not remove SourceBuffer: " + msg);
            }
          }
        }
      }
    }
    postMessageToMain({
      type: "clear-media-source",
      value: { mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId }
    });
  }
  function setMediaSourceDuration(duration) {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      return;
    }
    if (contentInfo.mediaSourceObj === null) {
      return;
    }
    if (contentInfo.mediaSourceObj.type === "worker") {
      try {
        contentInfo.mediaSourceObj.mediaSource.duration = duration;
      } catch (err) {
      }
    } else {
      postMessageToMain({
        type: "update-media-source-duration",
        value: {
          mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
          duration
        }
      });
    }
  }
  function addSourceBuffer(mediaType, typ) {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      throw new Error("Error 1");
    }
    if (contentInfo.mediaSourceObj === null) {
      throw new Error("Error 2");
    }
    if (contentInfo.mediaSourceObj.type === "main") {
      const {
        sourceBuffers,
        nextSourceBufferId
      } = contentInfo.mediaSourceObj;
      try {
        let mimeType = typ;
        if (shouldTransmux(typ)) {
          mimeType = getTransmuxedType(typ, mediaType);
        }
        const sourceBufferId = nextSourceBufferId;
        sourceBuffers.push({
          lastInitTimescale: void 0,
          id: sourceBufferId,
          transmuxer: mimeType === typ ? null : transmux,
          sourceBuffer: null
        });
        contentInfo.mediaSourceObj.nextSourceBufferId++;
        postMessageToMain({
          type: "create-source-buffer",
          value: {
            mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId,
            sourceBufferId,
            contentType: mimeType
          }
        });
        return sourceBufferId;
      } catch (err) {
        throw err;
      }
    } else {
      const {
        mediaSource,
        sourceBuffers,
        nextSourceBufferId
      } = contentInfo.mediaSourceObj;
      if (mediaSource.readyState === "closed") {
        throw new Error("A");
      }
      if (typ === "") {
        throw new Error("B");
      }
      try {
        let mimeType = typ;
        if (shouldTransmux(typ)) {
          mimeType = getTransmuxedType(typ, mediaType);
        }
        const sourceBuffer = mediaSource.addSourceBuffer(mimeType);
        const sourceBufferId = nextSourceBufferId;
        const queuedSourceBuffer = new QueuedSourceBuffer(sourceBuffer);
        sourceBuffers.push({
          lastInitTimescale: void 0,
          id: sourceBufferId,
          sourceBuffer: queuedSourceBuffer,
          transmuxer: mimeType === typ ? null : transmux
        });
        contentInfo.mediaSourceObj.nextSourceBufferId++;
        return sourceBufferId;
      } catch (err) {
        throw new Error("C");
      }
    }
  }
  function appendBuffer(sourceBufferId, resourceId, parseTimeInformation) {
    let segment = jsMemoryResources.get(resourceId);
    const mediaSourceObj = getMediaSourceObj();
    if (segment === void 0) {
      return AppendBufferResult.error(AppendBufferErrorCode.NoResource, "Segment preparation error: No resource with the given `resourceId`");
    }
    if (mediaSourceObj === void 0) {
      return AppendBufferResult.error(AppendBufferErrorCode.NoSourceBuffer, "Segment preparation error: No MediaSource attached");
    }
    const sourceBufferObjIdx = mediaSourceObj.sourceBuffers.findIndex(({ id }) => id === sourceBufferId);
    if (sourceBufferObjIdx < -1) {
      return AppendBufferResult.error(AppendBufferErrorCode.NoSourceBuffer, "Segment preparation error: No SourceBuffer with the given `SourceBufferId`");
    }
    const sourceBufferObj = mediaSourceObj.sourceBuffers[sourceBufferObjIdx];
    if (sourceBufferObj.transmuxer !== null) {
      try {
        const transmuxedData = sourceBufferObj.transmuxer(segment);
        if (transmuxedData !== null) {
          segment = transmuxedData;
        } else {
          return AppendBufferResult.error(AppendBufferErrorCode.TransmuxerError, "Segment preparation error: the transmuxer couldn't process the segment");
        }
      } catch (err) {
        const msg = formatErrMessage(err, "Unknown error while transmuxing segment");
        return AppendBufferResult.error(AppendBufferErrorCode.TransmuxerError, msg);
      }
    }
    let timescale = getMDHDTimescale(segment);
    if (timescale !== void 0) {
      sourceBufferObj.lastInitTimescale = timescale;
    } else {
      timescale = sourceBufferObj.lastInitTimescale;
    }
    let timeInfo;
    if (parseTimeInformation === true && timescale !== void 0) {
      timeInfo = getTimeInformationFromMp4(segment, timescale);
    }
    try {
      if (sourceBufferObj.sourceBuffer !== null) {
        sourceBufferObj.sourceBuffer.push(segment).then(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_update(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_update`", err);
          }
        }).catch(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_error(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_error`", err);
          }
        });
      } else {
        const buffer = segment.buffer;
        postMessageToMain({
          type: "append-buffer",
          value: {
            mediaSourceId: mediaSourceObj.mediaSourceId,
            sourceBufferId,
            data: buffer
          }
        }, [buffer]);
      }
    } catch (err) {
      return AppendBufferResult.error(AppendBufferErrorCode.UnknownError);
    }
    return AppendBufferResult.success(timeInfo?.time, timeInfo?.duration);
  }
  function removeBuffer(sourceBufferId, start, end) {
    try {
      const mediaSourceObj = getMediaSourceObj();
      if (mediaSourceObj === void 0) {
        return;
      }
      if (mediaSourceObj.type === "worker") {
        const sourceBuffer = mediaSourceObj.sourceBuffers.find(({ id }) => id === sourceBufferId);
        if (sourceBuffer === void 0) {
          return;
        }
        sourceBuffer.sourceBuffer.removeBuffer(start, end).then(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_update(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_update`", err);
          }
        }).catch(() => {
          try {
            playerInstance.getDispatcher()?.on_source_buffer_error(sourceBufferId);
          } catch (err) {
            console.error("Error when calling `on_source_buffer_error`", err);
          }
        });
      } else {
        postMessageToMain({
          type: "remove-buffer",
          value: {
            mediaSourceId: mediaSourceObj.mediaSourceId,
            sourceBufferId,
            start,
            end
          }
        });
      }
    } catch (err) {
      return;
    }
  }
  function endOfStream() {
    try {
      const mediaSourceObj = getMediaSourceObj();
      if (mediaSourceObj === void 0) {
        return;
      }
      if (mediaSourceObj.type === "worker") {
        mediaSourceObj.mediaSource.endOfStream();
      } else {
        postMessageToMain({
          type: "end-of-stream",
          value: { mediaSourceId: mediaSourceObj.mediaSourceId }
        });
      }
    } catch (err) {
      return;
    }
  }
  function startObservingPlayback() {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      return;
    }
    if (contentInfo.mediaSourceObj === null) {
      return;
    }
    postMessageToMain({
      type: "start-playback-observation",
      value: { mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId }
    });
  }
  function stopObservingPlayback() {
    const contentInfo = playerInstance.getContentInfo();
    if (contentInfo === null) {
      return;
    }
    if (contentInfo.mediaSourceObj === null) {
      return;
    }
    postMessageToMain({
      type: "stop-playback-observation",
      value: { mediaSourceId: contentInfo.mediaSourceObj.mediaSourceId }
    });
  }
  function freeResource(resourceId) {
    if (jsMemoryResources.get(resourceId) === void 0) {
      return false;
    }
    jsMemoryResources.delete(resourceId);
    return true;
  }
  var global2 = self;
  global2.jsLog = log;
  global2.jsFetch = doFetch;
  global2.jsAbortRequest = abortRequest;
  global2.jsAttachMediaSource = attachMediaSource;
  global2.jsRemoveMediaSource = removeMediaSource;
  global2.jsSetMediaSourceDuration = setMediaSourceDuration;
  global2.jsAddSourceBuffer = addSourceBuffer;
  global2.jsAppendBuffer = appendBuffer;
  global2.jsRemoveBuffer = removeBuffer;
  global2.jsEndOfStream = endOfStream;
  global2.jsStartObservingPlayback = startObservingPlayback;
  global2.jsStopObservingPlayback = stopObservingPlayback;
  global2.jsFreeResource = freeResource;
  global2.jsSeek = seek;
  global2.jsTimer = timer;
  global2.jsClearTimer = clearTimer;
  global2.jsGetResourceData = getResourceData;

  // src/ts-worker/MessageReceiver.ts
  var wasInitializedCalled = false;
  function MessageReceiver() {
    onmessage = function(evt) {
      if (evt.origin !== "") {
        console.error("Unexpected trans-origin message");
        return;
      }
      const { data } = evt;
      if (typeof data !== "object" || data === null || typeof data.type !== "string") {
        console.error("unexpected main message");
        return;
      }
      switch (data.type) {
        case "init":
          if (wasInitializedCalled) {
            return handleInitializationError("Worker initialization already done", 0 /* AlreadyInitializedError */);
          }
          wasInitializedCalled = true;
          const { wasmUrl, hasWorkerMse } = data.value;
          initialize(wasmUrl, hasWorkerMse);
          break;
        case "dispose":
          dispose();
          break;
        case "load": {
          const dispatcher = playerInstance.getDispatcher();
          if (dispatcher === null) {
            return postUnitializedWorkerError(data.value.contentId);
          }
          const contentInfo = {
            contentId: data.value.contentId,
            mediaSourceObj: null,
            observationsObj: null
          };
          playerInstance.changeContent(contentInfo);
          resetTransmuxer();
          dispatcher.load_content(data.value.url);
          break;
        }
        case "stop": {
          const dispatcher = playerInstance.getDispatcher();
          if (dispatcher === null) {
            return postUnitializedWorkerError(data.value.contentId);
          }
          dispatcher.stop();
          break;
        }
        case "media-source-state-changed": {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (dispatcher === null || contentInfo === null || contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          dispatcher.on_media_source_state_change(data.value.state);
          break;
        }
        case "source-buffer-updated": {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (dispatcher === null || contentInfo === null || contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          dispatcher.on_source_buffer_update(data.value.sourceBufferId);
          break;
        }
        case "observation": {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (dispatcher === null || contentInfo === null || contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          const mediaObservation = new MediaObservation(data.value.reason, data.value.currentTime, data.value.readyState, data.value.buffered, data.value.paused, data.value.seeking);
          dispatcher.on_playback_tick(mediaObservation);
          break;
        }
        case "create-media-source-error": {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (dispatcher === null || contentInfo === null || contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          dispatcher.stop();
          break;
        }
        case "update-media-source-duration-error": {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (dispatcher === null || contentInfo === null || contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          console.error("Error: when setting the MediaSource's duration");
          break;
        }
        case "create-source-buffer-error": {
          const dispatcher = playerInstance.getDispatcher();
          const contentInfo = playerInstance.getContentInfo();
          if (dispatcher === null || contentInfo === null || contentInfo.mediaSourceObj?.mediaSourceId !== data.value.mediaSourceId) {
            return;
          }
          dispatcher.stop();
          break;
        }
      }
    };
  }
  function handleInitializationError(err, code) {
    let message;
    if (typeof err === "string") {
      message = err;
    } else if (err instanceof Error) {
      message = err.message;
    }
    postMessageToMain({
      type: "initialization-error",
      value: {
        code,
        message
      }
    });
  }
  function postUnitializedWorkerError(contentId) {
    postMessageToMain({
      type: "content-error",
      value: {
        contentId,
        message: "Error: Worker not initialized.",
        code: 0 /* UnitializedError */
      }
    });
  }
  function initialize(wasmUrl, hasWorkerMse) {
    wasp_hls_default(fetch(wasmUrl)).then(() => {
      playerInstance.start(hasWorkerMse);
      postMessageToMain({ type: "initialized", value: null });
    }).catch((err) => {
      handleInitializationError(err, 1 /* WasmRequestError */);
    });
  }
  function dispose() {
    stopObservingPlayback();
    playerInstance.dispose();
  }

  // src/ts-worker/index.ts
  MessageReceiver();
})();
/*! @name mux.js @version 6.1.0 @license Apache-2.0 */
