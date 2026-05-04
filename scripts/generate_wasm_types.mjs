#!/usr/bin/env node
/**
 * # generate_wasm_types.mjs
 *
 * This file allows to generate the stable TypeScript enums exposed through the
 * repository from the current `wasm-bindgen` JavaScript output.
 *
 * You can either run it directly as a script (run
 * `node generate_wasm_types.mjs -h` to see the different options) or by
 * requiring/importing it as a node module.
 * If doing the latter you will obtain a function generating the corresponding
 * file, and throwing with an Error if the generated bindings no longer match
 * the expected shape.
 */

import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import ts from "typescript";
import { fileURLToPath, pathToFileURL } from "node:url";

const repoRoot = path.resolve(fileURLToPath(new URL("..", import.meta.url)));
const defaultInputPath = path.join(repoRoot, "tmp", "wasm-bindgen", "wasp_hls.js");
const defaultOutputPath = path.join(repoRoot, "src", "ts-common", "wasmTypes.ts");

const enumNames = [
  "AddSourceBufferErrorCode",
  "AttachMediaSourceErrorCode",
  "EndOfStreamErrorCode",
  "LogLevel",
  "MediaPlaylistParsingErrorCode",
  "MediaSourceDurationUpdateErrorCode",
  "MediaSourceReadyState",
  "MediaType",
  "MultivariantPlaylistParsingErrorCode",
  "OtherErrorCode",
  "PlaybackTickReason",
  "PlaylistNature",
  "PushedSegmentErrorCode",
  "RemoveBufferErrorCode",
  "RemoveMediaSourceErrorCode",
  "RequestErrorReason",
  "SegmentParsingErrorCode",
  "SourceBufferCreationErrorCode",
  "StartingPositionType",
  "TimerReason",
];

/**
 * Generate `wasmTypes.ts` from a `wasm-bindgen` JavaScript file.
 * @param {object} [options]
 * @param {string} [options.inputPath]
 * @param {string} [options.outputPath]
 */
export default function generateWasmTypes({
  inputPath = defaultInputPath,
  outputPath = defaultOutputPath,
} = {}) {
  const source = readFileSync(inputPath, "utf8");
  const sourceFile = ts.createSourceFile(
    inputPath,
    source,
    ts.ScriptTarget.Latest,
    true,
    ts.ScriptKind.JS,
  );
  const enums = new Map();

  for (const statement of sourceFile.statements) {
    if (!ts.isVariableStatement(statement)) {
      continue;
    }
    for (const declaration of statement.declarationList.declarations) {
      if (
        !ts.isIdentifier(declaration.name) ||
        !enumNames.includes(declaration.name.text) ||
        declaration.initializer === undefined ||
        !isObjectFreezeCall(declaration.initializer)
      ) {
        continue;
      }

      const objectLiteral = declaration.initializer.arguments[0];
      const members = [];

      for (const property of objectLiteral.properties) {
        if (!ts.isPropertyAssignment(property)) {
          continue;
        }
        const key = getPropertyName(property.name);
        if (key === undefined || /^\d+$/.test(key)) {
          continue;
        }
        if (!ts.isNumericLiteral(property.initializer)) {
          continue;
        }
        members.push({
          name: key,
          value: property.initializer.text,
        });
      }

      if (members.length === 0) {
        throw new Error(
          `Could not extract members for enum ${declaration.name.text}`,
        );
      }
      enums.set(declaration.name.text, members);
    }
  }

  for (const enumName of enumNames) {
    if (!enums.has(enumName)) {
      throw new Error(`Expected enum ${enumName} was not found in ${inputPath}`);
    }
  }

  const output = enumNames
    .map((enumName) => {
      const members = enums.get(enumName);
      const body = members
        .map((member) => `  ${member.name} = ${member.value},`)
        .join("\n");
      return `export enum ${enumName} {\n${body}\n}`;
    })
    .join("\n\n");

  writeFileSync(outputPath, `${output}\n`);
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  runFromCli(process.argv.slice(2));
}

/**
 * @param {string[]} args
 */
function runFromCli(args) {
  if (args[0] === "-h" || args[0] === "--help") {
    displayHelp();
    process.exit(0);
  }

  let inputPath = defaultInputPath;
  let outputPath = defaultOutputPath;

  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--input") {
      const value = args[++i];
      if (value === undefined) {
        throwCliError('missing value for "--input"');
      }
      inputPath = path.resolve(repoRoot, value);
    } else if (arg === "--output") {
      const value = args[++i];
      if (value === undefined) {
        throwCliError('missing value for "--output"');
      }
      outputPath = path.resolve(repoRoot, value);
    } else {
      throwCliError(`unknown option: "${arg}"`);
    }
  }

  try {
    generateWasmTypes({ inputPath, outputPath });
    console.log(`Generated ${path.relative(repoRoot, outputPath)}.`);
  } catch (error) {
    console.error("Wasm types generation failed.");
    if (error instanceof Error) {
      console.error(error.message);
    } else {
      console.error(error);
    }
    process.exit(1);
  }
}

/**
 * @param {ts.Expression} node
 * @returns {boolean}
 */
function isObjectFreezeCall(node) {
  return (
    ts.isCallExpression(node) &&
    ts.isPropertyAccessExpression(node.expression) &&
    ts.isIdentifier(node.expression.expression) &&
    node.expression.expression.text === "Object" &&
    node.expression.name.text === "freeze" &&
    node.arguments.length === 1 &&
    ts.isObjectLiteralExpression(node.arguments[0])
  );
}

/**
 * @param {ts.PropertyName} name
 * @returns {string | undefined}
 */
function getPropertyName(name) {
  if (ts.isIdentifier(name) || ts.isStringLiteral(name)) {
    return name.text;
  }
  return undefined;
}

function displayHelp() {
  console.log(`Usage: node scripts/generate_wasm_types.mjs [options]

Generate src/ts-common/wasmTypes.ts from wasm-bindgen's generated JavaScript.

Options:
  -h, --help        Show this help output
  --input <path>    Input wasm-bindgen JS file
  --output <path>   Output TypeScript file`);
}

/**
 * @param {string} message
 */
function throwCliError(message) {
  console.error(`ERROR: ${message}\n`);
  displayHelp();
  process.exit(1);
}
