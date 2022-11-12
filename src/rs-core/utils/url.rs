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
        } else if base_url.as_bytes()[base_url.len() - 1] == b'/' {
            let complete_url = if relative_url.inner.starts_with('/') {
                format!("{}{}", base_url, &relative_url.inner[1..])
            } else {
                format!("{}{}", base_url, relative_url.inner)
            };
            Url { inner: complete_url }
        } else {
            let complete_url = if relative_url.inner.starts_with('/') {
                format!("{}{}", base_url, relative_url.inner)
            } else {
                format!("{}/{}", base_url, relative_url.inner)
            };
            Url { inner: complete_url }
        }
    }

    pub fn take(self) -> String {
        self.inner
    }

    pub fn get_ref(&self) -> &str {
        self.inner.as_str()
    }

    pub fn is_absolute(&self) -> bool {
        // No Regex, because why not
        let bytes = self.inner.as_bytes();
        let mut offset = 0;
        loop {
            if bytes.len() < offset + 1 {
                return false;
            }
            if (bytes[offset] >= b'A' && bytes[offset] <= b'Z') ||
                (bytes[offset] >= b'a' && bytes[offset] <= b'z')
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

        if bytes.len() < offset +  2 {
            false
        } else {
            &bytes[offset..offset + 2] == b"//"
        }
    }

    pub fn pathname(&self) -> &str {
        let last_slash = self.inner.rfind('/');
        match last_slash {
            Some(idx) => &self.inner[0..idx],
            None => self.inner.as_str(),
        }
    }

    pub fn filename(&self) -> &str {
        let last_slash = self.inner.rfind('/');
        match last_slash {
            Some(idx) => &self.inner[idx + 1..],
            None => self.inner.as_str(),
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
