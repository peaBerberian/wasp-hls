const AUDIO_PROPERTIES = [
  "audioobjecttype",
  "channelcount",
  "samplerate",
  "samplingfrequencyindex",
  "samplesize",
] as const;

const VIDEO_PROPERTIES = [
  "width",
  "height",
  "profileIdc",
  "levelIdc",
  "profileCompatibility",
  "sarRatio",
];

export { AUDIO_PROPERTIES, VIDEO_PROPERTIES };
