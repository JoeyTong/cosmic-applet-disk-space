use crate::icons;
use crate::storage::{
    self, Drive, DriveKind, DriveStatus, Filesystem, Snapshot, Usage, format_bytes,
};
use cosmic::app::{Core, Task};
use cosmic::iced::widget::{column, container, progress_bar, row, scrollable, text};
use cosmic::iced::{Alignment, Border, Color, Length, Limits, Rectangle, Subscription, window};
use cosmic::surface;
use cosmic::widget::{self, autosize, button, icon};
use cosmic::{Element, Theme};
use std::borrow::Cow;
use std::sync::LazyLock;
use std::time::Instant;

pub const APP_ID: &str = "io.github.joeytong.cosmic-applet-disk-space";
const MAX_STRIP_WIDTH: f32 = 720.0;
const MAX_STRIP_HEIGHT: f32 = 640.0;
const POPUP_WIDTH: f32 = 420.0;
static PANEL_ID: LazyLock<widget::Id> = LazyLock::new(|| widget::Id::new("disk-space-panel"));

pub struct DiskSpace {
    core: Core,
    snapshot: Option<Snapshot>,
    last_success: Option<Instant>,
    error: Option<String>,
    scanning: bool,
    popup: Option<window::Id>,
    popup_drive: Option<String>,
    details_expanded: bool,
}

#[derive(Clone, Debug)]
pub enum Message {
    Refresh,
    Scanned(Result<Snapshot, String>),
    TogglePopup(Option<String>, Rectangle),
    ToggleDetails,
    ClosePopup,
    Closed(window::Id),
}

fn scan_task() -> Task<Message> {
    Task::perform(storage::scan(), |result| {
        cosmic::Action::App(Message::Scanned(result))
    })
}

impl cosmic::Application for DiskSpace {
    type Executor = cosmic::SingleThreadExecutor;
    type Flags = ();
    type Message = Message;
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    fn init(core: Core, _: ()) -> (Self, Task<Message>) {
        (
            Self {
                core,
                snapshot: None,
                last_success: None,
                error: None,
                scanning: true,
                popup: None,
                popup_drive: None,
                details_expanded: false,
            },
            scan_task(),
        )
    }

    fn subscription(&self) -> Subscription<Message> {
        cosmic::iced::time::every(storage::REFRESH_INTERVAL).map(|_| Message::Refresh)
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Refresh => {
                if !self.scanning {
                    self.scanning = true;
                    return scan_task();
                }
            }
            Message::Scanned(result) => {
                self.scanning = false;
                match result {
                    Ok(snapshot) => {
                        if self.popup_drive.as_ref().is_some_and(|path| {
                            !snapshot.drives.iter().any(|drive| &drive.path == path)
                        }) {
                            self.details_expanded = false;
                        }
                        self.snapshot = Some(snapshot);
                        self.last_success = Some(Instant::now());
                        self.error = None;
                    }
                    Err(error) => {
                        if self.error.as_ref() != Some(&error) {
                            tracing::warn!("{error}");
                        }
                        self.error = Some(error);
                    }
                }
            }
            Message::TogglePopup(drive, rectangle) => {
                self.details_expanded = false;
                if let Some(id) = self.popup.take() {
                    self.popup_drive = None;
                    return surface::surface_task(surface::action::destroy_popup(id));
                }
                self.popup_drive = drive;
                return surface::surface_task(surface::action::app_popup(
                    |_| Default::default(),
                    move |app: &mut Self| {
                        let id = window::Id::unique();
                        app.popup = Some(id);
                        let mut settings = app.core.applet.get_popup_settings(
                            app.core.main_window_id().unwrap_or(window::Id::RESERVED),
                            id,
                            None,
                            None,
                            None,
                        );
                        settings.positioner.anchor_rect = Rectangle {
                            x: rectangle.x as i32,
                            y: rectangle.y as i32,
                            width: rectangle.width.max(1.0) as i32,
                            height: rectangle.height.max(1.0) as i32,
                        };
                        settings.positioner.size_limits = popup_limits();
                        settings
                    },
                    None,
                ));
            }
            Message::ToggleDetails => {
                if self.popup.is_some() {
                    self.details_expanded = !self.details_expanded;
                }
            }
            Message::ClosePopup => {
                self.details_expanded = false;
                if let Some(id) = self.popup.take() {
                    self.popup_drive = None;
                    return surface::surface_task(surface::action::destroy_popup(id));
                }
            }
            Message::Closed(id) => {
                if self.popup == Some(id) {
                    self.popup = None;
                    self.popup_drive = None;
                    self.details_expanded = false;
                }
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let horizontal = self.core.applet.is_horizontal();
        let entries: Vec<Element<'_, Message>> = match &self.snapshot {
            Some(snapshot) if !snapshot.drives.is_empty() => snapshot
                .drives
                .iter()
                .map(|drive| self.drive_button(drive, horizontal))
                .collect(),
            _ => vec![self.status_button()],
        };
        let minor_size = self.minor_size();
        let bounds = self.core.applet.suggested_bounds;
        let scrollbar = scrollable::Scrollbar::new().width(3).scroller_width(3);
        let (strip, limits): (Element<'_, Message>, _) = if horizontal {
            (
                scrollable(row(entries).spacing(2).align_y(Alignment::Center))
                    .direction(scrollable::Direction::Horizontal(scrollbar))
                    .width(Length::Shrink)
                    .height(minor_size)
                    .into(),
                Limits::NONE
                    .max_width(
                        bounds
                            .filter(|size| size.width > 0.0)
                            .map_or(MAX_STRIP_WIDTH, |size| size.width.min(MAX_STRIP_WIDTH)),
                    )
                    .height(minor_size),
            )
        } else {
            (
                scrollable(column(entries).spacing(4).align_x(Alignment::Center))
                    .direction(scrollable::Direction::Vertical(scrollbar))
                    .width(minor_size)
                    .height(Length::Shrink)
                    .into(),
                Limits::NONE.width(minor_size).max_height(
                    bounds
                        .filter(|size| size.height > 0.0)
                        .map_or(MAX_STRIP_HEIGHT, |size| size.height.min(MAX_STRIP_HEIGHT)),
                ),
            )
        };
        autosize::autosize(strip, PANEL_ID.clone())
            .limits(limits)
            .into()
    }

    fn view_window(&self, _: window::Id) -> Element<'_, Message> {
        let header = row![
            widget::text::title4("Disk Space"),
            widget::space::horizontal(),
            button::icon(icon::from_name("view-refresh-symbolic"))
                .on_press_maybe((!self.scanning).then_some(Message::Refresh)),
            button::icon(icon::from_name("window-close-symbolic")).on_press(Message::ClosePopup),
        ]
        .align_y(Alignment::Center)
        .spacing(8);
        let mut content = column![].spacing(16).width(Length::Fill);

        if let Some(error) = &self.error {
            let age = self.last_success.map_or_else(
                || "No successful reading yet.".to_owned(),
                |last| {
                    format!(
                        "Last successful reading: {}s ago. Values below are stale.",
                        last.elapsed().as_secs()
                    )
                },
            );
            content = content.push(notice(format!("{error}\n{age}")));
        }

        let drives = self
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.drives.as_slice())
            .unwrap_or_default();
        let selected = self
            .popup_drive
            .as_ref()
            .and_then(|path| drives.iter().find(|drive| &drive.path == path));
        if let Some(drive) =
            selected.or_else(|| self.popup_drive.is_none().then(|| drives.first()).flatten())
        {
            content = content.push(self.drive_details(drive));
        } else {
            content = content.push(text(if self.popup_drive.is_some() {
                "This drive was disconnected. The panel updates automatically."
            } else if self.snapshot.is_none() && self.error.is_none() {
                "Reading local drives…"
            } else {
                "No local drives detected."
            }));
        }

        let footer = quiet_text(format!(
            "{} · refreshes every 2s · warning at ≥80%",
            if self.error.is_some() {
                "Retrying"
            } else {
                "Live"
            }
        ));
        let scrollbar = scrollable::Scrollbar::new()
            .width(4)
            .scroller_width(4)
            .spacing(8);
        // Reserve room for the footer: a shrink-height scrollable otherwise
        // consumes the remaining column height before the footer is laid out.
        let body = container(
            scrollable(content)
                .direction(scrollable::Direction::Vertical(scrollbar))
                .height(Length::Shrink),
        )
        .max_height(500);
        let layout = column![header, body, footer].spacing(16).padding(16);

        self.core
            .applet
            .popup_container(container(layout).max_height(620))
            .limits(popup_limits())
            .into()
    }

    fn on_close_requested(&self, id: window::Id) -> Option<Message> {
        Some(Message::Closed(id))
    }

    fn style(&self) -> Option<cosmic::iced::theme::Style> {
        Some(cosmic::applet::style())
    }
}

impl DiskSpace {
    fn minor_size(&self) -> f32 {
        let suggested = self.core.applet.suggested_size(true);
        let padding = self.core.applet.suggested_padding(true).1;
        let horizontal = self.core.applet.is_horizontal();
        let natural_size = f32::from(if horizontal { suggested.1 } else { suggested.0 })
            + 2.0 * f32::from(padding);
        // These are maximum bounds. A standalone launch can receive the whole
        // monitor's dimensions, rather than the panel's actual thickness.
        self.core
            .applet
            .suggested_bounds
            .map(|bounds| {
                if horizontal {
                    bounds.height
                } else {
                    bounds.width
                }
            })
            .filter(|size| *size > 0.0)
            .map_or(natural_size, |size| size.min(natural_size))
    }

    fn drive_button<'a>(&'a self, drive: &'a Drive, horizontal: bool) -> Element<'a, Message> {
        let stale = self.error.is_some();
        let warning = drive.warning();
        let icon_size = self.core.applet.suggested_size(true).0;
        let device_icon = icon(icons::drive(drive.kind)).size(icon_size);

        let content: Element<'_, Message> = if horizontal {
            let summary = drive.panel_summary();
            let capacity: Element<'_, Message> =
                if let Some((used, rest)) = summary.split_once(" / ") {
                    row![
                        self.core
                            .applet
                            .text(used.to_owned())
                            .font(cosmic::font::bold()),
                        self.core.applet.text(format!(" / {rest}")),
                    ]
                    .align_y(Alignment::Center)
                    .into()
                } else {
                    self.core.applet.text(summary).into()
                };
            let mut content = row![device_icon, capacity]
                .spacing(4)
                .align_y(Alignment::Center);
            if warning || stale || drive.status == DriveStatus::Partial {
                content = content.push(icon(icons::warning()).size(16));
            }
            if stale {
                content = content.push(text("Stale").size(12));
            }
            container(content)
                .height(self.minor_size())
                .center_y(self.minor_size())
                .padding([0, 4])
                .into()
        } else {
            let mut content = column![device_icon].align_x(Alignment::Center).spacing(3);
            if let Some(usage) = drive.usage {
                content = content
                    .push(
                        text(usage.panel_capacity())
                            .size(10)
                            .align_x(Alignment::Center),
                    )
                    .push(text(usage.percent_label()).size(11));
            } else {
                content = content.push(
                    text(drive.status.label())
                        .size(10)
                        .align_x(Alignment::Center),
                );
            }
            for (condition, label) in [
                (drive.shared, "Shared"),
                (stale, "Stale"),
                (drive.status == DriveStatus::Partial, "Partial"),
            ] {
                if condition {
                    content = content.push(text(label).size(10));
                }
            }
            if warning || stale || drive.status == DriveStatus::Partial {
                content = content.push(icon(icons::warning()).size(14));
            }
            container(content)
                .width(self.minor_size())
                .padding([8, 2])
                .into()
        };
        let content = container(content).style(move |theme| warning_style(theme, warning));
        let path = drive.path.clone();
        button::custom(content)
            .padding(0)
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_with_rectangle(move |offset, bounds| {
                Message::TogglePopup(
                    Some(path.clone()),
                    Rectangle {
                        x: bounds.x - offset.x,
                        y: bounds.y - offset.y,
                        ..bounds
                    },
                )
            })
            .into()
    }

    fn status_button(&self) -> Element<'_, Message> {
        let label = if self.error.is_some() {
            "Disk usage unavailable"
        } else if self.snapshot.is_none() {
            "Reading drives…"
        } else {
            "No local drives"
        };
        let handle = if self.error.is_some() {
            icons::warning()
        } else {
            icons::drive(DriveKind::Unknown)
        };
        let content: Element<'_, Message> = if self.core.applet.is_horizontal() {
            row![icon(handle).size(20), text(label)]
                .align_y(Alignment::Center)
                .spacing(8)
                .into()
        } else {
            column![icon(handle).size(20), text(label).size(10)]
                .width(self.minor_size())
                .align_x(Alignment::Center)
                .into()
        };
        button::custom(container(content).padding(6))
            .class(cosmic::theme::Button::AppletIcon)
            .on_press_with_rectangle(|offset, bounds| {
                Message::TogglePopup(
                    None,
                    Rectangle {
                        x: bounds.x - offset.x,
                        y: bounds.y - offset.y,
                        ..bounds
                    },
                )
            })
            .into()
    }

    fn drive_details<'a>(&self, drive: &'a Drive) -> Element<'a, Message> {
        let mut summary = column![
            row![
                icon(icons::drive(drive.kind)).size(28),
                column![
                    text(&drive.label).font(cosmic::font::bold()).size(18),
                    quiet_text(drive.kind.label()),
                ]
                .spacing(3)
                .width(Length::Fill),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        ]
        .spacing(16);
        if let Some(usage) = drive.usage {
            summary = summary.push(usage_view(usage, true));
        } else {
            summary = summary
                .push(
                    text(drive.status.label())
                        .font(cosmic::font::bold())
                        .size(24),
                )
                .push(quiet_text("Usage is not available for this drive."));
        }
        let warning = drive.warning();
        let mut details = column![
            container(summary)
                .padding(16)
                .width(Length::Fill)
                .style(move |theme| card_style(theme, warning, true)),
        ]
        .spacing(16);
        if warning {
            details = details.push(notice("Drive usage is at or above 80%.".into()));
        }
        if drive.status == DriveStatus::Partial {
            details = details.push(notice("Some mounted filesystems could not be read. The numbers include readable filesystems only.".into()));
        }
        if drive.shared {
            details = details.push(quiet_text(
                "Shared filesystem · totals include all member drives.",
            ));
        }
        let mut filesystems = column![
            row![
                widget::text::heading("Filesystems"),
                widget::space::horizontal(),
                quiet_text(format!("{} detected", drive.filesystems.len())),
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(10);
        if drive.filesystems.is_empty() {
            filesystems = filesystems.push(quiet_text("No mounted or recognized filesystem."));
        }
        for filesystem in &drive.filesystems {
            filesystems = filesystems.push(filesystem_view(filesystem));
        }
        let disclosure = button::custom(
            row![
                widget::text::heading("Details"),
                widget::space::horizontal(),
                quiet_text("Device, mounts & calculation"),
                icon::from_name(if self.details_expanded {
                    "pan-up-symbolic"
                } else {
                    "pan-down-symbolic"
                })
                .size(16)
                .icon(),
            ]
            .spacing(8)
            .align_y(Alignment::Center),
        )
        .class(cosmic::theme::Button::Text)
        .padding([10, 12])
        .width(Length::Fill)
        .on_press(Message::ToggleDetails);
        details = details
            .push(filesystems)
            .push(widget::divider::horizontal::default())
            .push(disclosure);
        if self.details_expanded {
            details = details.push(technical_details(drive));
        }
        details.into()
    }
}

fn filesystem_view(filesystem: &Filesystem) -> Element<'_, Message> {
    let warning = filesystem.usage.is_some_and(Usage::warning);
    let mut details = column![
        column![
            text(filesystem_title(filesystem))
                .font(cosmic::font::bold())
                .size(14),
            quiet_text(format!(
                "{} · {}{}",
                filesystem.filesystem_type,
                filesystem.status.label(),
                if filesystem.physical_drives.len() > 1 {
                    " · Shared"
                } else {
                    ""
                },
            )),
        ]
        .spacing(3),
    ]
    .spacing(12);
    if let Some(usage) = filesystem.usage {
        details = details.push(usage_view(usage, false));
        if warning {
            details = details.push(
                row![
                    icon(icons::warning()).size(16),
                    text("At or above 80% used").size(12)
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            );
        }
    } else {
        details = details.push(quiet_text("Usage unavailable"));
    }
    container(details)
        .padding(14)
        .width(Length::Fill)
        .style(move |theme| card_style(theme, warning, false))
        .into()
}

fn usage_view(usage: Usage, prominent: bool) -> Element<'static, Message> {
    let warning = usage.warning();
    let bar = progress_bar(0.0..=100.0, usage.percent() as f32)
        .girth(if prominent { 8 } else { 6 })
        .class(cosmic::theme::ProgressBar::custom(move |theme: &Theme| {
            progress_bar::Style {
                background: Color::from(theme.cosmic().bg_component_divider()).into(),
                bar: Color::from(if warning {
                    theme.cosmic().warning_color()
                } else {
                    theme.cosmic().accent_color()
                })
                .into(),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
            }
        }));
    let figure_size = if prominent { 32 } else { 22 };
    let figure_style = if warning {
        cosmic::theme::Text::Custom(|theme| text::Style {
            color: Some(theme.cosmic().warning_text_color().into()),
            ..Default::default()
        })
    } else {
        cosmic::theme::Text::Accent
    };
    let figures = row![
        column![
            quiet_text("Used"),
            text(format_bytes(usage.used_bytes))
                .size(figure_size)
                .font(cosmic::font::bold()),
        ]
        .spacing(2)
        .width(Length::FillPortion(3)),
        column![
            quiet_text("Utilization"),
            text(usage.percent_label())
                .size(figure_size)
                .font(cosmic::font::bold())
                .class(figure_style),
        ]
        .spacing(2)
        .align_x(Alignment::End)
        .width(Length::FillPortion(2)),
    ]
    .spacing(12)
    .align_y(Alignment::Start);
    let capacities = row![
        column![
            quiet_text("Total capacity"),
            text(format_bytes(usage.total_bytes))
                .font(cosmic::font::semibold())
                .size(14),
        ]
        .spacing(2)
        .width(Length::Fill),
        column![
            quiet_text("Available to apps"),
            text(
                usage
                    .available_bytes
                    .map_or_else(|| "Unknown".into(), format_bytes)
            )
            .font(cosmic::font::semibold())
            .size(14),
        ]
        .spacing(2)
        .align_x(Alignment::End)
        .width(Length::Fill),
    ]
    .spacing(12);
    column![figures, bar, capacities]
        .spacing(if prominent { 12 } else { 8 })
        .into()
}

fn filesystem_title(filesystem: &Filesystem) -> String {
    filesystem
        .label
        .clone()
        .unwrap_or_else(|| filesystem.devices.join(", "))
}

fn technical_details(drive: &Drive) -> Element<'_, Message> {
    let mut details = column![detail_field("Device", &drive.path)].spacing(12);
    if let Some(model) = &drive.model {
        details = details.push(detail_field("Model", model));
    }
    details = details
        .push(detail_field(
            "Raw physical capacity",
            format_bytes(drive.raw_capacity_bytes),
        ))
        .push(widget::divider::horizontal::default())
        .push(widget::text::heading("Mountpoints"));
    if drive.filesystems.is_empty() {
        details = details.push(quiet_text("No recognized filesystems."));
    }
    for filesystem in &drive.filesystems {
        let mut mounts = column![
            text(filesystem_title(filesystem))
                .font(cosmic::font::semibold())
                .size(13),
        ]
        .spacing(4);
        if filesystem.label.is_some() {
            mounts = mounts.push(quiet_text(filesystem.devices.join(", ")));
        }
        mounts = mounts.push(
            text(if filesystem.mountpoints.is_empty() {
                "Not mounted".into()
            } else {
                filesystem.mountpoints.join("\n")
            })
            .size(13),
        );
        if filesystem.physical_drives.len() > 1 {
            mounts = mounts.push(quiet_text(format!(
                "Shared by {}",
                filesystem.physical_drives.join(", ")
            )));
        }
        details = details.push(mounts);
    }
    details = details
        .push(widget::divider::horizontal::default())
        .push(widget::text::heading("How usage is calculated"))
        .push(quiet_text("Total capacity includes mounted filesystems only. Unmounted partitions, swap and unallocated space are excluded. Btrfs subvolumes and repeated mounts are counted once."))
        .push(quiet_text("Used space follows the kernel's filesystem usage. Available space is what applications can use; reserved blocks can make it smaller than total minus used."))
        .push(quiet_text("Units are binary: GiB = 1024³ bytes. Panel G means GiB. Amounts under 10 keep one decimal place; larger amounts are whole numbers. Percentages are truncated to one decimal; the ≥80% warning uses exact bytes."));
    if drive.shared {
        details = details.push(quiet_text("Shared filesystems show their complete usage on each physical member, not a per-disk allocation. Do not add these shared figures across drives."));
    }
    container(details)
        .padding(14)
        .width(Length::Fill)
        .style(|theme| card_style(theme, false, false))
        .into()
}

fn detail_field<'a>(label: &'a str, value: impl Into<Cow<'a, str>> + 'a) -> Element<'a, Message> {
    column![quiet_text(label), widget::text::body(value)]
        .spacing(3)
        .into()
}

fn quiet_text<'a>(
    label: impl Into<Cow<'a, str>> + 'a,
) -> widget::text::Text<'a, Theme, cosmic::Renderer> {
    widget::text::caption(label).class(cosmic::theme::Text::Custom(|theme| {
        let mut color: Color = theme.cosmic().on_bg_color().into();
        color.a = 0.75;
        text::Style {
            color: Some(color),
            ..Default::default()
        }
    }))
}

fn card_style(theme: &Theme, warning: bool, prominent: bool) -> container::Style {
    let cosmic = theme.cosmic();
    let (background, border) = if warning || prominent {
        let mut tint: Color = if warning {
            cosmic.warning_color()
        } else {
            cosmic.accent_color()
        }
        .into();
        tint.a = if warning { 0.12 } else { 0.08 };
        let mut border = tint;
        border.a = if warning { 0.5 } else { 0.22 };
        (tint, border)
    } else {
        (
            cosmic.bg_component_color().into(),
            cosmic.bg_component_divider().into(),
        )
    };
    let foreground: Color = if warning || prominent {
        cosmic.on_bg_color()
    } else {
        cosmic.on_bg_component_color()
    }
    .into();
    container::Style {
        text_color: Some(foreground),
        icon_color: Some(foreground),
        background: Some(background.into()),
        border: Border {
            color: border,
            width: 1.0,
            radius: if prominent {
                cosmic.corner_radii.radius_m.map(|radius| radius.max(12.0))
            } else {
                cosmic.corner_radii.radius_s.map(|radius| radius.max(8.0))
            }
            .into(),
        },
        ..Default::default()
    }
}

fn notice(message: String) -> Element<'static, Message> {
    container(row![icon(icons::warning()).size(18), text(message).size(13)].spacing(8))
        .padding(10)
        .style(|theme| warning_style(theme, true))
        .into()
}

fn warning_style(theme: &Theme, warning: bool) -> container::Style {
    if !warning {
        return container::Style::default();
    }
    let color: Color = theme.cosmic().warning_text_color().into();
    let mut background: Color = theme.cosmic().warning_color().into();
    background.a = 0.14;
    container::Style {
        text_color: Some(color),
        icon_color: Some(color),
        background: Some(background.into()),
        border: Border {
            color,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

fn popup_limits() -> Limits {
    Limits::NONE
        .width(POPUP_WIDTH)
        .min_height(1.0)
        .max_height(640.0)
}
