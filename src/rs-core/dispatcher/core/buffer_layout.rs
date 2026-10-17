use crate::{
    bindings::MediaType, playlist_store::PlaylistStore, utils::codecs::guess_media_type_from_codec,
};
use std::collections::HashSet;

/// Restrict this content's selections to the SourceBuffer configuration startup will create.
/// This policy belongs here: PlaylistStore provides metadata and enforces the supplied choices,
/// while the dispatcher decides which buffering configurations it can play without a restart.
pub(super) fn restrict_to_startup_buffer_layout(store: &mut PlaylistStore) {
    let expected = [MediaType::Audio, MediaType::Video].map(|media_type| {
        store
            .current_codec(media_type)
            .as_deref()
            .map_or(0, codec_track_types)
    });
    let mut allowed = HashSet::new();
    let tracks =
        std::iter::once(None).chain(store.audio_tracks().iter().map(|track| Some(track.id())));
    for track in tracks {
        let variants = match track {
            Some(id) => store.variants_for_audio_track(id),
            None => store.all_variants().iter().collect(),
        };
        for variant in variants {
            let mut layout = [0; 2];
            let mut known = true;
            for (media_type, codec) in store.variant_media_codecs(variant, track) {
                let index = match media_type {
                    MediaType::Audio => 0,
                    MediaType::Video => 1,
                };
                layout[index] = match codec {
                    Some(codec) => codec_track_types(&codec),
                    // Infer an audio-only input from a distinct AUDIO rendition when its
                    // codecs are unknown. Primary playlists can contain both track types,
                    // so do not guess their configuration without codecs or probe results.
                    None if media_type == MediaType::Audio => 1,
                    None => {
                        known = false;
                        break;
                    }
                };
            }
            if known && layout == expected {
                allowed.insert((variant.id(), track));
            }
        }
    }
    store.restrict_selections(allowed);
}

/// Track presence, independent of codec/profile. Unknown codec types have a separate bit.
fn codec_track_types(codecs: &str) -> u8 {
    codecs.split(',').fold(0, |mask, codec| {
        mask | match guess_media_type_from_codec(codec.trim()) {
            Some(MediaType::Audio) => 1,
            Some(MediaType::Video) => 2,
            None => 4,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::restrict_to_startup_buffer_layout;
    use crate::{
        bindings::MediaType,
        parser::ExternalMediaInfo,
        playlist_store::{LockVariantResponse, PlaylistStore, SetAudioTrackResponse},
        utils::url::Url,
    };
    fn parse_url(url: &str) -> Url {
        Url::new(url.to_string())
    }
    #[test]
    fn fixed_buffer_layout_filters_variants_and_rejects_switches_in_both_directions() {
        for start_muxed in [false, true] {
            let mut store = PlaylistStore::try_new(
                br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="separate",NAME="English",DEFAULT=YES,URI="audio.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="embedded",NAME="English",DEFAULT=YES
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="separate"
separate.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="embedded"
embedded.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=3000,CODECS="avc1.4D401F,mp4a.40.2"
muxed.m3u8
"#,
                parse_url("https://example.com/master.m3u8"),
            )
            .unwrap();
            let ids: Vec<_> = store.available_variants().iter().map(|v| v.id()).collect();
            let selected = ids[usize::from(start_muxed)];
            let rejected = ids[usize::from(!start_muxed)];
            store.set_variant(selected);
            restrict_to_startup_buffer_layout(&mut store);
            let expected = if start_muxed {
                vec![ids[1], ids[2]]
            } else {
                vec![ids[0]]
            };
            assert_eq!(
                store
                    .available_variants()
                    .iter()
                    .map(|v| v.id())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                store
                    .compatible_variants()
                    .iter()
                    .map(|v| v.id())
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(matches!(
                store.lock_variant(selected),
                LockVariantResponse::VariantLocked { .. }
            ));
            assert!(matches!(
                store.lock_variant(rejected),
                LockVariantResponse::NoVariantWithId
            ));
            assert!(store.is_variant_locked());
            assert!(store.set_variant(rejected).is_empty());
            assert_eq!(store.current_variant_id(), Some(selected));
            store.set_audio_track(store.fixed_audio_track_id());
            assert_eq!(store.available_variants().len(), expected.len());
        }
    }

    #[test]
    fn fixed_buffer_layout_rejects_audio_tracks_without_mutating_selection() {
        for start_muxed in [false, true] {
            let mut store = PlaylistStore::try_new(
                br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="Separate",DEFAULT=YES,URI="audio.m3u8"
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="Embedded"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="audio"
main.m3u8
"#,
                parse_url("https://example.com/master.m3u8"),
            )
            .unwrap();
            let tracks: Vec<_> = ["Separate", "Embedded"]
                .iter()
                .map(|name| {
                    store
                        .audio_tracks()
                        .iter()
                        .find(|track| track.name() == *name)
                        .unwrap()
                        .id()
                })
                .collect();
            let selected = tracks[usize::from(start_muxed)];
            let rejected = tracks[usize::from(!start_muxed)];
            store.set_audio_track(Some(selected));
            let variant = store.available_variants()[0].id();
            store.set_variant(variant);
            restrict_to_startup_buffer_layout(&mut store);
            assert_eq!(
                store
                    .available_audio_tracks()
                    .iter()
                    .map(|track| track.id())
                    .collect::<Vec<_>>(),
                vec![selected]
            );

            assert!(matches!(
                store.set_audio_track(Some(rejected)),
                SetAudioTrackResponse::NoCompatibleVariant
            ));
            assert_eq!(store.fixed_audio_track_id(), Some(selected));

            assert_eq!(store.available_variants().len(), 1);
            if start_muxed {
                assert!(matches!(
                    store.set_audio_track(None),
                    SetAudioTrackResponse::NoCompatibleVariant
                ));
                assert_eq!(store.fixed_audio_track_id(), Some(selected));
            }
        }
    }

    #[test]
    fn fixed_layout_uses_probed_tracks_and_does_not_guess_unknown_candidates() {
        let mut store = PlaylistStore::try_new(
            br#"#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID="audio",NAME="English",DEFAULT=YES,URI="audio.m3u8"
#EXT-X-STREAM-INF:BANDWIDTH=1000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="audio"
main.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2000,CODECS="avc1.42E01E,mp4a.40.2",AUDIO="audio"
other.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=3000
unknown.m3u8
"#,
            parse_url("https://example.com/master.m3u8"),
        )
        .unwrap();
        let ids: Vec<_> = store.available_variants().iter().map(|v| v.id()).collect();
        store.set_variant(ids[0]);
        store.set_external_media_info(
            ExternalMediaInfo {
                mime_type: "video/mp4".into(),
                media_type: MediaType::Video,
                codec: "avc1.42E01E,mp4a.40.2".into(),
            },
            MediaType::Video,
        );
        restrict_to_startup_buffer_layout(&mut store);

        assert_eq!(
            store
                .available_variants()
                .iter()
                .map(|v| v.id())
                .collect::<Vec<_>>(),
            vec![ids[0]]
        );
    }
}
