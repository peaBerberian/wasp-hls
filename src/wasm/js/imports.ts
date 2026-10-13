import type {
  ContentCompatibilityErrorCode,
  LogLevel,
  MediaPlaylistParsingErrorCode,
  MediaType,
  MultivariantPlaylistParsingErrorCode,
  OtherErrorCode,
  PlaylistNature,
  PlaylistType,
  PushedSegmentErrorCode,
  RequestErrorReason,
  SourceBufferCreationErrorCode,
  TimerReason,
} from "./enums.js";
import { SegmentParsingErrorCode } from "./enums.js";
import {
  getWasmExports,
  getUint8Memory,
  getUint32Memory,
  readOptionalF64,
  readString,
  writeOptionalString,
} from "./memory.js";
import {
  rawOptionalId,
  unwrapResult,
  writeAppendBufferResult,
} from "./helpers.js";
import type { HostBindings } from "./types.js";

export function createWasmImports(bindings: HostBindings): WebAssembly.Imports {
  return {
    wasp: {
      __js_func__log(logLevel: number, ptr: number, len: number): void {
        bindings.log(logLevel as LogLevel, readString(ptr, len));
      },
      __js_func__timer(duration: number, reason: number): number {
        return bindings.timer(duration, reason as TimerReason);
      },
      __js_func__clear_timer(id: number): void {
        bindings.clearTimer(id);
      },
      __js_func__get_resource_len(id: number): number {
        const data = bindings.getResourceData(id);
        return data == null ? -1 : data.byteLength;
      },
      __js_func__copy_resource_data(
        id: number,
        destPtr: number,
        destLen: number,
      ): number {
        const data = bindings.getResourceData(id);
        if (data == null) {
          return 0;
        }
        getUint8Memory().set(data.subarray(0, destLen), destPtr);
        return 1;
      },
      __js_func__fetch(
        urlPtr: number,
        urlLen: number,
        hasRangeBase: number,
        rangeBase: number,
        hasRangeEnd: number,
        rangeEnd: number,
        timeout: number,
      ): number {
        return bindings.fetch(
          readString(urlPtr, urlLen),
          hasRangeBase !== 0 ? rangeBase : undefined,
          hasRangeEnd !== 0 ? rangeEnd : undefined,
          timeout,
        );
      },
      __js_func__abort_request(requestId: number): number {
        return bindings.abortRequest(requestId) ? 1 : 0;
      },
      __js_func__attach_media_source(
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        return unwrapResult(
          bindings.attachMediaSource(),
          1,
          errCodeOut,
          errDescPtrOut,
          errDescLenOut,
        );
      },
      __js_func__remove_media_source(
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        return unwrapResult(
          bindings.removeMediaSource(),
          1,
          errCodeOut,
          errDescPtrOut,
          errDescLenOut,
        );
      },
      __js_func__set_media_source_duration(
        duration: number,
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        return unwrapResult(
          bindings.setMediaSourceDuration(duration),
          1,
          errCodeOut,
          errDescPtrOut,
          errDescLenOut,
        );
      },
      __js_func__add_source_buffer(
        mediaType: number,
        typPtr: number,
        typLen: number,
        sourceBufferIdOut: number,
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        const result = bindings.addSourceBuffer(
          mediaType as MediaType,
          readString(typPtr, typLen),
        );
        if (result.errorCode === undefined) {
          getUint32Memory()[sourceBufferIdOut >>> 2] = result.value ?? 0;
          return 1;
        }
        getUint32Memory()[errCodeOut >>> 2] = result.errorCode;
        writeOptionalString(result.description, errDescPtrOut, errDescLenOut);
        return 0;
      },
      __js_func__is_type_supported(
        mediaType: number,
        typPtr: number,
        typLen: number,
      ): number {
        const value = bindings.isTypeSupported(
          mediaType as MediaType,
          readString(typPtr, typLen),
        );
        return value === undefined ? -1 : value ? 1 : 0;
      },
      __js_func__inspect_segment(
        resourceId: number,
        mediaTypeOut: number,
        parsedMimeTypePtrOut: number,
        parsedMimeTypeLenOut: number,
        codecPtrOut: number,
        codecLenOut: number,
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        const result = bindings.inspectSegment(resourceId);
        if (result.errorCode === undefined && result.value !== undefined) {
          getUint32Memory()[mediaTypeOut >>> 2] = result.value.mediaType;
          writeOptionalString(
            result.value.mimeType,
            parsedMimeTypePtrOut,
            parsedMimeTypeLenOut,
          );
          writeOptionalString(result.value.codec, codecPtrOut, codecLenOut);
          return 1;
        }
        getUint32Memory()[errCodeOut >>> 2] =
          result.errorCode ?? SegmentParsingErrorCode.UnknownError;
        writeOptionalString(result.description, errDescPtrOut, errDescLenOut);
        return 0;
      },
      __js_func__append_buffer(
        // params
        sourceBufferId: number,
        resourceId: number,
        baseDecodeTimeStartHi: number,
        baseDecodeTimeStartLo: number,
        baseDecodeTimeStartTimescale: number,
        reset_transmuxer_state: number,

        // Return value
        hasStartOut: number,
        startValueHiOut: number,
        startValueLoOut: number,
        hasEndOut: number,
        endValueHiOut: number,
        endValueLoOut: number,
        timescale: number,

        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        return writeAppendBufferResult(
          bindings.appendBuffer(sourceBufferId, resourceId, {
            baseDecodeTimeStartHi,
            baseDecodeTimeStartLo,
            baseDecodeTimeStartTimescale,
            resetTransmuxerState: reset_transmuxer_state !== 0,
          }),

          hasStartOut,
          startValueHiOut,
          startValueLoOut,
          hasEndOut,
          endValueHiOut,
          endValueLoOut,
          timescale,
          errCodeOut,
          errDescPtrOut,
          errDescLenOut,
        );
      },
      __js_func__remove_buffer(
        sourceBufferId: number,
        start: number,
        end: number,
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        return unwrapResult(
          bindings.removeBuffer(sourceBufferId, start, end),
          1,
          errCodeOut,
          errDescPtrOut,
          errDescLenOut,
        );
      },
      __js_func__end_of_stream(
        errCodeOut: number,
        errDescPtrOut: number,
        errDescLenOut: number,
      ): number {
        return unwrapResult(
          bindings.endOfStream(),
          1,
          errCodeOut,
          errDescPtrOut,
          errDescLenOut,
        );
      },
      __js_func__start_observing_playback(): void {
        bindings.startObservingPlayback();
      },
      __js_func__stop_observing_playback(): void {
        bindings.stopObservingPlayback();
      },
      __js_func__free_resource(resourceId: number): number {
        return bindings.freeResource(resourceId) ? 1 : 0;
      },
      __js_func__set_playback_rate(playbackRate: number): void {
        bindings.setPlaybackRate(playbackRate);
      },
      __js_func__seek(position: number): void {
        bindings.seek(position);
      },
      __js_func__flush(): void {
        bindings.flush();
      },
      __js_func__set_media_offset(mediaOffset: number): void {
        bindings.setMediaOffset(mediaOffset);
      },
      __js_func__update_content_info(
        hasMin: number,
        min: number,
        hasMax: number,
        max: number,
        playlistNat: number,
        isFinalized: number,
        usesProgramDateTime: number,
      ): void {
        bindings.updateContentInfo(
          hasMin !== 0 ? min : undefined,
          hasMax !== 0 ? max : undefined,
          playlistNat as PlaylistNature,
          isFinalized !== 0,
          usesProgramDateTime !== 0,
        );
      },
      __js_func__announce_fetched_content(
        playlistType: number,
        variantInfoPtr: number,
        variantInfoLen: number,
        audioTracksInfoPtr: number,
        audioTracksInfoLen: number,
      ): void {
        const buffer = getWasmExports().memory.buffer;
        bindings.announceFetchedContent(
          playlistType as PlaylistType,
          new Uint32Array(buffer, variantInfoPtr, variantInfoLen),
          new Uint32Array(buffer, audioTracksInfoPtr, audioTracksInfoLen),
        );
      },
      __js_func__announce_variant_update(variantId: number): void {
        bindings.announceVariantUpdate(rawOptionalId(variantId));
      },
      __js_func__announce_track_update(
        mediaType: number,
        trackId: number,
        isSelected: number,
      ): void {
        bindings.announceTrackUpdate(
          mediaType as MediaType,
          rawOptionalId(trackId),
          isSelected !== 0,
        );
      },
      __js_func__announce_variant_lock_status_change(variantId: number): void {
        bindings.announceVariantLockStatusChange(rawOptionalId(variantId));
      },
      __js_func__start_rebuffering(): void {
        bindings.startRebuffering();
      },
      __js_func__stop_rebuffering(): void {
        bindings.stopRebuffering();
      },
      __js_func__monotonic_now(): number {
        return bindings.monotonicNow();
      },
      __js_func__get_random(): number {
        return bindings.getRandom();
      },
      __js_func__send_segment_request_error(
        fatal: number,
        urlPtr: number,
        urlLen: number,
        isInit: number,
        mediaType: number,
        startPtr: number,
        durationPtr: number,
        reason: number,
        status: number,
      ): void {
        const start = readOptionalF64(startPtr);
        const duration = readOptionalF64(durationPtr);
        bindings.sendSegmentRequestError(
          fatal !== 0,
          readString(urlPtr, urlLen),
          isInit !== 0,
          start === undefined || duration === undefined
            ? undefined
            : [start, duration],
          rawOptionalId(mediaType) as MediaType | undefined,
          reason as RequestErrorReason,
          rawOptionalId(status),
        );
      },
      __js_func__send_multivariant_playlist_request_error(
        fatal: number,
        urlPtr: number,
        urlLen: number,
        reason: number,
        status: number,
      ): void {
        bindings.sendMultivariantPlaylistRequestError(
          fatal !== 0,
          readString(urlPtr, urlLen),
          reason as RequestErrorReason,
          rawOptionalId(status),
        );
      },
      __js_func__send_media_playlist_request_error(
        fatal: number,
        urlPtr: number,
        urlLen: number,
        reason: number,
        mediaType: number,
        status: number,
      ): void {
        bindings.sendMediaPlaylistRequestError(
          fatal !== 0,
          readString(urlPtr, urlLen),
          reason as RequestErrorReason,
          rawOptionalId(mediaType) as MediaType | undefined,
          rawOptionalId(status),
        );
      },
      __js_func__send_source_buffer_creation_error(
        fatal: number,
        code: number,
        mediaType: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendSourceBufferCreationError(
          fatal !== 0,
          code as SourceBufferCreationErrorCode,
          mediaType as MediaType,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_multivariant_playlist_parsing_error(
        fatal: number,
        code: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendMultivariantPlaylistParsingError(
          fatal !== 0,
          code as MultivariantPlaylistParsingErrorCode,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_media_playlist_parsing_error(
        fatal: number,
        code: number,
        mediaType: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendMediaPlaylistParsingError(
          fatal !== 0,
          code as MediaPlaylistParsingErrorCode,
          rawOptionalId(mediaType) as MediaType | undefined,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_segment_parsing_error(
        fatal: number,
        code: number,
        mediaType: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendSegmentParsingError(
          fatal !== 0,
          code as SegmentParsingErrorCode,
          rawOptionalId(mediaType) as MediaType | undefined,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_pushed_segment_error(
        fatal: number,
        code: number,
        mediaType: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendPushedSegmentError(
          fatal !== 0,
          code as PushedSegmentErrorCode,
          mediaType as MediaType,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_remove_buffer_error(
        fatal: number,
        mediaType: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendRemoveBufferError(
          fatal !== 0,
          mediaType as MediaType,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_other_error(
        fatal: number,
        code: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendOtherError(
          fatal !== 0,
          code as OtherErrorCode,
          readString(messagePtr, messageLen),
        );
      },
      __js_func__send_content_compatibility_error(
        fatal: number,
        code: number,
        messagePtr: number,
        messageLen: number,
      ): void {
        bindings.sendContentCompatibilityError(
          fatal !== 0,
          code as ContentCompatibilityErrorCode,
          readString(messagePtr, messageLen),
        );
      },
    },
  };
}
