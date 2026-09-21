import {
  compareRuns,
  getResultsForSample,
  getSamplePerScenarios,
} from "./statistics.mjs";
/** @typedef {import("./statistics.mjs").ScenarioResult} ScenarioResult */
/** @typedef {import("./statistics.mjs").Comparison} Comparison */
/** @typedef {import("./statistics.mjs").Samples} Samples */

/**
 * Take test results as outputed by performance tests and output a markdown
 * table listing them in hopefully a readable way.
 * @param {ScenarioResult[]} results
 * @returns {string}
 */
export function formatResultAsMarkdownTable(results) {
  if (results.length === 0) {
    return "";
  }
  const { testNames, meanResult, medianResult } = getResultColumns(results);
  const nameColumnInnerLength = Math.max(
    testNames.reduce((acc, t) => Math.max(acc, t.length), 0) + 2 /* margin */,
    " Name ".length,
  );
  const meanColumnInnerLength = Math.max(
    meanResult.reduce((acc, t) => Math.max(acc, t.length), 0) + 2 /* margin */,
    " Mean ".length,
  );
  const medianColumnInnerLength = Math.max(
    medianResult.reduce((acc, t) => Math.max(acc, t.length), 0) +
      2 /* margin */,
    " Median ".length,
  );

  let str;

  {
    // Table header
    const nameWhitespaceLength = (nameColumnInnerLength - "Name".length) / 2;
    const meanWhitespaceLength = (meanColumnInnerLength - "Mean".length) / 2;
    const medianWhitespaceLength =
      (medianColumnInnerLength - "Median".length) / 2;
    str =
      "|" +
      " ".repeat(Math.floor(nameWhitespaceLength)) +
      "Name" +
      " ".repeat(Math.ceil(nameWhitespaceLength)) +
      "|" +
      " ".repeat(Math.floor(meanWhitespaceLength)) +
      "Mean" +
      " ".repeat(Math.ceil(meanWhitespaceLength)) +
      "|" +
      " ".repeat(Math.floor(medianWhitespaceLength)) +
      "Median" +
      " ".repeat(Math.ceil(medianWhitespaceLength)) +
      "|\n" +
      "|" +
      "-".repeat(nameColumnInnerLength) +
      "|" +
      "-".repeat(meanColumnInnerLength) +
      "|" +
      "-".repeat(medianColumnInnerLength) +
      "|";
  }
  for (let i = 0; i < results.length; i++) {
    str += "\n";
    const nameWhitespaceLength =
      (nameColumnInnerLength - testNames[i].length) / 2;
    const meanWhitespaceLength =
      (meanColumnInnerLength - meanResult[i].length) / 2;
    const medianWhitespaceLength =
      (medianColumnInnerLength - medianResult[i].length) / 2;
    str +=
      "|" +
      " ".repeat(Math.floor(nameWhitespaceLength)) +
      testNames[i] +
      " ".repeat(Math.ceil(nameWhitespaceLength)) +
      "|" +
      " ".repeat(Math.floor(meanWhitespaceLength)) +
      meanResult[i] +
      " ".repeat(Math.ceil(meanWhitespaceLength)) +
      "|" +
      " ".repeat(Math.floor(medianWhitespaceLength)) +
      medianResult[i] +
      " ".repeat(Math.ceil(medianWhitespaceLength)) +
      "|";
  }
  return str;
}

/**
 * Format the given report object into an readable HTML string.
 * @param {{success: boolean, commitSha?: string, baseBranch?: string, firstRun: Comparison, secondRun: Comparison | null}} reportObj
 * @returns {string}
 */
export function formatHtmlReport(reportObj) {
  let str = "<div>\n";
  str += "  <p>\n";
  if (reportObj.success) {
    str += "    ✅ Automated performance checks have passed ";
  } else {
    str += "    ❌ Automated performance checks have failed ";
  }
  if (reportObj.commitSha && reportObj.baseBranch) {
    str += `on commit <code>${reportObj.commitSha}</code> with the base branch <code>${reportObj.baseBranch}</code>`;
  }
  str += ".\n";
  str += "  </p>\n";
  str += "  <details>\n";
  str += "    <summary>Details</summary>\n\n";
  str += "<h2>Performance tests 1st run output</h2>\n";

  const { firstRun, secondRun } = reportObj;
  if (firstRun.worse.length > 0) {
    str += "\n<p>Median performance regressions (CI blocking):</p>\n\n";
    str += formatResultAsHtmlTable(firstRun.worse);
  }

  if (firstRun.meanOnlyWorse.length > 0) {
    str += "\n<p>Mean-only performance regressions (warning):</p>\n\n";
    str += formatResultAsHtmlTable(firstRun.meanOnlyWorse);
  }

  if (firstRun.better.length > 0) {
    str += "\n<p>Better performance for tests:</p>\n\n";
    str += formatResultAsHtmlTable(firstRun.better);
  }

  if (firstRun.notSignificative.length > 0) {
    str += "\n<p>No significative change in performance for tests:</p>\n\n";
    str += formatResultAsHtmlTable(firstRun.notSignificative);
  }
  str += "\n";

  if (secondRun) {
    str += "\n";
    str += "<h2>Performance tests 2nd run output</h2>\n";
    if (secondRun.worse.length > 0) {
      str += "\n<p>Median performance regressions (CI blocking):</p>\n\n";
      str += formatResultAsHtmlTable(secondRun.worse);
    }

    if (secondRun.meanOnlyWorse.length > 0) {
      str += "\n<p>Mean-only performance regressions (warning):</p>\n\n";
      str += formatResultAsHtmlTable(secondRun.meanOnlyWorse);
    }

    if (secondRun.better.length > 0) {
      str += "\n<p>Better performance for tests:</p>\n\n";
      str += formatResultAsHtmlTable(secondRun.better);
    }

    if (secondRun.notSignificative.length > 0) {
      str += "\n<p>No significative change in performance for tests:</p>\n\n";
      str += formatResultAsHtmlTable(secondRun.notSignificative);
    }
    str += "\n";
  }
  str += "  </details>\n";
  str += "</div>";
  return str;
}

/**
 * Take test results as outputed by performance tests and output an HTML
 * table listing them.
 * @param {ScenarioResult[]} results
 * @returns {string}
 */
function formatResultAsHtmlTable(results) {
  if (results.length === 0) {
    return "";
  }
  const { testNames, meanResult, medianResult } = getResultColumns(results);
  let str;
  str = '<table role="table">\n';
  str += "  <thead>\n";
  str += "    <tr>\n";
  str += "      <th>Name</th>\n";
  str += "      <th>Mean</th>\n";
  str += "      <th>Median</th>\n";
  str += "    </tr>\n";
  str += "  </thead>\n";
  str += "  <tbody>\n";

  for (let i = 0; i < results.length; i++) {
    str += "    <tr>\n";
    str += `      <td>${testNames[i]}</td>\n`;
    str += `      <td>${meanResult[i]}</td>\n`;
    str += `      <td>${medianResult[i]}</td>\n`;
    str += "    </tr>\n";
  }
  str += "  </tbody>\n";
  str += "</table>\n";
  return str;
}

/** @param {ScenarioResult[]} results */
function getResultColumns(results) {
  const testNames = results.map((r) =>
    r.regressionSignals === undefined
      ? r.testName
      : `${r.testName} (${r.regressionSignals})`,
  );
  const meanResult = results.map(
    (r) =>
      `${r.previousMean.toFixed(2)}ms -> ${r.currentMean.toFixed(2)}ms ` +
      `(corrected: ${r.meanDifferenceMs.toFixed(3)}ms, ` +
      `A/A bias: ${r.controlMeanDifferenceMs.toFixed(3)}ms, ` +
      `z: ${r.meanZScore.toFixed(5)})`,
  );
  const medianResult = results.map(
    (r) =>
      `${r.previousMedian.toFixed(2)}ms -> ${r.currentMedian.toFixed(2)}ms ` +
      `(corrected: ${r.medianDifferenceMs.toFixed(3)}ms, ` +
      `A/A bias: ${r.controlMedianDifferenceMs.toFixed(3)}ms, ` +
      `z: ${r.medianZScore.toFixed(5)})`,
  );

  return { testNames, meanResult, medianResult };
}

/** @param {Samples} samples
 * @param {Comparison} comparison */
export function logComparisonDiagnostics(samples, comparison) {
  const current = getSamplePerScenarios(
    samples.current.filter((sample) => sample.experiment === "treatment"),
  );
  const previous = getSamplePerScenarios(
    samples.previous.filter((sample) => sample.experiment === "treatment"),
  );
  const results = [
    ...comparison.worse,
    ...comparison.meanOnlyWorse,
    ...comparison.better,
    ...comparison.notSignificative,
  ];
  for (const testName of Object.keys(current)) {
    const result = results.find((result) => result.testName === testName);
    if (result === undefined) {
      throw new Error(`Missing comparison for ${testName}`);
    }
    const resultCurrent = getResultsForSample(current[testName]);
    const resultPrevious = getResultsForSample(previous[testName]);
    console.log("");
    console.log(`> Current results for test:`, testName);
    console.log("");
    console.log("    For current Player:");
    console.log(`      mean: ${resultCurrent.mean}`);
    console.log(`      median: ${resultCurrent.median}`);
    console.log(`      variance: ${resultCurrent.variance}`);
    console.log(`      standard deviation: ${resultCurrent.standardDeviation}`);
    console.log(
      `      standard error of mean: ${resultCurrent.standardErrorOfMean}`,
    );
    console.log(`      moe: ${resultCurrent.moe}`);
    console.log("");
    console.log("    For previous Player:");
    console.log(`      mean: ${resultPrevious.mean}`);
    console.log(`      median: ${resultPrevious.median}`);
    console.log(`      variance: ${resultPrevious.variance}`);
    console.log(
      `      standard deviation: ${resultPrevious.standardDeviation}`,
    );
    console.log(
      `      standard error of mean: ${resultPrevious.standardErrorOfMean}`,
    );
    console.log(`      moe: ${resultPrevious.moe}`);
    console.log("");
    console.log("    Results");
    console.log(
      `      A/A mean slot difference: ${result.controlMeanDifferenceMs} ms`,
    );
    console.log(
      `      A/B mean difference: ${result.meanDifferenceMs + result.controlMeanDifferenceMs} ms`,
    );
    console.log(
      `      bias-corrected mean difference (negative is slower): ${result.meanDifferenceMs} ms`,
    );
    console.log(`      Mean z-score: ${result.meanZScore}`);
    console.log(
      `      A/A median slot difference: ${result.controlMedianDifferenceMs} ms`,
    );
    console.log(
      `      A/B median difference: ${result.medianDifferenceMs + result.controlMedianDifferenceMs} ms`,
    );
    console.log(
      `      bias-corrected median difference (negative is slower): ${result.medianDifferenceMs} ms`,
    );
    console.log(`      Median z-score: ${result.medianZScore}`);

    console.log("");
  }
}

/** @param {Comparison} comparison */
export function logComparison(comparison) {
  logTable(
    "warn",
    "Median performance regressions (CI blocking):",
    comparison.worse,
  );
  logTable(
    "warn",
    "Mean-only performance regressions (warning):",
    comparison.meanOnlyWorse,
  );
  logTable("log", "Better performance for tests:", comparison.better);
  logTable(
    "log",
    "No significative change in performance for tests:",
    comparison.notSignificative,
  );
}

/** @param {Comparison} firstRun
 * @param {Comparison} secondRun */
export function logRetryComparison(firstRun, secondRun) {
  console.error(
    "\nFinal result after 2 attempts\n-----------------------------\n",
  );
  logTable(
    "warn",
    "Mean-only performance regressions on second attempt (warning):",
    secondRun.meanOnlyWorse,
  );
  const { confirmed, inconsistent } = compareRuns(firstRun, secondRun);
  logTable("error", "Worse performance at first attempt for tests:", confirmed);
  logTable(
    "warn",
    "Inconsistent results for tests (failed only one run):",
    inconsistent,
  );
}

/** @param {"log" | "warn" | "error"} level
 * @param {string} title
 * @param {ScenarioResult[]} results */
function logTable(level, title, results) {
  if (results.length > 0)
    console[level](
      "\n" + title + "\n\n" + formatResultAsMarkdownTable(results),
    );
}
