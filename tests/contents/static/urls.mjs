/* eslint-env node */

// Every URLs served by our server. For test purposes.

/**
 * @typedef StaticUrlItem
 * @property {string} url - Absolute URL to reach that resource. Resource may be
 * a playlist, a segment etc.
 * @property {path} path - Corresponding absolute path to the filesystem asset
 * behind it.
 * @property {string} contentType - Expected Content-type when serving that
 * resource.
 * @property {((buffer: ArrayBuffer) => ArrayBuffer)|undefined} [postProcess] - If
 * set, it takes the filesystem asset in ArrayBuffer form and give back the actual
 * resource to serve. This is often used e.g. to do storage-saving tricks useful
 * for tests like repeating the same fmp4 segment while just updating its tfdt.
 */

// TODO:
/** Array<StaticUrlItem> */
export default [];
