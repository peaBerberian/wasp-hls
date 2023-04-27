use std::fmt::Display;

/// Abstraction allowing to help with the handling of URLs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Url {
    inner: String,
}

impl Url {
    pub fn new(url: String) -> Self {
        Self { inner: url }
    }

    pub fn from_relative(base_url: &str, relative_url: Url) -> Self {
        if base_url.is_empty() {
            relative_url
        } else {
            if relative_url.inner.starts_with('/') {
                let complete_url = match url_domain_name(base_url) {
                    Some(base_domain) => format!("{}{}", base_domain, relative_url),
                    None => if base_url.as_bytes()[base_url.len() - 1] == b'/' {
                        format!("{}{}", base_url, &relative_url.inner[1..])
                    } else {
                        format!("{}{}", base_url, relative_url)
                    }
                };
                Url {
                    inner: complete_url,
                }
            } else if base_url.as_bytes()[base_url.len() - 1] == b'/' {
                Url { inner: format!("{}{}", base_url, relative_url) }
            } else {
                Url { inner: format!("{}/{}", base_url, relative_url) }
            }
        }
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
        if first_slash_idx == 0 || first_slash_idx >= bytes.len() - 2 || bytes[first_slash_idx + 1] != b'/' {
            None
        } else {
            match url[first_slash_idx + 2..].find('/') {
                Some(last_slash_idx) => Some(&url[0..(last_slash_idx + first_slash_idx + 2)]),
                None => Some(&url),
            }
        }
    }
}
