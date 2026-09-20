use super::{
    media_playlist::{MediaPlaylist, MediaPlaylistParsingError},
    multi_variant_playlist::{
        MediaPlaylistContext, MediaPlaylistPermanentId, MediaPlaylistUpdateError,
        MediaPlaylistUrlLocation, MultivariantPlaylist, MultivariantPlaylistParsingError,
    },
};
use crate::utils::{logger::*, url::Url};
use std::{fmt, io};

pub(crate) enum TopLevelPlaylist {
    Multivariant(MultivariantPlaylist),
    DirectMedia(DirectMediaPlaylist),
}

/// Top-level Media playlist, i.e. there's no Multivariant playlist to rely on,
/// only a media playlist.
pub(crate) struct DirectMediaPlaylist {
    /// Unique identifier for this Media playlist
    id: MediaPlaylistPermanentId,
    /// Url to that media playlist
    url: Url,
    /// The `MediaPlaylist` itself
    playlist: MediaPlaylist,
}

#[derive(Clone, Debug)]
pub(crate) struct ExternalMediaInfo {
    pub(crate) mime_type: String,
    pub(crate) media_type: crate::bindings::MediaType,
    pub(crate) codec: String,
}

impl TopLevelPlaylist {
    pub(crate) fn parse(data: &[u8], url: Url) -> Result<Self, TopLevelPlaylistParsingError> {
        match classify_playlist(data) {
            PlaylistKind::Multivariant => {
                log_info!("Parser: this is a multivariant playlist, parsing...");
                MultivariantPlaylist::parse(io::Cursor::new(data), url)
                    .map(Self::Multivariant)
                    .map_err(TopLevelPlaylistParsingError::Multivariant)
            }
            PlaylistKind::Media => {
                log_info!("Parser: this is a top-level media playlist, parsing...");
                DirectMediaPlaylist::parse(data, url)
                    .map(Self::DirectMedia)
                    .map_err(TopLevelPlaylistParsingError::Media)
            }
            PlaylistKind::NotAPlaylist => Err(TopLevelPlaylistParsingError::NotAPlaylist),
        }
    }

    pub(crate) fn url(&self) -> &Url {
        match self {
            Self::Multivariant(playlist) => playlist.url(),
            Self::DirectMedia(playlist) => playlist.url(),
        }
    }
}

impl DirectMediaPlaylist {
    pub(crate) fn parse(data: &[u8], url: Url) -> Result<Self, MediaPlaylistParsingError> {
        let playlist = MediaPlaylist::create(
            io::Cursor::new(data),
            url.clone(),
            None,
            None,
            &MediaPlaylistContext::default(),
        )?;
        Ok(Self {
            id: MediaPlaylistPermanentId::new(MediaPlaylistUrlLocation::Direct, 0),
            url,
            playlist,
        })
    }

    pub(crate) fn id(&self) -> &MediaPlaylistPermanentId {
        &self.id
    }

    pub(crate) fn url(&self) -> &Url {
        &self.url
    }

    pub(crate) fn playlist(&self) -> &MediaPlaylist {
        &self.playlist
    }

    pub(crate) fn playlist_mut(&mut self) -> &mut MediaPlaylist {
        &mut self.playlist
    }

    pub(crate) fn external_media_info(&self) -> Option<&ExternalMediaInfo> {
        self.playlist.external_media_info()
    }

    pub(crate) fn set_external_media_info(&mut self, external_media_info: ExternalMediaInfo) {
        self.playlist.set_external_media_info(external_media_info);
    }

    pub(crate) fn media_playlist_url(&self, wanted_id: &MediaPlaylistPermanentId) -> Option<&Url> {
        if wanted_id == &self.id {
            Some(&self.url)
        } else {
            None
        }
    }

    pub(crate) fn media_playlist(
        &self,
        wanted_id: &MediaPlaylistPermanentId,
    ) -> Option<&MediaPlaylist> {
        if wanted_id == &self.id {
            Some(&self.playlist)
        } else {
            None
        }
    }

    pub(crate) fn update_media_playlist(
        &mut self,
        id: &MediaPlaylistPermanentId,
        media_playlist_data: impl io::BufRead,
        url: Url,
        _sync_playlist_id: Option<MediaPlaylistPermanentId>,
    ) -> Result<&MediaPlaylist, MediaPlaylistUpdateError> {
        if id != &self.id {
            return Err(MediaPlaylistUpdateError::NotFound);
        }
        let updated = MediaPlaylist::create(
            media_playlist_data,
            url.clone(),
            Some(&self.playlist),
            None,
            &MediaPlaylistContext::default(),
        )?;
        self.url = url;
        self.playlist = updated;
        Ok(&self.playlist)
    }
}

#[derive(Debug)]
pub(crate) enum TopLevelPlaylistParsingError {
    Multivariant(MultivariantPlaylistParsingError),
    Media(MediaPlaylistParsingError),
    NotAPlaylist,
}

impl fmt::Display for TopLevelPlaylistParsingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Multivariant(err) => err.fmt(f),
            Self::Media(err) => err.fmt(f),
            Self::NotAPlaylist => {
                write!(f, "The loaded resource does not seem to be an HLS playlist")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PlaylistKind {
    Multivariant,
    Media,
    NotAPlaylist,
}

pub(super) fn tag_name(line: &str) -> Option<&str> {
    let stripped = line.strip_prefix("#EXT")?;
    let colon_idx = match stripped.find(':') {
        None => line.len(),
        Some(idx) => idx + 4,
    };
    Some(&line[4..colon_idx])
}

pub(super) fn is_multivariant_playlist_tag_name(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "-X-MEDIA"
            | "-X-STREAM-INF"
            | "-X-I-FRAME-STREAM-INF"
            | "-X-SESSION-DATA"
            | "-X-SESSION-KEY"
            | "-X-CONTENT-STEERING"
    )
}

pub(super) fn is_media_playlist_tag_name(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "-X-TARGETDURATION"
            | "-X-MEDIA-SEQUENCE"
            | "-X-DISCONTINUITY-SEQUENCE"
            | "-X-ENDLIST"
            | "-X-PLAYLIST-TYPE"
            | "-X-I-FRAMES-ONLY"
            | "-X-PART-INF"
            | "-X-SERVER-CONTROL"
    )
}

pub(super) fn is_media_segment_tag_name(tag_name: &str) -> bool {
    matches!(
        tag_name,
        "INF"
            | "-X-BYTERANGE"
            | "-X-DISCONTINUITY"
            | "-X-KEY"
            | "-X-MAP"
            | "-X-PROGRAM-DATE-TIME"
            | "-X-GAP"
            | "-X-BITRATE"
            | "-X-PART"
    )
}

pub(super) fn media_playlist_singleton_tag_name(tag_name: &str) -> Option<&'static str> {
    match tag_name {
        "-X-VERSION" => Some("-X-VERSION"),
        "-X-INDEPENDENT-SEGMENTS" => Some("-X-INDEPENDENT-SEGMENTS"),
        "-X-START" => Some("-X-START"),
        "-X-TARGETDURATION" => Some("-X-TARGETDURATION"),
        "-X-MEDIA-SEQUENCE" => Some("-X-MEDIA-SEQUENCE"),
        "-X-DISCONTINUITY-SEQUENCE" => Some("-X-DISCONTINUITY-SEQUENCE"),
        "-X-ENDLIST" => Some("-X-ENDLIST"),
        "-X-PLAYLIST-TYPE" => Some("-X-PLAYLIST-TYPE"),
        "-X-I-FRAMES-ONLY" => Some("-X-I-FRAMES-ONLY"),
        "-X-PART-INF" => Some("-X-PART-INF"),
        "-X-SERVER-CONTROL" => Some("-X-SERVER-CONTROL"),
        _ => None,
    }
}

pub(super) fn multivariant_playlist_singleton_tag_name(tag_name: &str) -> Option<&'static str> {
    match tag_name {
        "-X-VERSION" => Some("-X-VERSION"),
        "-X-INDEPENDENT-SEGMENTS" => Some("-X-INDEPENDENT-SEGMENTS"),
        "-X-START" => Some("-X-START"),
        _ => None,
    }
}

pub fn classify_playlist(data: &[u8]) -> PlaylistKind {
    let mut first_non_empty_line = true;

    for line in data.split(|b| *b == b'\n') {
        let decoded = String::from_utf8_lossy(line);
        let line = decoded.trim();

        if line.is_empty() {
            continue;
        }

        if first_non_empty_line {
            first_non_empty_line = false;
            if line != "#EXTM3U" {
                return PlaylistKind::NotAPlaylist;
            }
            continue;
        }

        if let Some(tag_name) = tag_name(line) {
            if is_multivariant_playlist_tag_name(tag_name) {
                return PlaylistKind::Multivariant;
            }

            if is_media_playlist_tag_name(tag_name) || is_media_segment_tag_name(tag_name) {
                return PlaylistKind::Media;
            }
        }
    }

    // Reached end of file without finding media playlist tags.
    // If #EXTM3U was present, assume Multivariant.
    if first_non_empty_line {
        PlaylistKind::NotAPlaylist
    } else {
        PlaylistKind::Multivariant
    }
}

#[cfg(test)]
mod tests {
    use super::{classify_playlist, PlaylistKind, TopLevelPlaylist, TopLevelPlaylistParsingError};
    use crate::{
        parser::{MediaPlaylistParsingError, MultivariantPlaylistParsingError},
        utils::url::Url,
    };

    #[test]
    fn classifies_non_playlist_input() {
        assert_eq!(
            classify_playlist(b"<html>not hls</html>"),
            PlaylistKind::NotAPlaylist
        );
    }

    #[test]
    fn classifies_media_playlist_input() {
        assert_eq!(
            classify_playlist(b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg.ts\n"),
            PlaylistKind::Media
        );
    }

    #[test]
    fn classifies_media_playlist_with_invalid_utf8_in_tag_value() {
        assert_eq!(
            classify_playlist(b"#EXTM3U\n#EXT-X-TARGETDURATION:4\xff\n#EXTINF:4,\nseg.ts\n"),
            PlaylistKind::Media
        );
    }

    #[test]
    fn classifies_multivariant_playlist_input_from_unparsed_tags() {
        assert_eq!(
            classify_playlist(b"#EXTM3U\n#EXT-X-SESSION-DATA:DATA-ID=\"x\",VALUE=\"y\"\n"),
            PlaylistKind::Multivariant
        );
    }

    #[test]
    fn rejects_mixed_tags_when_classified_as_media() {
        let err = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"a\",URI=\"a.m3u8\"\n#EXTINF:4,\nseg.ts\n",
            Url::new("https://example.com/media.m3u8".to_owned()),
        );

        assert!(matches!(
            err,
            Err(TopLevelPlaylistParsingError::Media(
                MediaPlaylistParsingError::ConflictingPlaylistTagTypes
            ))
        ));
    }

    #[test]
    fn rejects_mixed_tags_when_classified_as_multivariant() {
        let err = TopLevelPlaylist::parse(
            b"#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1000\nvideo.m3u8\n#EXT-X-TARGETDURATION:4\n",
            Url::new("https://example.com/master.m3u8".to_owned()),
        );

        assert!(matches!(
            err,
            Err(TopLevelPlaylistParsingError::Multivariant(
                MultivariantPlaylistParsingError::ConflictingPlaylistTagTypes
            ))
        ));
    }
}
