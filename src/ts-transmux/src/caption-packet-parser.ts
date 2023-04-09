// Supplemental enhancement information (SEI) NAL units have a
// payload type field to indicate how they are to be
// interpreted. CEAS-708 caption content is always transmitted with
// payload type 0x04.
const USER_DATA_REGISTERED_ITU_T_T35 = 4;
const RBSP_TRAILING_BITS = 128;

export interface ParsedSei {
  payloadType: number;
  payloadSize: number;
  payload: Uint8Array | null;
}

/**
 * Parse a supplemental enhancement information (SEI) NAL unit.
 * Stops parsing once a message of type ITU T T35 has been found.
 *
 * @param {Uint8Array} bytes - The bytes of a SEI NAL unit
 * @return {Object} - The parsed SEI payload
 * @see Rec. ITU-T H.264, 7.3.2.3.1
 */
function parseSei(bytes: Uint8Array): ParsedSei {
  const result: ParsedSei = {
    payloadType: -1,
    payloadSize: 0,
    payload: null,
  };
  let payloadType = 0;
  let payloadSize = 0;

  let i = 0;
  // go through the sei_rbsp parsing each each individual sei_message
  while (i < bytes.byteLength) {
    // stop once we have hit the end of the sei_rbsp
    if (bytes[i] === RBSP_TRAILING_BITS) {
      break;
    }

    // Parse payload type
    while (bytes[i] === 0xff) {
      payloadType += 255;
      i++;
    }
    payloadType += bytes[i++];

    // Parse payload size
    while (bytes[i] === 0xff) {
      payloadSize += 255;
      i++;
    }
    payloadSize += bytes[i++];

    // this sei_message is a 608/708 caption so save it and break
    // there can only ever be one caption message in a frame's sei
    if (
      result.payload === null &&
      payloadType === USER_DATA_REGISTERED_ITU_T_T35
    ) {
      const userIdentifier = String.fromCharCode(
        bytes[i + 3],
        bytes[i + 4],
        bytes[i + 5],
        bytes[i + 6]
      );

      if (userIdentifier === "GA94") {
        result.payloadType = payloadType;
        result.payloadSize = payloadSize;
        result.payload = bytes.subarray(i, i + payloadSize);
        break;
      } else {
        result.payload = null;
      }
    }

    // skip the payload and parse the next message
    i += payloadSize;
    payloadType = 0;
    payloadSize = 0;
  }

  return result;
}

// see ANSI/SCTE 128-1 (2013), section 8.1
function parseUserData(sei: ParsedSei): Uint8Array | null {
  if (sei.payload === null) {
    return null;
  }

  // itu_t_t35_contry_code must be 181 (United States) for
  // captions
  if (sei.payload[0] !== 181) {
    return null;
  }

  // itu_t_t35_provider_code should be 49 (ATSC) for captions
  if (((sei.payload[1] << 8) | sei.payload[2]) !== 49) {
    return null;
  }

  // the user_identifier should be "GA94" to indicate ATSC1 data
  if (
    String.fromCharCode(
      sei.payload[3],
      sei.payload[4],
      sei.payload[5],
      sei.payload[6]
    ) !== "GA94"
  ) {
    return null;
  }

  // finally, user_data_type_code should be 0x03 for caption data
  if (sei.payload[7] !== 0x03) {
    return null;
  }

  // return the user_data_type_structure and strip the trailing
  // marker bits
  return sei.payload.subarray(8, sei.payload.length - 1);
}

export interface CaptionPacket {
  type: number;
  pts: number;
  ccData: number;
}

// see CEA-708-D, section 4.4
function parseCaptionPackets(
  pts: number,
  userData: Uint8Array
): CaptionPacket[] {
  const results: CaptionPacket[] = [];

  // if this is just filler, return immediately
  if (!(userData[0] & 0x40)) {
    return results;
  }

  // parse out the cc_data_1 and cc_data_2 fields
  const count = userData[0] & 0x1f;
  for (let i = 0; i < count; i++) {
    const offset = i * 3;
    // capture cc data when cc_valid is 1
    if (userData[offset + 2] & 0x04) {
      const data = {
        type: userData[offset + 2] & 0x03,
        pts,
        ccData: (userData[offset + 3] << 8) | userData[offset + 4],
      };
      results.push(data);
    }
  }
  return results;
}

function discardEmulationPreventionBytes(data: Uint8Array): Uint8Array {
  const length = data.byteLength;
  const emulationPreventionBytesPositions: number[] = [];

  // Find all `Emulation Prevention Bytes`
  let i = 1;
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

export {
  parseSei,
  parseUserData,
  parseCaptionPackets,
  discardEmulationPreventionBytes,
  USER_DATA_REGISTERED_ITU_T_T35,
};
