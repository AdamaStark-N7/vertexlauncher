//! Legal: the licences of the fonts and icons Vertex bundles, as an accordion: open a notice to read its full text.

use std::collections::BTreeSet;
use std::sync::{Mutex, OnceLock};

use vertex_shell::sturdy::ui::{
    AlignX, AlignY, ChildAlignment, Clip, CornerRadius, Cursor, Direction, ElementDesc, Padding,
    Sizing, TextStyle, TextWrap, Ui,
};

use super::super::icons::Icon;
use super::super::kit::{self, Kit, TextRole};

/// One notice as shown: the projects it covers and the licence text they share.
struct Notice {
    name: String,
    source: String,
    text: String,
}

/// Vertex's own bundled fonts and icons.
const BUNDLED: &[(&str, &str, &str)] = &[
    (
        "Tabler Icons (MIT)",
        "legal/TABLER_LICENSE.txt",
        include_str!("../../../../../launcher_ui/src/legal/TABLER_LICENSE.txt"),
    ),
    (
        "Maple Mono NF and Maple Mono NF CN (SIL OFL 1.1)",
        "legal/MAPLE_LICENSE.txt",
        include_str!("../../../../../launcher_ui/src/legal/MAPLE_LICENSE.txt"),
    ),
    (
        "Noto Color Emoji (SIL OFL 1.1)",
        "legal/NOTO_EMOJI_LICENSE.txt",
        include_str!("../../../../../launcher_ui/src/legal/NOTO_EMOJI_LICENSE.txt"),
    ),
];

/// Vertex's notices plus SturdyEngine5's own list of what it embeds, with projects that share the same licence text
/// folded into one entry so nothing is shown twice.
fn notices() -> &'static [Notice] {
    static NOTICES: OnceLock<Vec<Notice>> = OnceLock::new();
    NOTICES.get_or_init(|| {
        let mut notices: Vec<Notice> = Vec::new();
        let mut add = |name: String, source: String, text: String| {
            let key = normalize(&text);
            match notices
                .iter_mut()
                .find(|notice| normalize(&notice.text) == key)
            {
                Some(existing) => {
                    for part in name.split(", ") {
                        if !existing
                            .name
                            .split(", ")
                            .any(|known| known.eq_ignore_ascii_case(part))
                        {
                            existing.name.push_str(", ");
                            existing.name.push_str(part);
                        }
                    }
                }
                None => notices.push(Notice { name, source, text }),
            }
        };
        for (name, source, text) in BUNDLED {
            add((*name).to_owned(), (*source).to_owned(), (*text).to_owned());
        }
        for license in vertex_shell::sturdy::images::third_party_licenses() {
            add(license.project, license.license_file, license.text);
        }
        notices
    })
}

/// A licence text with whitespace runs collapsed, so copies that differ only in line wrapping count as the same.
fn normalize(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Which notices are open, by index; it survives leaving the screen.
static OPEN: Mutex<BTreeSet<usize>> = Mutex::new(BTreeSet::new());

pub fn render(ui: &mut Ui<'_>, kit: &mut Kit) {
    let _scroll = kit.scroll(ui, "legal::scroll", kit::SPACE_LG, kit::SPACE_XL);
    kit.text(ui, TextRole::Title, "Legal");
    {
        let _header = kit.card(ui, "legal::header", kit::SPACE_SM);
        kit.text(ui, TextRole::Heading, "Included Notices");
        kit.separator(ui);
        kit.text(
            ui,
            TextRole::Body,
            "Open a notice below to read its full legal text.",
        );
        kit.text(ui, TextRole::Small, &format!("{} total", notices().len()));
    }
    for (index, notice) in notices().iter().enumerate() {
        let open = OPEN.lock().is_ok_and(|open| open.contains(&index));
        let _card = kit.card(ui, &format!("legal::notice::{index}"), kit::SPACE_SM);
        let header_id = format!("legal::notice::{index}::header");
        {
            let hovered = ui.hovered(&header_id);
            let _header = ui.element(
                &ElementDesc::new()
                    .id(header_id.clone())
                    .width(Sizing::Grow)
                    .height(Sizing::Fixed(34.0))
                    .direction(Direction::LeftToRight)
                    .child_gap(kit::SPACE_SM)
                    .padding(Padding::symmetric(kit::SPACE_SM, 0))
                    .align(ChildAlignment::new(AlignX::Left, AlignY::Center))
                    .background(if hovered {
                        kit.palette.hovered_fill()
                    } else {
                        vertex_shell::sturdy::glam::Vec4::ZERO
                    })
                    .corner_radius(CornerRadius::all(8.0))
                    .cursor(Cursor::Pointer),
            );
            kit.icon(
                ui,
                if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                },
                16.0,
                kit.palette.text,
            );
            kit::label(
                ui,
                &notice.name,
                &TextStyle {
                    wrap: TextWrap::Words,
                    ..kit.style(TextRole::Strong)
                },
            );
        }
        if kit.clicked(ui, &header_id) {
            if let Ok(mut open) = OPEN.lock()
                && !open.remove(&index)
            {
                open.insert(index);
            }
        }
        if open {
            kit::label(
                ui,
                &notice.source,
                &TextStyle {
                    wrap: TextWrap::Words,
                    ..kit.style(TextRole::Small)
                },
            );
            kit.separator(ui);
            let text_id = format!("legal::notice::{index}::text");
            {
                let _text = ui.element(
                    &ElementDesc::new()
                        .id(text_id.clone())
                        .width(Sizing::Grow)
                        .height(Sizing::Fixed(
                            (ui.viewport_size().y * 0.45).clamp(180.0, 480.0),
                        ))
                        .direction(Direction::TopToBottom)
                        .padding(Padding::all(kit::SPACE_MD))
                        .background(kit.palette.bg_dark)
                        .corner_radius(CornerRadius::all(8.0))
                        .clip(Clip::vertical()),
                );
                kit.scrollbar(ui, &text_id);
                kit::label(
                    ui,
                    notice.text.trim(),
                    &TextStyle {
                        wrap: TextWrap::Words,
                        ..kit.style(TextRole::Small)
                    },
                );
            }
        }
    }
}
