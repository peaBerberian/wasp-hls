/** @typedef {"control" | "treatment"} Experiment */
/** @typedef {"current" | "previous"} Page */
/** @typedef {{name: string, value: number, experiment: Experiment, processIteration: number, attempt: number}} Sample */
/** @typedef {{testName: string, previousMean: number, currentMean: number, previousMedian: number, currentMedian: number, meanDifferenceMs: number, medianDifferenceMs: number, controlMeanDifferenceMs: number, controlMedianDifferenceMs: number, meanZScore: number, medianZScore: number, regressionSignals?: string}} ScenarioResult */
/** @typedef {{worse: ScenarioResult[], meanOnlyWorse: ScenarioResult[], better: ScenarioResult[], notSignificative: ScenarioResult[]}} Comparison */
/** @typedef {{mean: number[], median: number[]}} Differences */

/** @typedef {{experiment: Experiment, processIteration: number}} ProcessPlan */
/** @typedef {Record<Page, Sample[]>} Samples */

/**
 * Construct array from the given list which contains both the value and a added
 * `rank` property useful for the Mann–Whitney U test.
 * @param {Array.<number>} list
 * @returns {Array<{rank: number, value: number}>}
 */
function rankSamples(list) {
  list.sort((a, b) => a - b);
  const withRank = list.map(function (item, index) {
    return {
      rank: index + 1,
      value: item,
    };
  });

  for (let i = 0; i < withRank.length; ) {
    let count = 1;
    let total = withRank[i].rank;

    for (
      let j = 0;
      withRank[i + j + 1] !== undefined &&
      withRank[i + j].value === withRank[i + j + 1].value;
      j++
    ) {
      total += withRank[i + j + 1].rank;
      count++;
    }

    const rank = total / count;
    for (let k = 0; k < count; k++) {
      withRank[i + k].rank = rank;
    }

    i = i + count;
  }

  return withRank;
}

/**
 * Compare paired browser-process samples against the A/A control.
 * @param {Samples} allSamples
 * @param {ProcessPlan[]} processPlans
 * @returns {Comparison}
 */
export function compareSamples(allSamples, processPlans) {
  const scenarioNames = new Set();
  for (const sample of [...allSamples.current, ...allSamples.previous]) {
    if (!Number.isFinite(sample.value)) {
      throw new Error(`Invalid performance value for ${sample.name}`);
    }
    scenarioNames.add(sample.name);
  }
  if (scenarioNames.size === 0) {
    throw new Error("No performance measurements received.");
  }
  const samplesPerScenario = {
    current: getSamplePerScenarios(
      allSamples.current.filter((sample) => sample.experiment === "treatment"),
    ),
    previous: getSamplePerScenarios(
      allSamples.previous.filter((sample) => sample.experiment === "treatment"),
    ),
  };
  const treatmentDifferences = getProcessDifferences("treatment");
  const controlDifferences = getProcessDifferences("control");

  /** @type {Comparison} */
  const results = {
    worse: [],
    meanOnlyWorse: [],
    better: [],
    notSignificative: [],
  };
  for (const testName of Object.keys(samplesPerScenario.current)) {
    const sampleCurrent = samplesPerScenario.current[testName];
    const samplePrevious = samplesPerScenario.previous[testName];
    const treatmentSamples = treatmentDifferences[testName];
    const controlSamples = controlDifferences[testName];
    const resultCurrent = getResultsForSample(sampleCurrent);
    const resultPrevious = getResultsForSample(samplePrevious);
    const resultTreatmentMean = getResultsForSample(treatmentSamples.mean);
    const resultControlMean = getResultsForSample(controlSamples.mean);
    const resultTreatmentMedian = getResultsForSample(treatmentSamples.median);
    const resultControlMedian = getResultsForSample(controlSamples.median);

    const meanDiffMs = resultTreatmentMean.mean - resultControlMean.mean;
    const medianDiffMs =
      resultTreatmentMedian.median - resultControlMedian.median;
    const meanUValue = getUValueFromSamples(
      treatmentSamples.mean,
      controlSamples.mean,
    );
    const meanZScore = Math.abs(
      calculateZScore(
        meanUValue,
        treatmentSamples.mean.length,
        controlSamples.mean.length,
      ),
    );
    const medianUValue = getUValueFromSamples(
      treatmentSamples.median,
      controlSamples.median,
    );
    const medianZScore = Math.abs(
      calculateZScore(
        medianUValue,
        treatmentSamples.median.length,
        controlSamples.median.length,
      ),
    );
    const isMeanSignificant = meanZScore > 2.575829;
    const isMedianSignificant = medianZScore > 2.575829;
    const isMeanWorse = isMeanSignificant && meanDiffMs < -2;
    const isMedianWorse = isMedianSignificant && medianDiffMs < -2;
    const isMedianBetter = isMedianSignificant && medianDiffMs > 2;

    /** @type {ScenarioResult} */
    const result = {
      testName,
      previousMean: resultPrevious.mean,
      currentMean: resultCurrent.mean,
      previousMedian: resultPrevious.median,
      currentMedian: resultCurrent.median,
      meanDifferenceMs: meanDiffMs,
      medianDifferenceMs: medianDiffMs,
      controlMeanDifferenceMs: resultControlMean.mean,
      controlMedianDifferenceMs: resultControlMedian.median,
      meanZScore,
      medianZScore,
    };

    if (isMedianWorse) {
      result.regressionSignals = isMeanWorse ? "mean + median" : "median";
      results.worse.push(result);
    } else if (isMeanWorse) {
      result.regressionSignals = "mean only";
      results.meanOnlyWorse.push(result);
    } else if (isMedianBetter) {
      results.better.push(result);
    } else {
      results.notSignificative.push(result);
    }
  }
  return results;
  /** @param {number} u
   * @param {number} len1
   * @param {number} len2 */
  function calculateZScore(u, len1, len2) {
    return (
      (u - (len1 * len2) / 2) /
      Math.sqrt((len1 * len2 * (len1 + len2 + 1)) / 12)
    );
  }

  /**
   * Return one previous-minus-current mean and median difference per browser process
   * and scenario.
   * @param {"control"|"treatment"} experiment
   * @returns {Object.<string, {mean: Array.<number>, median: Array.<number>}>}
   */
  function getProcessDifferences(experiment) {
    /** @type {Map<string, {name: string, current: number[], previous: number[]}>} */
    const valuesPerProcess = new Map();
    for (const page of /** @type {Page[]} */ (["current", "previous"])) {
      for (const sample of allSamples[page]) {
        if (sample.experiment !== experiment) {
          continue;
        }
        const key = `${sample.processIteration}:${sample.name}`;
        let values = valuesPerProcess.get(key);
        if (values === undefined) {
          values = { name: sample.name, current: [], previous: [] };
          valuesPerProcess.set(key, values);
        }
        values[page].push(sample.value);
      }
    }

    /** @type {Record<string, Differences>} */
    const differences = {};
    for (const plan of processPlans.filter(
      (plan) => plan.experiment === experiment,
    )) {
      for (const name of scenarioNames) {
        const { current = [], previous = [] } =
          valuesPerProcess.get(`${plan.processIteration}:${name}`) ?? {};
        if (current.length === 0 || previous.length === 0) {
          throw new Error(
            `Missing paired ${experiment} results for ${name} in process ${plan.processIteration}`,
          );
        }
        const currentMean = getResultsForSample(current).mean;
        const previousMean = getResultsForSample(previous).mean;
        if (differences[name] === undefined) {
          differences[name] = { mean: [], median: [] };
        }
        const currentMedian = getResultsForSample(current).median;
        const previousMedian = getResultsForSample(previous).median;
        differences[name].mean.push(previousMean - currentMean);
        differences[name].median.push(previousMedian - currentMedian);
      }
    }
    return differences;
  }
}

/**
 * Calculate U value from the Mann–Whitney U test from two samples.
 * @param {Array.<number>} sampleCurrent
 * @param {Array.<number>} samplePrevious
 * @returns {number}
 */
function getUValueFromSamples(sampleCurrent, samplePrevious) {
  const concatSamples = sampleCurrent.concat(samplePrevious);
  const ranked = rankSamples(concatSamples);

  const summedRanks1 = sumRanks(ranked, sampleCurrent);
  const summedRanks2 = sumRanks(ranked, samplePrevious);
  const n1 = sampleCurrent.length;
  const n2 = samplePrevious.length;

  const u1 = calculateUValue(summedRanks1, n1, n2);
  const u2 = calculateUValue(summedRanks2, n2, n1);

  /** @param {number} rank
   * @param {number} currLen
   * @param {number} otherLen */
  function calculateUValue(rank, currLen, otherLen) {
    return currLen * otherLen + (currLen * (currLen + 1)) / 2 - rank;
  }
  return Math.min(u1, u2);

  /** @param {Array<{rank: number, value: number}>} rankedList
   * @param {number[]} observations */
  function sumRanks(rankedList, observations) {
    const remainingToFind = observations.slice();
    let rank = 0;
    rankedList.forEach(function (observation) {
      const index = remainingToFind.indexOf(observation.value);
      if (index > -1) {
        rank += observation.rank;
        remainingToFind.splice(index, 1);
      }
    });
    return rank;
  }
}

/**
 * Construct a "result object" from the given sample.
 * That object will contain various useful information like the mean,
 * standard deviation, and so on.
 * @param {Array.<number>} sample
 */
export function getResultsForSample(sample) {
  sample = [...sample].sort((a, b) => a - b);
  let median;
  if (sample.length === 0) {
    median = 0;
  } else {
    median =
      sample.length % 2 === 0
        ? (sample[sample.length / 2 - 1] + sample[sample.length / 2]) / 2
        : sample[Math.floor(sample.length / 2)];
  }
  const mean = sample.reduce((acc, x) => acc + x, 0) / sample.length;
  const variance =
    sample.reduce((acc, x) => {
      return acc + Math.pow(x - mean, 2);
    }, 0) /
      (sample.length - 1) || 0;
  const standardDeviation = Math.sqrt(variance);
  const standardErrorOfMean = standardDeviation / Math.sqrt(sample.length);
  const criticalVal = 1.96;
  const moe = standardErrorOfMean * criticalVal;
  return {
    mean,
    median,
    variance,
    standardErrorOfMean,
    standardDeviation,
    moe,
  };
}

/**
 * Transform the sample object given to divide sample numbers per scenario (the
 * `name` property).
 * In the returned object, keys will be the scenario's name and value will be
 * the array of results (in terms of number) for that scenario.
 * @param {Sample[]} samplesObj
 * @returns {Record<string, number[]>}
 */
export function getSamplePerScenarios(samplesObj) {
  return samplesObj.reduce((acc, x) => {
    if (acc[x.name] === undefined) {
      acc[x.name] = [x.value];
    } else {
      acc[x.name].push(x.value);
    }
    return acc;
  }, /** @type {Record<string, number[]>} */ ({}));
}

/**
 * A median regression blocks CI only when the same scenario fails both runs.
 * @param {Comparison} firstRun
 * @param {Comparison | null} secondRun
 */
export function compareRuns(firstRun, secondRun) {
  const firstFailures = new Map(
    firstRun.worse.map((result) => [result.testName, result]),
  );
  const secondFailures = new Map(
    (secondRun?.worse ?? []).map((result) => [result.testName, result]),
  );
  const confirmed = firstRun.worse.filter((result) =>
    secondFailures.has(result.testName),
  );
  const inconsistent = [
    ...firstRun.worse.filter((result) => !secondFailures.has(result.testName)),
    ...(secondRun?.worse ?? []).filter(
      (result) => !firstFailures.has(result.testName),
    ),
  ];
  return { confirmed, inconsistent, success: confirmed.length === 0 };
}
