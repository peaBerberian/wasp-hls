// IE11 doesn't support indexOf for TypedArrays.
// Once IE11 support is dropped, this function should be removed.
function typedArrayIndexOf(
  typedArray: Uint8Array | null,
  element: number,
  fromIndex: number
): number {
  if (typedArray === null) {
    return -1;
  }

  let currentIndex = fromIndex;
  for (; currentIndex < typedArray.length; currentIndex++) {
    if (typedArray[currentIndex] === element) {
      return currentIndex;
    }
  }
  return -1;
}

function getNullTerminatedStr(str: string): string {
  const indexOf = str.indexOf("\0");
  if (indexOf < 0) {
    return str;
  }
  return str.substring(0, indexOf);
}

const textEncodingDescriptionByte = {
  Iso88591: 0x00, // ISO-8859-1, terminated with \0.
  Utf16: 0x01, // UTF-16 encoded Unicode BOM, terminated with \0\0
  Utf16be: 0x02, // UTF-16BE encoded Unicode, without BOM, terminated with \0\0
  Utf8: 0x03, // UTF-8 encoded Unicode, terminated with \0
};

// return a percent-encoded representation of the specified byte range
// @see http://en.wikipedia.org/wiki/Percent-encoding
function percentEncode(bytes: Uint8Array, start: number, end: number): string {
  let result = "";
  for (let i = start; i < end; i++) {
    result += "%" + ("00" + bytes[i].toString(16)).slice(-2);
  }
  return result;
}
// return the string representation of the specified byte range,
// interpreted as UTf-8.
function parseUtf8(bytes: Uint8Array, start: number, end: number): string {
  return decodeURIComponent(percentEncode(bytes, start, end));
}
// return the string representation of the specified byte range,
// interpreted as ISO-8859-1.
function parseIso88591(bytes: Uint8Array, start: number, end: number): string {
  return unescape(percentEncode(bytes, start, end)); // jshint ignore:line
}
function parseSyncSafeInteger(data: Uint8Array): number {
  return (data[0] << 21) | (data[1] << 14) | (data[2] << 7) | data[3];
}

const frameParsers = {
  /* eslint-disable @typescript-eslint/naming-convention */
  /* eslint-disable object-shorthand */
  APIC(frame: any): void {
    let i = 1;
    const LINK_MIME_TYPE = "-->";

    if (frame.data[0] !== textEncodingDescriptionByte.Utf8) {
      // ignore frames with unrecognized character encodings
      return;
    }

    // parsing fields [ID3v2.4.0 section 4.14.]
    const mimeTypeEndIndex = typedArrayIndexOf(frame.data, 0, i);
    if (mimeTypeEndIndex < 0) {
      // malformed frame
      return;
    }

    // parsing Mime type field (terminated with \0)
    frame.mimeType = parseIso88591(frame.data, i, mimeTypeEndIndex);
    i = mimeTypeEndIndex + 1;

    // parsing 1-byte Picture Type field
    frame.pictureType = frame.data[i];
    i++;

    const descriptionEndIndex = typedArrayIndexOf(frame.data, 0, i);
    if (descriptionEndIndex < 0) {
      // malformed frame
      return;
    }

    // parsing Description field (terminated with \0)
    frame.description = parseUtf8(frame.data, i, descriptionEndIndex);
    i = descriptionEndIndex + 1;

    if (frame.mimeType === LINK_MIME_TYPE) {
      // parsing Picture Data field as URL (always represented as ISO-8859-1
      // [ID3v2.4.0 section 4.])
      frame.url = parseIso88591(frame.data, i, frame.data.length);
    } else {
      // parsing Picture Data field as binary data
      frame.pictureData = frame.data.subarray(i, frame.data.length);
    }
  },

  "T*": function (frame: any): void {
    if (frame.data[0] !== textEncodingDescriptionByte.Utf8) {
      // ignore frames with unrecognized character encodings
      return;
    }

    // parse text field, do not include null terminator in the frame value
    // frames that allow different types of encoding contain terminated text
    // [ID3v2.4.0 section 4.]
    frame.value = getNullTerminatedStr(
      parseUtf8(frame.data, 1, frame.data.length)
    );
    // text information frames supports multiple strings, stored as a terminator
    // separated list [ID3v2.4.0 section 4.2.]
    frame.values = frame.value.split("\0");
  },
  TXXX: function (frame: any): void {
    if (frame.data[0] !== textEncodingDescriptionByte.Utf8) {
      // ignore frames with unrecognized character encodings
      return;
    }

    const descriptionEndIndex = typedArrayIndexOf(frame.data, 0, 1);

    if (descriptionEndIndex === -1) {
      return;
    }

    // parse the text fields
    frame.description = parseUtf8(frame.data, 1, descriptionEndIndex);
    // do not include the null terminator in the tag value
    // frames that allow different types of encoding contain terminated text
    // [ID3v2.4.0 section 4.]
    frame.value = getNullTerminatedStr(
      parseUtf8(frame.data, descriptionEndIndex + 1, frame.data.length)
    );
    frame.data = frame.value;
  },
  "W*": function (frame: any): void {
    // parse URL field; URL fields are always represented as ISO-8859-1
    // [ID3v2.4.0 section 4.]
    // if the value is followed by a string termination all the following
    // information should be ignored [ID3v2.4.0 section 4.3]
    frame.url = getNullTerminatedStr(
      parseIso88591(frame.data, 0, frame.data.length)
    );
  },
  WXXX: function (frame: any): void {
    if (frame.data[0] !== textEncodingDescriptionByte.Utf8) {
      // ignore frames with unrecognized character encodings
      return;
    }

    const descriptionEndIndex = typedArrayIndexOf(frame.data, 0, 1);

    if (descriptionEndIndex === -1) {
      return;
    }

    // parse the description and URL fields
    frame.description = parseUtf8(frame.data, 1, descriptionEndIndex);
    // URL fields are always represented as ISO-8859-1 [ID3v2.4.0 section 4.]
    // if the value is followed by a string termination all the following information
    // should be ignored [ID3v2.4.0 section 4.3]
    frame.url = getNullTerminatedStr(
      parseIso88591(frame.data, descriptionEndIndex + 1, frame.data.length)
    );
  },
  PRIV: function (frame: any): void {
    let i: number;
    for (i = 0; i < frame.data.length; i++) {
      if (frame.data[i] === 0) {
        // parse the description and URL fields
        frame.owner = parseIso88591(frame.data, 0, i);
        break;
      }
    }
    frame.privateData = frame.data.subarray(i + 1);
    frame.data = frame.privateData;
  },
  /* eslint-enable @typescript-eslint/naming-convention */
  /* eslint-enable object-shorthand */
};

function parseId3Frames(data: Uint8Array) {
  let frameStart = 10;
  let tagSize = 0;
  const frames: any[] = [];

  // If we don't have enough data for a header, 10 bytes,
  // or 'ID3' in the first 3 bytes this is not a valid ID3 tag.
  if (
    data.length < 10 ||
    data[0] !== "I".charCodeAt(0) ||
    data[1] !== "D".charCodeAt(0) ||
    data[2] !== "3".charCodeAt(0)
  ) {
    return;
  }
  // the frame size is transmitted as a 28-bit integer in the
  // last four bytes of the ID3 header.
  // The most significant bit of each byte is dropped and the
  // results concatenated to recover the actual value.
  tagSize = parseSyncSafeInteger(data.subarray(6, 10));

  // ID3 reports the tag size excluding the header but it's more
  // convenient for our comparisons to include it
  tagSize += 10;
  // check bit 6 of byte 5 for the extended header flag.
  const hasExtendedHeader = data[5] & 0x40;
  if (hasExtendedHeader !== 0) {
    // advance the frame start past the extended header
    frameStart += 4; // header size field
    frameStart += parseSyncSafeInteger(data.subarray(10, 14));

    // clip any padding off the end
    tagSize -= parseSyncSafeInteger(data.subarray(16, 20));
  }

  let frameSize: number;
  let frameHeader: string;

  // parse one or more ID3 frames
  // http://id3.org/id3v2.3.0#ID3v2_frame_overview
  do {
    // determine the number of bytes in this frame
    frameSize = parseSyncSafeInteger(
      data.subarray(frameStart + 4, frameStart + 8)
    );
    if (frameSize < 1) {
      break;
    }
    frameHeader = String.fromCharCode(
      data[frameStart],
      data[frameStart + 1],
      data[frameStart + 2],
      data[frameStart + 3]
    );

    const frame = {
      id: frameHeader,
      data: data.subarray(frameStart + 10, frameStart + frameSize + 10),
      key: frameHeader,
    };

    // parse frame values
    if (frameParsers[frame.id as keyof typeof frameParsers] !== undefined) {
      // use frame specific parser
      frameParsers[frame.id as keyof typeof frameParsers](frame);
    } else if (frame.id[0] === "T") {
      // use text frame generic parser
      frameParsers["T*"](frame);
    } else if (frame.id[0] === "W") {
      // use URL link frame generic parser
      frameParsers["W*"](frame);
    }

    frames.push(frame);

    frameStart += 10; // advance past the frame header
    frameStart += frameSize; // advance past the frame body
  } while (frameStart < tagSize);

  return frames;
}

export { parseId3Frames, parseSyncSafeInteger, frameParsers };
