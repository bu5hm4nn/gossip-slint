use nostr_types::ContentSegment;
use slint::{ModelRc, SharedString, VecModel};

use super::*;
pub struct NoteData {}

impl NoteData {
    pub fn fill_ui_notedata(event: nostr_types::Event) -> UiNoteData {
        UiNoteData {
            id: event.id.as_bech32_string().into(),
            author: event.pubkey.as_bech32_string().into(),
            created_at: NoteData::format_created_at(&event.created_at),
            created_ago: NoteData::format_created_ago(&event.created_at),
            kind: NoteData::parse_event_kind(&event.kind),
            content: NoteData::parse_content(&event.content),
            raw_content: event.content.into(),
            tags: NoteData::parse_tags(event.tags),
            repost: UiRepostType::None,         // TODO parse reposts
            encryption: UiEncryptionType::None, // TODO parse encryption
            bookmarked: false,                  // TODO update bookmarked status
        }
    }

    pub fn format_created_at(created_at: &nostr_types::Unixtime) -> SharedString {
        if let Ok(stamp) = time::OffsetDateTime::from_unix_timestamp(created_at.0) {
            if let Ok(formatted) = stamp.format(&time::format_description::well_known::Rfc2822) {
                formatted.into()
            } else {
                "invalid".into()
            }
        } else {
            "invalid".into()
        }
    }

    pub fn format_created_ago(created_at: &nostr_types::Unixtime) -> SharedString {
        crate::date_ago::date_ago(*created_at).into()
    }

    pub fn parse_event_kind(kind: &nostr_types::EventKind) -> i32 {
        let u: u32 = From::from(*kind);
        return u as i32;
    }

    fn parse_tags(tags: Vec<nostr_types::TagV3>) -> slint::ModelRc<slint::ModelRc<SharedString>> {
        ModelRc::new(VecModel::from(
            tags.into_iter()
                .map(|tag| {
                    ModelRc::new(VecModel::from(
                        tag.into_inner()
                            .into_iter()
                            .map(|a| SharedString::from(a))
                            .collect::<Vec<SharedString>>(),
                    ))
                })
                .collect::<Vec<_>>(),
        ))
    }

    fn parse_content(content: &String) -> ModelRc<UiContentSegment> {
        let display_content = content.clone();
        let shattered_content = nostr_types::ShatteredContent::new(display_content);
        let mut ui_content: Vec<UiContentSegment> = Vec::new();
        for segment in &shattered_content.segments {
            let ui_segment = match segment {
                ContentSegment::Plain(span) => UiContentSegment {
                    content: shattered_content.slice(&span).unwrap().into(),
                    r#type: UiContentSegmentType::Plain,
                },
                ContentSegment::NostrUrl(_url) => UiContentSegment {
                    content: "TODO nostr-url".into(),
                    r#type: UiContentSegmentType::NostrUrl,
                },
                ContentSegment::Hyperlink(span) => UiContentSegment {
                    content: shattered_content.slice(&span).unwrap().into(),
                    r#type: UiContentSegmentType::Hyperlink,
                },
                ContentSegment::TagReference(_tag) => UiContentSegment {
                    content: "TODO tag".into(),
                    r#type: UiContentSegmentType::NostrUrl,
                },
            };
            ui_content.push(ui_segment);
        }
        ModelRc::new(VecModel::from(ui_content))
    }
}
