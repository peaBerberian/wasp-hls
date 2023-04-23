# Changelog

## 0.4.2 (Unreleased yet)

- Better detect mimetype from segment's extension by stripping query and fragment components from its URL
- Fix issue in GOP (group of pictures) creation code in the mpeg-ts to fmp4 transmuxer. The real impact on playback in unclear (none was noticed).

## 0.4.1 (2023-04-22)

### Bug fixes

- Fix HTTP Range requests (by prepending the forgotten `bytes=` string)

## 0.4.0 (2023-04-21)

(First public release)

### Features

- Add TypeScript declaration files for embedded wasm and worker files

## 0.3.0 (2023-04-21)

### Features

- Add embedded wasm and worker

## 0.2.0 (2023-04-21)

### Features

- Emit TypeScript declaration files and add more types

## 0.1.1 (2023-04-21)

### Bug fixes

- Fix-up export paths in the package published on npm

## 0.1.0 (2023-04-21)

Initial release
