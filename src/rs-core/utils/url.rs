use std::fmt::Display;

/// Abstraction allowing to help with the handling of URLs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    inner: String,
}

impl Url {
    pub fn new(url: String) -> Self {
        // TODO: HLS section 6.3.1 requires stopping playback if any playlist URI has an unsupported
        // scheme, including URIs never selected for loading.
        // Validate resolved playlist URIs against supported schemes and surface a fatal
        // UnsupportedScheme error instead of relying on fetch failures.
        Self { inner: url }
    }

    pub fn from_relative(base_url: &str, relative_url: Url) -> Self {
        if base_url.is_empty() {
            relative_url
        } else if relative_url.inner.starts_with('/') {
            let complete_url = match url_domain_name(base_url) {
                Some(base_domain) => format!("{}{}", base_domain, relative_url),
                None => {
                    if base_url.as_bytes()[base_url.len() - 1] == b'/' {
                        format!("{}{}", base_url, &relative_url.inner[1..])
                    } else {
                        format!("{}{}", base_url, relative_url)
                    }
                }
            };
            Url {
                inner: complete_url,
            }
        } else if base_url.as_bytes()[base_url.len() - 1] == b'/' {
            Url {
                inner: format!("{}{}", base_url, relative_url),
            }
        } else {
            Url {
                inner: format!("{}/{}", base_url, relative_url),
            }
        }
    }

    /// Remove _HLS_skip without rewriting other query parameters.
    /// Return None without allocating if it is not present.
    pub(crate) fn without_hls_skip(&self) -> Option<Self> {
        let (before_fragment, fragment) = self
            .inner
            .split_once('#')
            .map_or((self.inner.as_str(), None), |(url, fragment)| {
                (url, Some(fragment))
            });
        let (path, query) = before_fragment.split_once('?')?;
        if !query
            .split('&')
            .any(|param| param.split('=').next() == Some("_HLS_skip"))
        {
            return None;
        }
        let query = query
            .split('&')
            .filter(|param| param.split('=').next() != Some("_HLS_skip"))
            .collect::<Vec<_>>()
            .join("&");
        let mut url = path.to_owned();
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }
        if let Some(fragment) = fragment {
            url.push('#');
            url.push_str(fragment);
        }
        Some(Self::new(url))
    }

    pub fn take(self) -> String {
        self.inner
    }

    pub fn get_ref(&self) -> &str {
        self.inner.as_str()
    }

    pub fn domain_name(&self) -> Option<&str> {
        url_domain_name(&self.inner)
    }

    pub fn is_absolute(&self) -> bool {
        is_absolute_url(self.inner.as_bytes())
    }

    pub fn pathname(&self) -> &str {
        let hash_idx = self.inner.find('#');
        let parsed = match hash_idx {
            Some(idx) => &self.inner[0..idx],
            None => &self.inner,
        };
        let query_idx = parsed.find('?');
        let parsed = match query_idx {
            Some(idx) => &parsed[0..idx],
            None => parsed,
        };
        let last_slash = parsed.rfind('/');
        match last_slash {
            Some(idx) => &parsed[0..idx],
            None => parsed,
        }
    }

    pub fn filename(&self) -> &str {
        let hash_idx = self.inner.find('#');
        let parsed = match hash_idx {
            Some(idx) => &self.inner[0..idx],
            None => &self.inner,
        };
        let query_idx = parsed.find('?');
        let parsed = match query_idx {
            Some(idx) => &parsed[0..idx],
            None => parsed,
        };
        let last_slash = parsed.rfind('/');
        match last_slash {
            Some(idx) => &parsed[idx + 1..],
            None => parsed,
        }
    }

    pub fn extension(&self) -> &str {
        let filename = self.filename();
        let last_dot = filename.rfind('.');
        match last_dot {
            Some(idx) => &filename[idx + 1..],
            None => "",
        }
    }
}

impl Display for Url {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.get_ref())
    }
}

fn is_absolute_url(bytes: &[u8]) -> bool {
    let mut offset = 0;
    loop {
        if bytes.len() < offset + 1 {
            return false;
        }
        if (bytes[offset] >= b'A' && bytes[offset] <= b'Z')
            || (bytes[offset] >= b'a' && bytes[offset] <= b'z')
        {
            offset += 1;
            continue;
        } else if bytes[offset] == b':' {
            if offset == 0 {
                return false;
            }
            offset += 1;
            break;
        } else {
            break;
        }
    }

    if bytes.len() < offset + 2 {
        false
    } else {
        &bytes[offset..offset + 2] == b"//"
    }
}

fn url_domain_name(url: &str) -> Option<&str> {
    let bytes = url.as_bytes();
    if !is_absolute_url(bytes) {
        None
    } else {
        let first_slash_idx = url.find('/')?;
        if first_slash_idx == 0
            || first_slash_idx >= bytes.len() - 2
            || bytes[first_slash_idx + 1] != b'/'
        {
            None
        } else {
            match url[first_slash_idx + 2..].find('/') {
                Some(last_slash_idx) => Some(&url[0..(last_slash_idx + first_slash_idx + 2)]),
                None => Some(url),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Url;

    #[test]
    fn removing_hls_skip_preserves_other_query_bytes_and_fragment() {
        for url in [
            "https://e/p",
            "https://e/p?",
            "https://e/p?token=a%2Fb#_HLS_skip=YES",
        ] {
            assert!(Url::new(url.to_owned()).without_hls_skip().is_none());
        }
        for (input, expected) in [
            ("https://e/p?_HLS_skip=YES", "https://e/p"),
            (
                "https://e/p?_HLS_skip=v2&token=a%2Fb&x=1&_HLS_skip=YES&x=2#fragment",
                "https://e/p?token=a%2Fb&x=1&x=2#fragment",
            ),
            (
                "https://e/p?token=_HLS_skip%3DYES#_HLS_skip=YES",
                "https://e/p?token=_HLS_skip%3DYES#_HLS_skip=YES",
            ),
            ("https://e/p#fragment", "https://e/p#fragment"),
        ] {
            assert_eq!(
                Url::new(input.to_owned())
                    .without_hls_skip()
                    .unwrap_or_else(|| Url::new(input.to_owned()))
                    .get_ref(),
                expected
            );
        }
    }
}
