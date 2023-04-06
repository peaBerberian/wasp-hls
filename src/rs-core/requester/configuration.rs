const DEFAULT_BACKOFF_BASE: f64 = 300.;
const DEFAULT_BACKOFF_MAX: f64 = 3000.;

/// Inner configuration on which the `Requester` relies.
/// Can be updated at any time through the `config_mut` `Requester`'s method.
pub(crate) struct RequesterConfiguration {
    /// Amount of times a failed segment request might be retried on errors that seem temporary: `1`
    /// meaning it will be retried once, `2` twice, `0` never retried etc.
    ///
    /// To set to `-1` for infinite retry.
    pub(crate) segment_request_max_retry: i32,

    /// Timeout, in milliseconds, used for segment requests.
    ///
    /// If that timeout is exceeded, the corresponding request will fail.
    ///
    /// To set to `-1` to disable.
    pub(crate) segment_request_timeout: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the server.
    /// That timeout then grows exponentially the more the request has to be retried (in the case it
    /// fails multiple time consecutively).
    ///
    /// This is roughly the initial delay, in milliseconds, the initial backoff for a retried
    /// segment request should be.
    pub(crate) segment_backoff_base: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the server.
    /// That timeout then grows exponentially the more the request has to be retried (in the case it
    /// fails multiple time consecutively).
    ///
    /// This is roughly the maximum delay, in milliseconds, the backoff delay for a retried segment
    /// request should be.
    pub(crate) segment_backoff_max: f64,

    /// Amount of times a failed Multivariant Playlist request might be retried on errors that seem
    /// temporary: `1` meaning it will be retried once, `2` twice, `0` never retried etc.
    ///
    /// To set to `-1` for infinite retry.
    pub(crate) multi_variant_playlist_max_retry: i32,

    /// Timeout, in milliseconds, used for Multivariant playlist requests.
    ///
    /// If that timeout is exceeded, the corresponding request will fail.
    ///
    /// To set to `-1` to disable.
    pub(crate) multi_variant_playlist_request_timeout: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the server.
    /// That timeout then grows exponentially the more the request has to be retried (in the case it
    /// fails multiple time consecutively).
    ///
    /// This is roughly the initial delay, in milliseconds, the initial backoff for a retried
    /// Multivariant Playlist request should be.
    pub(crate) multi_variant_playlist_backoff_base: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the server.
    /// That timeout then grows exponentially the more the request has to be retried (in the case it
    /// fails multiple time consecutively).
    ///
    /// This is roughly the maximum delay, in milliseconds, the backoff delay for a retried
    /// Multivariant Playlist request should be.
    pub(crate) multi_variant_playlist_backoff_max: f64,

    /// Amount of times a failed Media Playlist request might be retried on errors that seem
    /// temporary: `1` meaning it will be retried once, `2` twice, `0` never retried etc.
    ///
    /// To set to `-1` for infinite retry.
    pub(crate) media_playlist_max_retry: i32,

    /// Timeout, in milliseconds, used for Media playlist requests.
    ///
    /// If that timeout is exceeded, the corresponding request will fail.
    ///
    /// To set to `-1` to disable.
    pub(crate) media_playlist_request_timeout: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the server.
    /// That timeout then grows exponentially the more the request has to be retried (in the case it
    /// fails multiple time consecutively).
    ///
    /// This is roughly the initial delay, in milliseconds, the initial backoff for a retried Media
    /// Playlist request should be.
    pub(crate) media_playlist_backoff_base: f64,

    /// When a request is retried, a timeout is awaited to avoid overloading the server.
    /// That timeout then grows exponentially the more the request has to be retried (in the case it
    /// fails multiple time consecutively).
    ///
    /// This is roughly the maximum delay, in milliseconds, the backoff delay for a retried Media
    /// Playlist request should be.
    pub(crate) media_playlist_backoff_max: f64,
}

impl Default for RequesterConfiguration {
    fn default() -> Self {
        Self {
            segment_request_max_retry: 0,
            segment_request_timeout: 30000.,
            multi_variant_playlist_request_timeout: 10000.,
            media_playlist_max_retry: 0,
            media_playlist_request_timeout: 10000.,
            segment_backoff_base: DEFAULT_BACKOFF_BASE,
            segment_backoff_max: DEFAULT_BACKOFF_MAX,
            multi_variant_playlist_max_retry: 0,
            multi_variant_playlist_backoff_base: DEFAULT_BACKOFF_BASE,
            multi_variant_playlist_backoff_max: DEFAULT_BACKOFF_MAX,
            media_playlist_backoff_base: DEFAULT_BACKOFF_BASE,
            media_playlist_backoff_max: DEFAULT_BACKOFF_MAX,
        }
    }
}
