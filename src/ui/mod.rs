//! Window layout: panels, overlays, keyboard shortcuts.

use std::sync::Arc;

pub mod artist;
pub mod collection;
pub(crate) mod devices;
mod dialogs;
pub(crate) mod friends;
pub mod game;
pub mod home;
mod keys;
pub mod library;
pub mod login;
mod lyrics;
pub mod new_releases;
pub mod player_bar;
pub mod playlist_map;
mod prerelease;
pub mod queue;
pub mod radio;
pub mod search;
pub mod settings;
pub mod show;
pub mod sidebar;
pub mod skin_cards;
mod soloist;
pub mod theme_editor;
pub mod topbar;
mod update;
pub mod widgets;
pub mod winamp;

use egui::{Align2, Color32, Context, CornerRadius, Frame, Id, Margin, Rect, Stroke, vec2};

use crate::api::models::pick_image;
use crate::app::App;
use crate::backend::AuthStatus;
use crate::model::{Action, Loadable, Page, ToastKind};
use crate::settings::{PanelEdge, PanelPlace, SidePanel, side_panel_places};
use crate::theme::{self, Icon};

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let ctx = &ctx;
    keys::handle(app, ctx);
    for path in winamp::dropped_skins(ctx) {
        app.actions.push(Action::InstallSkin(path));
    }
    let signed_in = app.is_connected() && app.user.is_some();
    let connecting = matches!(app.auth, AuthStatus::Connecting | AuthStatus::Starting)
        || (app.is_connected() && app.user.is_none());
    if !signed_in {
        player_bar::end_tint_session(ctx);
        login::show(app, ui, connecting);
        update::show(app, ctx);
        toasts(app, ctx, 20.0);
        caption_buttons(ui, &app.palette, app.locale);
        fastframe_titlebar::resize(ui);
        return;
    }
    let soloist_view = app.soloist_view();
    if soloist_view != crate::app::SoloistView::App {
        player_bar::end_tint_session(ctx);
        if soloist_view == crate::app::SoloistView::Loading {
            login::show(app, ui, true);
        } else {
            soloist::onboarding(app, ui);
        }
        toasts(app, ctx, 20.0);
        caption_buttons(ui, &app.palette, app.locale);
        fastframe_titlebar::resize(ui);
        return;
    }
    #[cfg(feature = "milkdrop")]
    if app.immersive() {
        if lyrics::immersive(app, ui) {
            player_bar::show(app, ui);
        }
        lyrics::immersive_click(app, ui);
        devices::popup(app, ctx);
        dialogs::show(app, ctx);
        toasts(app, ctx, theme::PLAYER_BAR_HEIGHT + 16.0);
        return;
    }
    if titlebar_spans_window(cfg!(target_os = "macos"), fastframe_titlebar::enabled()) {
        fastframe_titlebar::drag(
            ui,
            Rect::from_min_size(
                ui.max_rect().min,
                vec2(
                    ui.max_rect().width(),
                    theme::top_bar_height() + theme::titlebar_inset(ui.ctx()),
                ),
            ),
        );
    }
    player_bar::show(app, ui);
    if app.lyrics_fullscreen.is_some() {
        lyrics::fullscreen(app, ui);
    } else {
        if app.settings.sidebar_visible {
            sidebar::show(app, ui);
        }
        // Each edge's panels in their order, the first nearest the window's
        // edge; stacked, several stand side by side.
        for place in app.open_side_panels() {
            match place.panel {
                SidePanel::Queue => queue::side_panel(app, ui),
                SidePanel::Lyrics => lyrics::side_panel(app, ui),
                SidePanel::Friends => friends::side_panel(app, ui),
            }
        }
        side_panel_grips(app, ui);
        central(app, ui);
    }
    keep_room_for_panels(app, ctx);
    devices::popup(app, ctx);
    dialogs::show(app, ctx);
    update::show(app, ctx);
    widgets::drag_ghost(ctx, &app.palette, app.locale);
    toasts(app, ctx, theme::PLAYER_BAR_HEIGHT + 16.0);
    caption_buttons(ui, &app.palette, app.locale);
    fastframe_titlebar::resize(ui);
}

/// The main window's narrowest width with these panels open: their least
/// widths and the page's.
fn main_min_width(page: f32, sidebar: bool, side_panels: usize) -> f32 {
    let sidebar = if sidebar { SIDEBAR_MIN_WIDTH } else { 0.0 };
    let panels = side_panels as f32 * theme::SIDE_PANEL_MIN_WIDTH;
    (sidebar + panels + page).max(crate::window::MAIN_MIN_SIZE[0])
}

/// A side panel being dragged by its grip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct MovingPanel(SidePanel);

/// The egui panel for `panel`, at the edge of the window it stands at.
pub(crate) fn side_panel_at(app: &App, panel: SidePanel) -> egui::Panel {
    let edge = side_panel_places(&app.settings.side_panels)
        .into_iter()
        .find(|place| place.panel == panel)
        .map_or(PanelEdge::Right, |place| place.edge);
    match edge {
        PanelEdge::Left => egui::Panel::left(panel.id()),
        PanelEdge::Right => egui::Panel::right(panel.id()),
    }
}

/// Where a dragged side panel would land with the pointer at `x`: the edge
/// on the pointer's half of the window, its place among the `others` there
/// (counted from the window's edge), and the line where it would stand,
/// between `page`, the page's span, and the window's edge.
fn panel_landing(
    x: f32,
    window: Rect,
    page: Rect,
    others: &[(PanelPlace, Rect)],
) -> (PanelEdge, usize, f32) {
    let edge = if x < window.center().x {
        PanelEdge::Left
    } else {
        PanelEdge::Right
    };
    let there: Vec<Rect> = others
        .iter()
        .filter(|(place, _)| place.edge == edge)
        .map(|(_, rect)| *rect)
        .collect();
    let index = there
        .iter()
        .filter(|rect| match edge {
            PanelEdge::Left => rect.center().x < x,
            PanelEdge::Right => rect.center().x > x,
        })
        .count();
    let line = match (edge, there.get(index)) {
        (PanelEdge::Left, Some(rect)) => rect.left(),
        (PanelEdge::Right, Some(rect)) => rect.right(),
        (PanelEdge::Left, None) => page.left(),
        (PanelEdge::Right, None) => page.right(),
    };
    (edge, index, line)
}

/// A grip at the top of each open side panel. Drag it to the other half of
/// the window to move the panel to that edge, or along its own edge to
/// change the order; while it moves, a line marks where it would stand.
fn side_panel_grips(app: &mut App, ui: &mut egui::Ui) {
    use crate::i18n::gettext;
    let ctx = ui.ctx().clone();
    let window = ui.max_rect();
    let page = ui.available_rect_before_wrap();
    let open: Vec<(PanelPlace, Rect)> = app
        .open_side_panels()
        .into_iter()
        .filter_map(|place| {
            let rect = egui::PanelState::load(&ctx, Id::new(place.panel.id()))?.outer_rect;
            Some((place, rect))
        })
        .collect();
    let palette = app.palette;
    // Translators: The handle at the top of the Queue, Lyrics or Listening activity panel, dragged to move the panel to the other side of the window or beside another panel.
    let label = gettext(app.locale, "Move panel");
    for (place, rect) in &open {
        let grip = Rect::from_min_size(
            egui::pos2(rect.left() + 12.0, rect.top() + 1.0),
            vec2(GRIP_SIZE.x, GRIP_SIZE.y),
        );
        let id = Id::new(("side-panel-grip", place.panel.id()));
        let mut child = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(grip)
                .layer_id(egui::LayerId::new(egui::Order::Foreground, id)),
        );
        let response = child
            .dnd_drag_source(id, MovingPanel(place.panel), |ui| {
                let (rect, response) = ui.allocate_exact_size(GRIP_SIZE, egui::Sense::hover());
                let colour = if response.hovered() {
                    palette.text
                } else {
                    palette.dim
                };
                for row in [-2.0, 2.0] {
                    for column in [-6.0, 0.0, 6.0] {
                        ui.painter()
                            .circle_filled(rect.center() + vec2(column, row), 1.2, colour);
                    }
                }
                response
            })
            .response;
        let label = label.clone();
        response.widget_info(move || {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label.clone())
        });
        if response.hovered() {
            ctx.set_cursor_icon(egui::CursorIcon::Grab);
        }
    }
    let Some(moving) = egui::DragAndDrop::payload::<MovingPanel>(&ctx) else {
        return;
    };
    let Some(pointer) = ctx.pointer_latest_pos() else {
        return;
    };
    let others: Vec<(PanelPlace, Rect)> = open
        .iter()
        .filter(|(place, _)| place.panel != moving.0)
        .copied()
        .collect();
    let (edge, index, line) = panel_landing(pointer.x, window, page, &others);
    ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
    ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        Id::new("side-panel-landing"),
    ))
    .line_segment(
        [
            egui::pos2(line, window.top()),
            egui::pos2(line, window.bottom()),
        ],
        Stroke::new(3.0, palette.accent),
    );
    if ctx.input(|input| input.pointer.any_released()) {
        egui::DragAndDrop::clear_payload(&ctx);
        app.actions.push(Action::PlaceSidePanel {
            panel: moving.0,
            edge,
            index,
        });
    }
}

/// The grip's size: two rows of three dots, in the panel's top margin.
const GRIP_SIZE: egui::Vec2 = vec2(24.0, 10.0);

/// The sidebar's narrowest width.
pub(crate) const SIDEBAR_MIN_WIDTH: f32 = 210.0;

/// Raise the window's minimum width while the Queue or Lyrics panel is open,
/// so even at its narrowest the page beside the panels keeps the room its
/// top bar needs (#624), and lower it again once they close. Window managers
/// that tile ignore the minimum; the panels still give way there.
///
/// Full-screen lyrics draw no sidebar or side panel (an open queue becomes a
/// column inside them), so they ask for no room for them. A screen narrower
/// than the panels' minimum, such as a portrait monitor, then holds the
/// full-screen window without a minimum wider than itself (#783).
fn keep_room_for_panels(app: &App, ctx: &Context) {
    if crate::window::fixed_size() {
        return;
    }
    let panels = app.lyrics_fullscreen.is_none();
    let width = main_min_width(
        topbar::least_width(ctx),
        panels && app.settings.sidebar_visible,
        if panels {
            app.open_side_panels().len()
        } else {
            0
        },
    )
    .round();
    let id = Id::new("main-min-width");
    let sent = ctx.data(|data| data.get_temp::<f32>(id));
    // The window starts at MAIN_MIN_SIZE, so there is nothing to send until
    // the panels need more.
    if sent.unwrap_or(crate::window::MAIN_MIN_SIZE[0]) != width {
        ctx.data_mut(|data| data.insert_temp(id, width));
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(vec2(
            width,
            crate::window::MAIN_MIN_SIZE[1],
        )));
    }
}

/// How a resizable side panel fits beside the page.
pub(crate) struct PanelFit {
    /// The widths the panel may take this frame.
    pub range: std::ops::RangeInclusive<f32>,
    /// Whether the room the page needs holds the panel under the width the
    /// person chose for it.
    pub yielding: bool,
}

/// Fit a side panel between `range` and the page beside it. The panel gives
/// up width, down to its minimum, so the page keeps `room_for_page`, the
/// least its top bar needs (#624). The width chosen for the panel stays in
/// the settings and comes back once the window widens again.
///
/// `room` is what the panel and everything after it share.
pub(crate) fn yielding_panel(
    ctx: &Context,
    id: &str,
    range: std::ops::RangeInclusive<f32>,
    chosen: f32,
    room: f32,
) -> PanelFit {
    let (min, max) = range.into_inner();
    let max = max.min(room).max(min);
    let width = chosen.clamp(min, max);
    // egui remembers the width the panel last drew at. While nobody drags
    // its handle, hand it back the chosen width, either way: a resize made
    // elsewhere (the fullscreen queue column shares the queue width) shows
    // here too, and the panel never writes a stale width back over it.
    let id = Id::new(id);
    if !ctx.is_being_dragged(id.with("__resize"))
        && let Some(mut state) = egui::containers::panel::PanelState::load(ctx, id)
        && (state.outer_rect.width() - width).abs() > 0.5
    {
        state.outer_rect.set_width(width);
        ctx.data_mut(|data| data.insert_persisted(id, state));
    }
    PanelFit {
        range: min..=max,
        yielding: max < chosen - 0.5,
    }
}

/// Whether the person is dragging this panel's resize handle right now.
pub(crate) fn panel_dragged(ctx: &Context, id: &str) -> bool {
    ctx.is_being_dragged(Id::new(id).with("__resize"))
}

/// Whether a panel's new width is one the person chose: it moved while not
/// held back by the page, or by their own drag.
pub(crate) fn panel_width_chosen(ctx: &Context, id: &str, fit: &PanelFit) -> bool {
    !fit.yielding || panel_dragged(ctx, id)
}

/// Keeps the most recent loading preview of each metadata type available to
/// the loaded hero as an artwork fallback. The fixed typed slot bounds this to
/// one playlist, album, artist, and show instead of scanning known metadata on
/// every loaded frame.
fn loading_preview<T>(
    ctx: &Context,
    key: &str,
    details: &Loadable<T>,
    known: impl FnOnce() -> Option<T>,
) -> Option<Arc<T>>
where
    T: Send + Sync + 'static,
{
    let memory_id = Id::new("collection-loading-preview");
    let key = Id::new(key);
    if details.get().is_some() {
        ctx.data(|data| data.get_temp::<(Id, Arc<T>)>(memory_id))
            .filter(|(stored_key, _)| *stored_key == key)
            .map(|(_, preview)| preview)
    } else {
        let preview = known().map(Arc::new);
        ctx.data_mut(|data| match &preview {
            Some(preview) => {
                data.insert_temp(memory_id, (key, Arc::clone(preview)));
            }
            None => data.remove::<(Id, Arc<T>)>(memory_id),
        });
        preview
    }
}

fn page_tint(app: &mut App) -> Option<Color32> {
    let page = app.page().clone();
    let image = match &page {
        Page::Playlist(id) => app
            .playlist_pages
            .get(id)
            .and_then(|page| page.playlist.get())
            .or_else(|| app.known_playlist(id))
            .and_then(|playlist| pick_image(&playlist.images, 64))
            .map(str::to_string),
        Page::Album(id) => app
            .album_pages
            .get(id)
            .and_then(|page| page.album.get())
            .or_else(|| app.known_album(id))
            .and_then(|album| pick_image(&album.images, 64))
            .map(str::to_string),
        Page::Artist(id) => app
            .artist_pages
            .get(id)
            .and_then(|page| page.artist.get())
            .or_else(|| app.known_artist(id))
            .and_then(|artist| pick_image(&artist.images, 64))
            .map(str::to_string),
        Page::Show(id) => app
            .show_pages
            .get(id)
            .and_then(|page| page.show.get())
            .or_else(|| app.known_show(id))
            .and_then(|show| pick_image(&show.images, 64))
            .map(str::to_string),
        Page::Radio(seed) => pick_image(&app.radio_images(seed), 64).map(str::to_string),
        Page::LikedSongs => {
            return app
                .settings
                .accent_from_art
                .then_some(Color32::from_rgb(0x50, 0x38, 0xc8));
        }
        _ => None,
    };
    if !app.settings.accent_from_art && image.is_some() {
        return None;
    }
    match image {
        Some(url) => app.tint_for(Some(&url)).or_else(|| app.now_playing_tint()),
        None => app.now_playing_tint(),
    }
}

fn central(app: &mut App, ui: &mut egui::Ui) {
    let palette = app.palette;
    let tint = page_tint(app);
    egui::CentralPanel::default()
        .frame(Frame::new().fill(palette.window))
        .show(ui, |ui| {
            let rect = ui.max_rect();
            let page = app.page().clone();
            let overview = matches!(
                page,
                Page::Home | Page::Search | Page::Settings | Page::Queue | Page::Game
            );
            // The wash belongs to the page's hero, so it scrolls away with
            // it. The page's offset is known once it has been laid out, so
            // the previous frame's serves.
            let scrolled_id = Id::new(("page-scrolled", page.encode()));
            let scrolled = ui
                .ctx()
                .data(|data| data.get_temp::<f32>(scrolled_id))
                .unwrap_or(0.0);
            let countdown = matches!(&page, Page::Album(id)
                if app.album_pages.get(id).is_some_and(|page| page.prerelease.is_some()));
            // Countdown pages own their hero gradient. The generic artwork wash
            // otherwise leaves an unrelated color band behind the search bar.
            if !countdown && let Some((top, bottom)) = header_wash(&palette, overview, tint) {
                widgets::paint_vertical_gradient(ui, header_rect(rect, scrolled), top, bottom);
            }
            ui.spacing_mut().item_spacing = vec2(8.0, 6.0);
            // egui fades a scrolled page's edge into the panel's plain
            // colour, which shows as a pale band over a cover's tint; the
            // page casts a shadow under the header instead.
            ui.spacing_mut().scroll.fade.strength = 0.0;
            topbar::show(app, ui);
            // One banner at a time: a song that will not play matters more
            // than a slow app, which waits its turn.
            if app.dialog.is_none() && page != Page::Settings {
                if app.soloist_waiting() {
                    soloist::banner(app, ui);
                } else if app.audio_key_banner {
                    audio_key_banner(app, ui);
                } else if app.slow_banner {
                    slow_banner(app, ui);
                }
            }
            let page_key = page.encode();
            // A playlist come back to opens where it was, as in Spotify,
            // even while its rows are read again and it is briefly shorter.
            let remembers = matches!(page, Page::Playlist(_) | Page::LikedSongs);
            let arrived = app
                .page_scroll
                .as_ref()
                .is_none_or(|(at, _)| *at != page_key);
            if arrived {
                app.scroll_return = remembers
                    .then(|| app.page_offsets.get(&page_key).copied())
                    .flatten()
                    .filter(|offset| *offset > 0.0)
                    .map(|offset| (page_key.clone(), offset, 0));
            }
            // A wheel, a drag or a key while returning is the listener's own
            // scroll, which wins.
            let scrolled_by_hand = ui.input(|input| {
                input.pointer.primary_down()
                    || input.events.iter().any(|event| {
                        matches!(
                            event,
                            egui::Event::MouseWheel { .. } | egui::Event::Key { pressed: true, .. }
                        )
                    })
            });
            if scrolled_by_hand && !arrived {
                app.scroll_return = None;
            }
            let mut area = egui::ScrollArea::vertical()
                .id_salt(("page", &page_key))
                .auto_shrink([false, false]);
            if let Some((_, offset)) = app.restore_scroll.take().filter(|(at, _)| *at == page_key) {
                area = area.vertical_scroll_offset(offset);
                app.scroll_return = None;
            } else if let Some((_, offset, _)) = app
                .scroll_return
                .as_ref()
                .filter(|(at, _, _)| *at == page_key)
            {
                area = area.vertical_scroll_offset(*offset);
            }
            let scroll = crate::autoscroll::show(ui, area, egui::Vec2b::new(false, true), |ui| {
                Frame::new()
                    .inner_margin(Margin {
                        left: widgets::PAGE_PADDING as i8,
                        right: widgets::PAGE_PADDING as i8,
                        top: 4,
                        bottom: 48,
                    })
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        if vacuum_notice(app, ui, &page) {
                            return;
                        }
                        match page {
                            Page::Home => home::show(app, ui),
                            Page::TopSongs => collection::top_songs(app, ui),
                            Page::LocalFiles => collection::local_files(app, ui),
                            Page::NewReleases => new_releases::page(app, ui),
                            Page::Search => search::show(app, ui),
                            Page::LikedSongs => collection::liked(app, ui),
                            Page::Albums | Page::Artists | Page::Podcasts | Page::Episodes => {
                                library::show(app, ui, page)
                            }
                            Page::Playlist(id) => collection::playlist(app, ui, &id),
                            Page::Album(id) => collection::album(app, ui, &id),
                            Page::Artist(id) => artist::show(app, ui, &id),
                            Page::Show(id) => show::show(app, ui, &id),
                            Page::Radio(seed) => radio::radio(app, ui, &seed),
                            Page::Queue => queue::page(app, ui),
                            Page::Settings => settings::show(app, ui),
                            Page::Game => game::show(app, ui),
                        }
                    });
                if app.dialog.is_none() && !app.show_devices {
                    keys::scroll_page(ui);
                }
            });
            let now = scroll.state.offset.y;
            match &mut app.scroll_return {
                // Held until the page is long enough to show the place, for
                // a few seconds at most.
                Some((at, offset, frames)) if *at == page_key => {
                    *frames += 1;
                    if (now - *offset).abs() < 1.0 || *frames > 300 {
                        app.scroll_return = None;
                    }
                }
                _ => {
                    if remembers {
                        app.page_offsets.insert(page_key.clone(), now);
                    }
                }
            }
            app.page_scroll = Some((page_key, now));
            ui.ctx()
                .data_mut(|data| data.insert_temp(scrolled_id, scroll.state.offset.y));
            header_shadow(ui, scroll.inner_rect, scroll.state.offset.y, palette.dark);
        });
}

/// With Vacuum hiding pages about a person or popularity, draws the notice
/// in such a page's place, with Show anyway, and answers whether it did:
/// a playlist judged to be about a person or popularity, your top songs, and
/// a radio station, which is built around its seed.
fn vacuum_notice(app: &mut App, ui: &mut egui::Ui, page: &Page) -> bool {
    let Some(gate) = crate::vacuum::gate::current() else {
        return false;
    };
    if !gate.options.hide_pages || app.vacuum_shown_anyway.contains(&page.encode()) {
        return false;
    }
    let hidden = match page {
        Page::TopSongs | Page::Radio(_) => true,
        Page::Playlist(id) => {
            let owner = app
                .known_playlist(id)
                .and_then(|playlist| playlist.owner.id.clone());
            !gate.playlist_shown(id, owner.as_deref())
        }
        _ => false,
    };
    if !hidden {
        return false;
    }
    let palette = app.palette;
    ui.add_space(60.0);
    widgets::empty_state(
        ui,
        &palette,
        Icon::EyeOff,
        &crate::i18n::gettext(app.locale, "Hidden by Vacuum"),
        &crate::i18n::gettext(
            app.locale,
            "This page is about a person, or about what is popular.",
        ),
    );
    ui.add_space(12.0);
    ui.vertical_centered(|ui| {
        if theme::pill_button(
            ui,
            &palette,
            &crate::i18n::gettext(app.locale, "Show anyway"),
            false,
        )
        .clicked()
        {
            app.actions.push(Action::VacuumShowAnyway(page.encode()));
        }
    });
    true
}

/// Where the "Why?" link of the slow-app banner leads.
const SLOW_BANNER_GUIDE: &str = "https://spotifast.rocks/troubleshooting/#rate-limits";
/// Where the "Why?" link of the audio-key banner leads.
pub const AUDIO_KEY_GUIDE: &str = "https://spotifast.rocks/troubleshooting/#audio-key";

/// A strip under the top bar suggesting a personal app, after Spotify
/// told the shared app, which only listeners signed in before 0.14.2 still
/// use, that it sent too many requests. It stays put while the
/// page scrolls, and goes when it is closed.
fn slow_banner(app: &mut App, ui: &mut egui::Ui) {
    use crate::i18n::gettext;
    let locale = app.locale;
    banner(
        app,
        ui,
        &gettext(locale, "Spotify is being slow on the shared sign-in."),
        &gettext(
            locale,
            "Sirensong no longer offers the shared sign-in, and Spotify can stop accepting it at any time. Register Sirensong and you won't have to wait.",
        ),
        false,
        vec![
            (
                gettext(locale, "Register Sirensong"),
                Action::OpenPersonalAppSetup,
            ),
            (
                // Translators: a button that opens a page explaining why
                // the banner it sits in appeared.
                gettext(locale, "Why?"),
                Action::OpenUrl(SLOW_BANNER_GUIDE.into()),
            ),
        ],
        Some(Action::CloseSlowBanner),
    );
}

/// A strip under the top bar after Spotify refused the key that decrypts a
/// song on this computer, offering another Spotify device, which fetches
/// its own audio, and the guide's explanation.
fn audio_key_banner(app: &mut App, ui: &mut egui::Ui) {
    use crate::i18n::gettext;
    let locale = app.locale;
    banner(
        app,
        ui,
        &gettext(
            locale,
            "Spotify won't let this computer play this song right now.",
        ),
        &gettext(
            locale,
            "Spotify refused the key that unlocks the audio. Sirensong can't fix that, but it sometimes clears up on its own. You can still play on your phone or another Spotify device.",
        ),
        false,
        vec![
            (
                gettext(locale, "Play on another device"),
                Action::PlayOnAnotherDevice,
            ),
            (
                gettext(locale, "Why?"),
                Action::OpenUrl(AUDIO_KEY_GUIDE.into()),
            ),
        ],
        Some(Action::CloseAudioKeyBanner),
    );
}

/// A notice under the top bar: a bold line, with a spinner while `busy`, a
/// sentence or two, its buttons with the main one first, and a close button
/// unless the notice goes by itself.
fn banner(
    app: &mut App,
    ui: &mut egui::Ui,
    title: &str,
    text: &str,
    busy: bool,
    buttons: Vec<(std::borrow::Cow<'static, str>, Action)>,
    close: Option<Action>,
) {
    use crate::i18n::gettext;
    let palette = app.palette;
    let locale = app.locale;
    Frame::new()
        .outer_margin(Margin {
            left: widgets::PAGE_PADDING as i8,
            right: widgets::PAGE_PADDING as i8,
            top: 0,
            bottom: 8,
        })
        .fill(palette.surface)
        .stroke(Stroke::new(1.0, palette.outline))
        .corner_radius(CornerRadius::same(theme::RADIUS))
        .inner_margin(Margin {
            left: 16,
            right: 6,
            top: 12,
            bottom: 12,
        })
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                if let Some(close) = close
                    && theme::icon_button(
                        ui,
                        Icon::X,
                        16.0,
                        palette.secondary,
                        palette.text,
                        &gettext(locale, "Close banner"),
                    )
                    .clicked()
                {
                    app.actions.push(close);
                }
                ui.with_layout(egui::Layout::top_down(egui::Align::Min), |ui| {
                    ui.spacing_mut().item_spacing = vec2(8.0, 4.0);
                    ui.horizontal(|ui| {
                        if busy {
                            theme::spinner(ui, 14.0, palette.accent);
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(title)
                                    .font(theme::bold(14.0))
                                    .color(palette.text),
                            )
                            .wrap(),
                        );
                    });
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(text)
                                .font(theme::regular(13.0))
                                .color(palette.secondary),
                        )
                        .wrap(),
                    );
                    if !buttons.is_empty() {
                        ui.add_space(6.0);
                        ui.horizontal_wrapped(|ui| {
                            for (index, (label, action)) in buttons.into_iter().enumerate() {
                                if theme::pill_button(ui, &palette, &label, index == 0).clicked() {
                                    app.actions.push(action);
                                }
                            }
                        });
                    }
                });
            });
        });
}

/// The gradient behind a page's header, top to bottom: the art's colour
/// blended into the window, stronger on a cover's own page than on an
/// overview, or else the palette's `tint` fading out, whose alpha is its
/// strength. `None` leaves the plain window.
fn header_wash(
    palette: &theme::Palette,
    overview: bool,
    art: Option<Color32>,
) -> Option<(Color32, Color32)> {
    if let Some(art) = art {
        let strength = if overview { 0.45 } else { 0.85 };
        return Some((blend(palette.window, art, strength), palette.window));
    }
    (palette.tint.a() > 0).then_some((palette.tint, Color32::TRANSPARENT))
}

/// Where the header's wash is painted: the top of the panel, moved up by
/// how far the page has scrolled, so it leaves with the hero.
fn header_rect(panel: Rect, scrolled: f32) -> Rect {
    Rect::from_min_size(
        panel.min - vec2(0.0, scrolled.max(0.0)),
        vec2(panel.width(), 340.0),
    )
}

/// The shadow the header casts on a page scrolled under it, deepening over
/// the first few points of scrolling. Dark in both themes, lighter over a
/// light page.
fn header_shadow(ui: &egui::Ui, page: Rect, scrolled: f32, dark: bool) {
    let depth = (scrolled / 24.0).clamp(0.0, 1.0);
    if depth <= 0.0 {
        return;
    }
    let strength = if dark { 110.0 } else { 36.0 };
    let rect = Rect::from_min_size(page.min, vec2(page.width(), 14.0));
    widgets::paint_vertical_gradient(
        ui,
        rect,
        egui::Color32::from_black_alpha((strength * depth) as u8),
        egui::Color32::TRANSPARENT,
    );
}

/// Without a native title bar, the top of every panel drags the window, not
/// only the page's top bar.
const fn titlebar_spans_window(on_macos: bool, custom_titlebar: bool) -> bool {
    on_macos || custom_titlebar
}

/// The room the top bar's own controls need beside the caption buttons: the
/// 760-point minimum with a 250-point sidebar leaves 510 points.
const TOPBAR_MIN_CONTENT: f32 = 510.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct WindowControlsReservation {
    pub topbar_width: f32,
    pub topbar_top: f32,
    pub queue_top: f32,
    pub lyrics_top: f32,
}

/// The caption buttons on the right belong to the outermost header: the
/// queue's or the lyrics' when either panel is open, otherwise the top bar,
/// beside its controls or above them when the page is too narrow.
fn controls_reservation(
    footprint: fastframe_titlebar::Footprint,
    queue: bool,
    lyrics: bool,
    topbar_width: f32,
) -> WindowControlsReservation {
    let mut space = WindowControlsReservation {
        topbar_width: 0.0,
        topbar_top: 0.0,
        queue_top: 0.0,
        lyrics_top: 0.0,
    };
    if footprint.right <= 0.0 {
        return space;
    }
    if queue {
        space.queue_top = footprint.height;
    } else if lyrics {
        space.lyrics_top = footprint.height;
    } else {
        let reserve = footprint.reserve(topbar_width, TOPBAR_MIN_CONTENT);
        space.topbar_width = reserve.inline_width;
        space.topbar_top = reserve.push_down;
    }
    space
}

/// Left-hand Linux controls sit above the sidebar's navigation rows.
pub(super) fn linux_left_controls_inset(ctx: &egui::Context) -> f32 {
    if !cfg!(target_os = "linux") {
        return 0.0;
    }
    let footprint = fastframe_titlebar::footprint(ctx, theme::CUSTOM_TOP_BAR_HEIGHT);
    if footprint.left > 0.0 {
        footprint.height
    } else {
        0.0
    }
}

pub(super) fn window_controls_reservation(
    ctx: &egui::Context,
    queue: bool,
    lyrics: bool,
    topbar_width: f32,
) -> WindowControlsReservation {
    controls_reservation(
        fastframe_titlebar::footprint(ctx, theme::CUSTOM_TOP_BAR_HEIGHT),
        queue,
        lyrics,
        topbar_width,
    )
}

/// Draws the caption buttons over the window's top corners in the palette's
/// colours: Windows' glyphs in the secondary text colour, GNOME's round
/// buttons in the text colour on the hover surface.
fn caption_buttons(ui: &mut egui::Ui, palette: &theme::Palette, locale: crate::i18n::Locale) {
    use crate::i18n::gettext;
    let style = fastframe_titlebar::Style {
        icon: if cfg!(target_os = "linux") {
            palette.text
        } else {
            palette.secondary
        },
        button: palette.surface_hover,
        header_height: theme::CUSTOM_TOP_BAR_HEIGHT,
    };
    // Literal calls, so xgettext extracts each label.
    let minimize = gettext(locale, "Minimize");
    let maximize = gettext(locale, "Maximize");
    let restore = gettext(locale, "Restore");
    let close = gettext(locale, "Close");
    let labels = fastframe_titlebar::Labels {
        minimize: &minimize,
        maximize: &maximize,
        restore: &restore,
        close: &close,
    };
    fastframe_titlebar::controls(ui, &style, &labels);
}

pub fn blend(base: Color32, tint: Color32, amount: f32) -> Color32 {
    let a = egui::Rgba::from(base);
    let b = egui::Rgba::from(tint);
    let mixed = a * (1.0 - amount) + b * amount;
    let mut color = Color32::from(mixed);
    color[3] = 255;
    color
}

fn toasts(app: &mut App, ctx: &egui::Context, bottom_offset: f32) {
    if app.toasts.is_empty() {
        return;
    }
    let palette = app.palette;
    egui::Area::new(egui::Id::new("toasts"))
        .anchor(Align2::RIGHT_BOTTOM, vec2(-20.0, -bottom_offset))
        .order(egui::Order::Tooltip)
        .interactable(false)
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 8.0;
            for toast in &app.toasts {
                let age = toast.created.elapsed().as_secs_f32();
                let alpha = if age < 0.15 {
                    age / 0.15
                } else if age > 2.8 {
                    ((3.2 - age) / 0.4).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                ui.set_opacity(alpha);
                Frame::new()
                    .fill(palette.overlay)
                    .stroke(Stroke::new(1.0, palette.outline))
                    .corner_radius(CornerRadius::same(theme::RADIUS))
                    .inner_margin(Margin::symmetric(14, 10))
                    .shadow(egui::epaint::Shadow {
                        offset: [0, 4],
                        blur: 16,
                        spread: 0,
                        color: palette.shadow,
                    })
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (icon, color) = match toast.kind {
                                ToastKind::Info => (Icon::CircleCheck, palette.accent),
                                ToastKind::Error => (Icon::CircleAlert, palette.danger),
                            };
                            theme::icon(ui, icon, 16.0, color);
                            // Laid out at its own width. The area
                            // remembers its size, so after a short toast a
                            // label left to wrap at the area's width broke
                            // long messages on every word.
                            let galley = ui.painter().layout(
                                toast.message.clone(),
                                theme::medium(13.5),
                                palette.text,
                                280.0,
                            );
                            ui.add(egui::Label::new(galley));
                        });
                    });
            }
        });
}

#[cfg(test)]
mod window_chrome_tests {
    use super::*;
    use fastframe_titlebar::Footprint;

    /// Windows' three buttons in its 48-point header.
    const BUTTONS: Footprint = Footprint {
        left: 0.0,
        right: 3.0 * 36.0 + 5.0,
        height: theme::CUSTOM_TOP_BAR_HEIGHT,
    };

    #[test]
    fn the_side_panels_drag_a_window_without_a_native_title_bar() {
        assert!(titlebar_spans_window(true, false));
        assert!(titlebar_spans_window(false, true));
        assert!(!titlebar_spans_window(false, false));
    }

    #[test]
    fn caption_space_belongs_to_the_outermost_header() {
        let values = |footprint, queue, lyrics| {
            let space = controls_reservation(footprint, queue, lyrics, f32::INFINITY);
            [
                space.topbar_width,
                space.topbar_top,
                space.queue_top,
                space.lyrics_top,
            ]
        };
        let [width, height] = [BUTTONS.right, BUTTONS.height];
        assert_eq!(values(BUTTONS, false, false), [width, 0.0, 0.0, 0.0]);
        assert_eq!(values(BUTTONS, true, false), [0.0, 0.0, height, 0.0]);
        assert_eq!(values(BUTTONS, false, true), [0.0, 0.0, 0.0, height]);
        assert_eq!(values(BUTTONS, true, true), [0.0, 0.0, height, 0.0]);
        // No buttons on the right: full screen, the native frame, macOS's
        // traffic lights, or a GNOME layout with them all on the left.
        let none = Footprint {
            left: 50.0,
            ..Footprint::default()
        };
        for (queue, lyrics) in [(false, false), (true, false), (false, true)] {
            assert_eq!(values(none, queue, lyrics), [0.0; 4]);
        }
    }

    #[test]
    fn minimum_windows_window_stacks_caption_space_above_the_topbar() {
        let available = 760.0 - 250.0;
        let space = controls_reservation(BUTTONS, false, false, available);
        assert_eq!(space.topbar_width, 0.0);
        assert_eq!(space.topbar_top, BUTTONS.height);

        let inline =
            controls_reservation(BUTTONS, false, false, TOPBAR_MIN_CONTENT + BUTTONS.right);
        assert_eq!(inline.topbar_width, BUTTONS.right);
        assert_eq!(inline.topbar_top, 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A page fades from its cover's colour while art tints the interface,
    /// otherwise from the palette's own wash, and from nothing when the
    /// palette has none.
    #[test]
    fn the_header_fades_from_the_art_or_the_palettes_wash() {
        let mut palette = theme::Palette::dark();
        assert_eq!(header_wash(&palette, false, None), None);

        let art = Color32::from_rgb(0x50, 0x38, 0xc8);
        let (page, _) = header_wash(&palette, false, Some(art)).unwrap();
        let (overview, _) = header_wash(&palette, true, Some(art)).unwrap();
        assert_eq!(page, blend(palette.window, art, 0.85));
        assert_eq!(overview, blend(palette.window, art, 0.45));

        palette.tint = Color32::from_black_alpha(24);
        assert_eq!(
            header_wash(&palette, false, None),
            Some((palette.tint, Color32::TRANSPARENT))
        );
        assert_eq!(
            header_wash(&palette, false, Some(art)).unwrap().0,
            page,
            "art wins over the wash"
        );
    }

    /// The wash sits at the top of an unscrolled page and leaves with the
    /// hero as the page scrolls, instead of staying behind the page.
    #[test]
    fn the_header_wash_scrolls_away_with_the_page() {
        let panel = Rect::from_min_size(egui::pos2(0.0, 80.0), vec2(800.0, 600.0));
        assert_eq!(header_rect(panel, 0.0).min, panel.min);
        assert_eq!(header_rect(panel, 0.0).width(), 800.0);
        assert_eq!(header_rect(panel, 120.0).min.y, panel.min.y - 120.0);
        assert_eq!(header_rect(panel, 120.0).max.y, panel.min.y + 220.0);
        assert!(
            header_rect(panel, 400.0).max.y < panel.min.y,
            "gone once scrolled past"
        );
        assert_eq!(
            header_rect(panel, -5.0).min,
            panel.min,
            "no overscroll bounce"
        );
    }

    /// The header casts a shadow only on a page scrolled under it, and it
    /// is black in both themes, never the page's own colour.
    #[test]
    fn the_header_shadow_appears_once_the_page_scrolls() {
        let ctx = egui::Context::default();
        let page = Rect::from_min_size(egui::pos2(0.0, 80.0), vec2(800.0, 600.0));
        let shadows = |scrolled: f32, dark: bool| {
            let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {
                header_shadow(ui, page, scrolled, dark);
            });
            output.textures_delta.clear();
            output
                .shapes
                .iter()
                .filter_map(|shape| match &shape.shape {
                    egui::Shape::Mesh(mesh) => Some(mesh.vertices.clone()),
                    _ => None,
                })
                .flatten()
                .collect::<Vec<_>>()
        };
        assert!(
            shadows(0.0, true).is_empty(),
            "nothing at the top of the page"
        );
        for dark in [true, false] {
            let vertices = shadows(40.0, dark);
            let top = vertices
                .iter()
                .find(|vertex| vertex.pos.y == page.top())
                .expect("a shadow along the page's top edge");
            assert!(top.color.a() > 0);
            assert_eq!((top.color.r(), top.color.g(), top.color.b()), (0, 0, 0));
        }
    }
}

#[cfg(test)]
mod side_panel_landing_tests {
    use super::*;

    /// A dragged panel lands at the edge on the pointer's half, before the
    /// panels there that are nearer the window's edge than the pointer is
    /// counted, and the line stands where it would.
    #[test]
    fn a_dragged_panel_lands_on_the_pointers_side_and_place() {
        let window = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1200.0, 800.0));
        let page = Rect::from_min_max(egui::pos2(300.0, 0.0), egui::pos2(700.0, 800.0));
        let queue = PanelPlace {
            panel: SidePanel::Queue,
            edge: PanelEdge::Right,
        };
        let friends = PanelPlace {
            panel: SidePanel::Friends,
            edge: PanelEdge::Right,
        };
        let outer = Rect::from_min_max(egui::pos2(950.0, 0.0), egui::pos2(1200.0, 800.0));
        let inner = Rect::from_min_max(egui::pos2(700.0, 0.0), egui::pos2(950.0, 800.0));
        let others = [(queue, outer), (friends, inner)];
        assert_eq!(
            panel_landing(1100.0, window, page, &others),
            (PanelEdge::Right, 0, 1200.0),
            "over the outer panel's middle: nearest the edge"
        );
        assert_eq!(
            panel_landing(900.0, window, page, &others),
            (PanelEdge::Right, 1, 950.0),
            "between the two"
        );
        assert_eq!(
            panel_landing(650.0, window, page, &others),
            (PanelEdge::Right, 2, 700.0),
            "inside them both, beside the page"
        );
        assert_eq!(
            panel_landing(100.0, window, page, &others),
            (PanelEdge::Left, 0, 300.0),
            "the left half, beside the page, with none there"
        );
    }
}
