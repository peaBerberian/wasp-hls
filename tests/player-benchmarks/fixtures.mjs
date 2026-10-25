import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join, resolve, sep } from "node:path";

const mime = {
  ".m3u8": "application/vnd.apple.mpegurl",
  ".mp4": "video/mp4",
  ".m4s": "video/mp4",
  ".ts": "video/mp2t",
};

/** Keep metadata omissions explicit, independent of the encoded media. */
export function manifestVariant(master, kind) {
  if (kind === "master") return master;
  if (kind === "no-codecs") return master.replace(/,CODECS="[^"]*"/g, "");
  throw new Error(`Unsupported manifest kind: ${kind}`);
}

/** Describe the packagers' simple VoD output, not arbitrary HLS syntax. */
export function describeMediaPlaylist(text) {
  const durations = [...text.matchAll(/^#EXTINF:([\d.]+)/gm)].map((m) =>
    Number(m[1]),
  );
  const urls = text
    .split(/\r?\n/)
    .filter((line) => line && !line.startsWith("#"));
  if (
    !durations.length ||
    durations.length !== urls.length ||
    !text.includes("#EXT-X-ENDLIST")
  )
    throw new Error("Invalid generated VoD playlist");
  return {
    segments: durations.length,
    uniqueUrls: new Set(urls).size,
    duration: durations.reduce((sum, duration) => sum + duration, 0),
    discontinuities: [...text.matchAll(/^#EXT-X-DISCONTINUITY$/gm)].length,
    hash: createHash("sha256").update(text).digest("hex"),
  };
}

/** Generate once, before any browser is started. Both formats use one encode. */
export async function prepareFixtures(directory, formats, large) {
  // GPAC prints its version to stderr and may exit 1 without a filter graph.
  const gpac = spawnSync("gpac", ["-version"], { encoding: "utf8" });
  if (gpac.error) throw gpac.error;
  const gpacVersion = `${gpac.stdout}\n${gpac.stderr}`.match(
    /gpac - GPAC[^\r\n]+/,
  )?.[0];
  if (!gpacVersion) throw new Error("Cannot determine GPAC version");
  const tools = {
    ffmpeg: execFileSync("ffmpeg", ["-version"], { encoding: "utf8" }).split(
      "\n",
    )[0],
    gpac: gpacVersion,
  };
  const recipe = {
    version: 1,
    duration: 60,
    segmentDuration: 2,
    size: "960x540",
    rate: 24,
    videoKbps: 1600,
    audioKbps: 128,
    tools,
  };
  const fingerprint = createHash("sha256")
    .update(JSON.stringify(recipe))
    .digest("hex");
  const base = join(directory, "generated", fingerprint);
  await mkdir(base, { recursive: true });
  const source = join(base, "source.mp4");
  await cached(join(base, "source-ready.json"), () => {
    execFileSync(
      "ffmpeg",
      [
        "-y",
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=960x540:rate=24",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=880:sample_rate=48000",
        "-t",
        "60",
        "-c:v",
        "libx264",
        "-preset",
        "superfast",
        "-threads",
        "2",
        "-b:v",
        "1600k",
        "-g",
        "48",
        "-keyint_min",
        "48",
        "-sc_threshold",
        "0",
        "-c:a",
        "aac",
        "-b:a",
        "128k",
        "-ac",
        "2",
        "-movflags",
        "+faststart",
        source,
      ],
      { stdio: "inherit" },
    );
  });
  const sourceHash = createHash("sha256")
    .update(await readFile(source))
    .digest("hex");
  const records = {};
  const memoryMedia = new Set();
  for (const format of formats) {
    const output = join(base, format);
    await mkdir(output, { recursive: true });
    await cached(join(output, "ready.json"), () => {
      execFileSync(
        "gpac",
        [
          "-i",
          source +
            (format === "mpegts" || format === "fmp4-muxed"
              ? ":#Representation=av"
              : ""),
          "-o",
          `${join(output, "master.m3u8")}:segdur=2:muxtype=${format === "mpegts" ? "ts" : "mp4"}:profile=live`,
        ],
        { stdio: ["ignore", "pipe", "pipe"] },
      );
    });
    const master = await readFile(join(output, "master.m3u8"), "utf8");
    if (!master.includes("CODECS="))
      throw new Error("Packager did not declare codecs");
    const children = [
      ...master.matchAll(/URI="([^"]+)"|^([^#\s][^\r\n]*)/gm),
    ].map((m) => m[1] ?? m[2]);
    if (!children.length)
      throw new Error("Packager produced no media playlists");
    if ([...master.matchAll(/^#EXT-X-STREAM-INF:/gm)].length !== 1)
      throw new Error("Benchmark content must have one representation");
    if (large) {
      const longOutput = join(output, "long");
      await mkdir(longOutput, { recursive: true });
      await cached(join(longOutput, "ready.json"), async () => {
        // Copy encoded samples, but generate distinct URLs and continuous timestamps.
        // This writes about 1.5 GB per format; never part of the default run.
        for (const child of children) {
          const audio =
            format === "fmp4" &&
            /#EXT-X-MEDIA:TYPE=AUDIO[^\n]*URI="([^"]+)"/.exec(master)?.[1] ===
              child;
          execFileSync(
            "ffmpeg",
            [
              "-y",
              "-hide_banner",
              "-loglevel",
              "error",
              "-stream_loop",
              "119",
              "-i",
              source,
              "-t",
              "7200",
              ...(format === "fmp4"
                ? ["-map", audio ? "0:a:0" : "0:v:0"]
                : ["-map", "0:v:0", "-map", "0:a:0"]),
              "-c",
              "copy",
              "-f",
              "hls",
              "-hls_time",
              "2",
              "-hls_playlist_type",
              "vod",
              "-hls_segment_type",
              format === "mpegts" ? "mpegts" : "fmp4",
              ...(format === "mpegts"
                ? []
                : ["-hls_fmp4_init_filename", `${child}.init.mp4`]),
              join(longOutput, child),
            ],
            { stdio: "inherit" },
          );
        }
        await writeFile(join(longOutput, "master.m3u8"), master);
      });
    }
    const media = {};
    for (const child of children) {
      media[child] = describeMediaPlaylist(
        await readFile(join(output, child), "utf8"),
      );
      for (const prefix of large ? ["", "long/"] : [""]) {
        const text = await readFile(join(output, prefix, child), "utf8");
        const init = /#EXT-X-MAP:URI="([^"]+)"/.exec(text)?.[1];
        if (init) memoryMedia.add(`${format}/${prefix}${init}`);
        for (const url of text
          .split(/\r?\n/)
          .filter((line) => line && !line.startsWith("#"))
          .slice(0, 10))
          memoryMedia.add(`${format}/${prefix}${url}`);
      }
      if (large) {
        const long = describeMediaPlaylist(
          await readFile(join(output, "long", child), "utf8"),
        );
        if (
          long.segments < 3500 ||
          long.uniqueUrls !== long.segments ||
          long.discontinuities ||
          Math.abs(long.duration - 7200) > 2
        )
          throw new Error(
            "Long fixture must have unique segments and a continuous two-hour timeline",
          );
        media[`long/${child}`] = long;
      }
    }
    records[format] = {
      fingerprint,
      sourceHash,
      recipe,
      master,
      children,
      media,
    };
  }
  return {
    metadata: records,
    /** Direct media is meaningful only for a multiplexed A/V playlist. */
    entry(format, kind, isLarge = false, memory = false) {
      const record = records[format];
      if (kind === "direct" && format === "fmp4")
        throw new Error(
          "Direct separate-track fMP4 would omit audio; use fmp4-muxed explicitly",
        );
      const entry =
        kind === "direct"
          ? record.children.at(-1)
          : kind === "no-codecs"
            ? "no-codecs.m3u8"
            : "master.m3u8";
      return `/media/${memory ? "bounded/" : ""}${format}/${isLarge ? "long/" : ""}${entry}`;
    },
    async serve(path, response) {
      if (!path.startsWith("/media/")) return false;
      let relative = decodeURIComponent(path.slice("/media/".length));
      const bounded = relative.startsWith("bounded/");
      if (bounded) relative = relative.slice("bounded/".length);
      const file = resolve(base, relative);
      if (!file.startsWith(base + sep)) throw new Error("Invalid fixture path");
      // Leave later media requests pending. Playlists stay intact (including
      // two-hour metadata), while every player retains the same first 20 s.
      if (bounded && !relative.endsWith(".m3u8") && !memoryMedia.has(relative))
        return true;
      const text = file.endsWith("/no-codecs.m3u8")
        ? manifestVariant(
            await readFile(join(file, "..", "master.m3u8"), "utf8"),
            "no-codecs",
          )
        : await readFile(file);
      response.setHeader(
        "Content-Type",
        Object.entries(mime).find(([ext]) => file.endsWith(ext))?.[1] ??
          "application/octet-stream",
      );
      response.setHeader("Cache-Control", "no-store");
      response.end(text);
      return true;
    },
  };
}

async function cached(marker, generate) {
  try {
    await readFile(marker);
    return;
  } catch (error) {
    if (error.code !== "ENOENT") throw error;
  }
  await generate();
  // Incomplete runs have no marker and are regenerated, not silently reused.
  await writeFile(marker, "{}\n");
}
