import { createServer } from "http";
import runChrome from "../../scripts/run_chrome.mjs";
import runFirefox from "../../scripts/run_firefox.mjs";
import launchStaticServer from "../../scripts/launch_static_server.mjs";
import { currentDirectory } from "./prepare.mjs";
import { compareSamples } from "./statistics.mjs";
import { logComparisonDiagnostics } from "./report.mjs";
import createContentServer from "../contents/server.mjs";
/** @typedef {{listeningPromise: Promise<void>, close: () => void}} TestServer */
/** @typedef {import("./statistics.mjs").Experiment} Experiment */
/** @typedef {import("./statistics.mjs").Page} Page */
/** @typedef {import("./statistics.mjs").Sample} Sample */
/** @typedef {import("./statistics.mjs").Samples} Samples */
/** @typedef {import("./statistics.mjs").ProcessPlan} ProcessPlan */
/** @typedef {import("./statistics.mjs").Comparison} Comparison */

/**
 * Initialize and start all tests on a browser..
 * @param {Object} params
 * @param {number} params.controlIterations
 * @param {number} params.treatmentIterations
 * @param {string} [params.browser="chrome"] - The browser to run the tests on.
 * "chrome" by default. Can be either "chrome" or "firefox".
 * @param {number} params.contentServerPort - The port through which test
 * contents are served.
 * @param {number} params.resultServerPort - The port through which test
 * results should be sent.
 * @param {number} params.testPagePort - The port through which the test page
 * is acceeded.
 * @returns {Promise<Comparison>}
 */
export function runPerformanceTests({
  browser = "chrome",
  contentServerPort,
  resultServerPort,
  testPagePort,
  controlIterations,
  treatmentIterations,
}) {
  /** @type {import("child_process").ChildProcess | undefined} */
  let currentBrowser;
  /** @type {Array<() => Promise<void>>} */
  const tasks = [];
  /** @type {ProcessPlan[]} */
  const processPlans = [];
  /** @type {Samples} */
  const allSamples = { current: [], previous: [] };
  return new Promise((resolve, reject) => {
    let isFinished = false;
    /** @type {TestServer | undefined} */
    let contentServer;
    /** @type {TestServer | undefined} */
    let resultServer;
    /** @type {TestServer | undefined} */
    let staticServer;

    const onFinished = () => {
      let results;
      try {
        results = compareSamples(allSamples, processPlans);
        logComparisonDiagnostics(allSamples, results);
      } catch (error) {
        onError(error);
        return;
      }
      isFinished = true;
      closeServers();
      closeBrowser();
      resolve(results);
    };
    /** @param {unknown} error */
    const onError = (error) => {
      isFinished = true;
      closeServers();
      closeBrowser();
      reject(error);
    };

    const closeServers = () => {
      contentServer?.close();
      contentServer = undefined;
      resultServer?.close();
      resultServer = undefined;
      staticServer?.close();
      staticServer = undefined;
    };

    initServers({
      contentServerPort,
      testPagePort,
      resultServerPort,
      onDone: () => {
        closeBrowser();
        startNextTaskOrFinish(onFinished).catch(onError);
      },
      onValue: (page, sample) => allSamples[page].push(sample),
      onError,
    })
      .then((servers) => {
        contentServer = servers.contentServer;
        resultServer = servers.resultServer;
        staticServer = servers.staticServer;
        if (isFinished) {
          closeServers();
          return;
        }
        return startAllTests({ browser, testPagePort, resultServerPort });
      })
      .catch(onError);
  });

  /**
   * Build the `tasks` array and start all tests on the given browser.
   * @param {Object} params
   * @param {string} params.browser - The web browser to run those tests on. Can
   * be either "chrome" or "firefox".
   * @param {number} params.resultServerPort - The port through which test
   * results should be sent.
   * @param {number} params.testPagePort - The port through which the test page
   * is acceeded.
   * @returns {Promise<void>}
   */
  async function startAllTests({ browser, testPagePort, resultServerPort }) {
    tasks.length = 0;
    processPlans.length = 0;
    /** @type {Array<{experiment: Experiment, startWithCurrent: boolean}>} */
    const iterations = [];
    /** @type {Array<[Experiment, number]>} */
    const experiments = [
      ["control", controlIterations],
      ["treatment", treatmentIterations],
    ];
    for (const [experiment, count] of experiments) {
      for (let i = 0; i < count; i++) {
        iterations.push({ experiment, startWithCurrent: i % 2 === 0 });
      }
    }
    for (let i = iterations.length - 1; i > 0; i--) {
      const randomIndex = Math.floor(Math.random() * (i + 1));
      [iterations[i], iterations[randomIndex]] = [
        iterations[randomIndex],
        iterations[i],
      ];
    }
    for (const [
      index,
      { experiment, startWithCurrent },
    ] of iterations.entries()) {
      processPlans.push({
        experiment,
        processIteration: index + 1,
      });
      tasks.push(() =>
        startIteration({
          browser,
          experiment,
          processIteration: index + 1,
          startWithCurrent,
          iteration: index + 1,
          total: iterations.length,
          testPagePort,
          resultServerPort,
        }),
      );
    }
    const firstTask = tasks.shift();
    if (firstTask === undefined) {
      throw new Error("No task scheduled");
    }
    return firstTask();
  }

  /**
   * Free all resources and terminate script.
   */
  function closeBrowser() {
    if (currentBrowser !== undefined) {
      currentBrowser.kill("SIGKILL");
      currentBrowser = undefined;
    }
  }

  /**
   * Starts the next function in the `tasks` array.
   * If no task are available anymore, call the `onFinished` callback.
   * @param {() => void} onFinished
   */
  function startNextTaskOrFinish(onFinished) {
    const nextTask = tasks.shift();
    if (nextTask === undefined) {
      onFinished();
      return Promise.resolve();
    }
    return nextTask();
  }

  /**
   * @param {{browser: string, experiment: Experiment, processIteration: number, startWithCurrent: boolean, iteration: number, total: number, testPagePort: number, resultServerPort: number}} options
   * @returns {Promise<void>}
   */
  async function startIteration({
    browser,
    experiment,
    processIteration,
    startWithCurrent,
    iteration,
    total,
    testPagePort,
    resultServerPort,
  }) {
    if (currentBrowser !== undefined) {
      currentBrowser.kill("SIGKILL");
    }
    const pagePrefix = experiment === "control" ? "control-" : "";
    const page = startWithCurrent ? "current" : "previous";
    const url =
      `http://localhost:${testPagePort}/${pagePrefix}${page}.html` +
      `#p=${resultServerPort};e=${experiment};o=${processIteration};`;
    if (browser === "firefox") {
      // eslint-disable-next-line no-console
      console.log(`Running tests on Firefox (${iteration}/${total})`);
      currentBrowser = await runFirefox(url, {
        headless: true,
        enableAutoPlay: true,
      }).catch((err) => {
        throw new Error("Could not launch page on Firefox: " + String(err));
      });
    } else {
      // eslint-disable-next-line no-console
      console.log(`Running tests on Chrome (${iteration}/${total})`);
      currentBrowser = await runChrome(url, {
        headless: true,
        enableAutoPlay: true,
      }).catch((err) => {
        throw new Error("Could not launch page on Chrome: " + String(err));
      });
    }
  }
}

/**
 * Initialize all servers used for the performance tests.
 * @param {Object} params
 * @param {number} params.contentServerPort - The port through which test
 * contents are served.
 * @param {number} params.resultServerPort - The port through which test
 * results should be sent.
 * @param {number} params.testPagePort - The port through which the test page
 * is acceeded.
 * @param {() => void} params.onDone
 * @param {(page: Page, sample: Sample) => void} params.onValue
 * @param {(error: unknown) => void} params.onError
 * @returns {Promise<{contentServer: TestServer, resultServer: TestServer, staticServer: TestServer}>} - Resolves when all servers are listening.
 */
async function initServers({
  contentServerPort,
  testPagePort,
  resultServerPort,
  onDone,
  onValue,
  onError,
}) {
  let contentServer;
  let staticServer;
  let resultServer;
  try {
    contentServer = createContentServer({ port: contentServerPort });
    staticServer = launchStaticServer(currentDirectory, {
      httpPort: testPagePort,
    });
    resultServer = createResultServer({
      port: resultServerPort,
      onDone,
      onValue,
      onError,
    });
    await Promise.all([
      contentServer.listeningPromise,
      staticServer.listeningPromise,
      resultServer.listeningPromise,
    ]);
    return { contentServer, resultServer, staticServer };
  } catch (error) {
    contentServer?.close();
    staticServer?.close();
    resultServer?.close();
    throw error;
  }
}

/**
 * Create HTTP server which will receive test results and react appropriately.
 * @param {Object} params
 * @param {number} params.port
 * @param {() => void} params.onDone
 * @param {(page: Page, sample: Sample) => void} params.onValue
 * @param {(error: unknown) => void} params.onError
 * @returns {TestServer}
 */
function createResultServer({ port, onDone, onValue, onError }) {
  const server = createServer(onRequest);
  return {
    listeningPromise: new Promise(
      /** @param {(value?: void) => void} res */ (res) => {
        server.listen(port, function () {
          res();
        });
      },
    ),
    close() {
      server.close();
    },
  };

  /** @param {import("http").IncomingMessage} request
   * @param {import("http").ServerResponse} response */
  function onRequest(request, response) {
    if (request.method === "OPTIONS") {
      answerWithCORS(response, 200);
      response.end();
    } else if (request.method == "POST") {
      let body = "";
      request.on(
        "data",
        /** @param {Buffer} data */ function (data) {
          body += data;
        },
      );
      request.on("end", function () {
        try {
          const parsedBody = JSON.parse(body);
          if (parsedBody.type === "log") {
            // eslint-disable-next-line no-console
            console.warn("LOG:", parsedBody.data);
          } else if (parsedBody.type === "error") {
            answerWithCORS(response, 200, "OK");
            onError(
              new Error("ERROR: A fatal error happened: " + parsedBody.data),
            );
            return;
          } else if (parsedBody.type === "done") {
            onDone();
          } else if (parsedBody.type === "value") {
            /** @type {Page} */
            let page;
            if (parsedBody.page === "current") {
              page = "current";
            } else if (parsedBody.page === "previous") {
              page = "previous";
            } else {
              onError(new Error("Unknown page: " + parsedBody.page));
              return;
            }
            onValue(page, parsedBody.data);
          }
          answerWithCORS(response, 200, "OK");
          return;
        } catch {
          answerWithCORS(response, 500, "Invalid data format.");
          return;
        }
      });
    }
  }

  /**
   * Add CORS headers, Content-Length, body, HTTP status and answer with the
   * Response Object given.
   * @param {import("http").ServerResponse} response
   * @param {number} status
   * @param {string | Buffer} [body]
   */
  function answerWithCORS(response, status, body) {
    if (Buffer.isBuffer(body)) {
      response.setHeader("Content-Length", body.byteLength);
    }
    response.writeHead(status, {
      "Access-Control-Allow-Origin": "*",
      "Access-Control-Allow-Headers": "*",
      "Access-Control-Allow-Credentials": "true",
      "Access-Control-Allow-Methods": "GET, OPTIONS",
    });
    if (body !== undefined) {
      response.end(body);
    } else {
      response.end();
    }
  }
}
