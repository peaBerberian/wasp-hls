import { resolve } from "node:path";

export const players = ["wasp", "hlsjs", "shaka"];
export const formats = ["fmp4", "mpegts", "fmp4-muxed"];
export const scenarios = [
  "startup",
  "seek",
  "reload",
  "playback",
  "pressure",
  "large-playlist",
];
export const help =
  "npm run test -- player-benchmarks [--player wasp|hlsjs|shaka] [--format fmp4|mpegts|fmp4-muxed] [--manifest master|direct|no-codecs] [--scenario startup|seek|reload|playback|pressure|large-playlist] [--iterations 5] [--diagnostic] [--report path]";

export function parseOptions(args, directory) {
  const options = {
    players: [],
    formats: [],
    scenarios: [],
    manifest: "master",
    iterations: 5,
    diagnostic: false,
    report: resolve(directory, "results/latest.json"),
  };
  const repeated = {
    "--player": "players",
    "--format": "formats",
    "--scenario": "scenarios",
  };
  for (let i = 0; i < args.length; i++) {
    const separator = args[i].indexOf("=");
    const flag = separator < 0 ? args[i] : args[i].slice(0, separator);
    const inline = separator < 0 ? undefined : args[i].slice(separator + 1);
    if (flag === "--diagnostic" && inline === undefined) {
      options.diagnostic = true;
      continue;
    }
    if (
      ![
        ...Object.keys(repeated),
        "--manifest",
        "--iterations",
        "--report",
      ].includes(flag)
    )
      throw new Error(`Unknown option ${flag}`);
    const value = inline ?? args[++i];
    if (!value || value.startsWith("--"))
      throw new Error(`Missing value for ${flag}`);
    if (flag in repeated) options[repeated[flag]].push(value);
    else if (flag === "--iterations") options.iterations = Number(value);
    else if (flag === "--manifest") options.manifest = value;
    else options.report = resolve(value);
  }
  for (const [key, allowed, defaults] of [
    ["players", players, players],
    ["formats", formats, ["fmp4", "mpegts"]],
    ["scenarios", scenarios, ["startup"]],
  ]) {
    if (!options[key].length) options[key] = defaults;
    options[key] = [...new Set(options[key])];
    for (const value of options[key])
      if (!allowed.includes(value)) throw new Error(`Invalid ${key}: ${value}`);
  }
  if (!["master", "direct", "no-codecs"].includes(options.manifest))
    throw new Error("Invalid manifest kind");
  if (options.manifest === "direct" && options.formats.includes("fmp4"))
    throw new Error(
      "Direct fMP4 would omit audio; select fmp4-muxed explicitly",
    );
  if (!Number.isInteger(options.iterations) || options.iterations < 1)
    throw new Error("iterations must be a positive integer");
  return options;
}

/** Interleave conditions within each player block, rotating both orders. */
export function* samplePlan(options) {
  for (const format of options.formats)
    for (let iteration = 0; iteration < options.iterations; iteration++)
      for (let p = 0; p < options.players.length; p++)
        for (let s = 0; s < options.scenarios.length; s++)
          yield {
            format,
            iteration: iteration + 1,
            player: options.players[(p + iteration) % options.players.length],
            scenario:
              options.scenarios[(s + iteration) % options.scenarios.length],
          };
}
