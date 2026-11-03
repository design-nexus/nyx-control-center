//! A view of Backup's versioned, grant-scoped status. No Backup internals are imported.
use crate::{
    tiles::{Ctx, frame},
    ui::{act, bind, icon, icon_button, label},
};
use gtk::prelude::*;
use nyx_providers::backup;
use nyx_widgets::layout::{Size, Tile};
pub fn build(_: &Ctx, size: Size, _: &Tile) -> gtk::Widget {
    let b = frame(size);
    let head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    head.append(&icon("drive-harddisk-symbolic", 20));
    let name = label("Backup", "cc-title");
    head.append(&name);
    let open = icon_button("window-new-symbolic", "Open Backup");
    open.connect_clicked(|_| nyx_providers::spawn(&["nyx-backup"]));
    head.append(&open);
    b.append(&head);
    let status = label("Waiting for Backup", "cc-subtitle");
    b.append(&status);
    let last = label("No successful backup recorded", "cc-subtitle");
    b.append(&last);
    let schedule = label("", "cc-subtitle");
    b.append(&schedule);
    let progress = gtk::ProgressBar::new();
    b.append(&progress);
    let actions = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let run = icon_button("media-playback-start-symbolic", "Start the granted backup profile");
    let cancel = icon_button("process-stop-symbolic", "Cancel this profile's active job");
    actions.append(&run);
    actions.append(&cancel);
    b.append(&actions);
    run.connect_clicked(|_| {
        let s = backup::live().get();
        if let Some(g) = s.grant {
            act(async move { backup::action(&g, "backup.start", None).await })
        }
    });
    cancel.connect_clicked(|_| {
        let s = backup::live().get();
        if let (Some(g), Some(j)) = (s.grant, s.active) {
            act(async move { backup::action(&g, "backup.cancel", Some(&j)).await })
        }
    });
    bind(&b, backup::live(), move |s| {
        name.set_text(if s.name.is_empty() { "Backup" } else { &s.name });
        status.set_text(s.problem.as_deref().unwrap_or(match s.health.as_str() {
            "working" => "Backup job running",
            "protected" => "Last backup succeeded",
            "attention" => "Last backup needs attention",
            "destination_unavailable" => "Destination unavailable",
            _ => "No successful backup recorded",
        }));
        last.set_text(
            &s.last_success
                .and_then(|t| gtk::glib::DateTime::from_unix_local(t).ok())
                .and_then(|d| d.format("Last success: %x %X").ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "No successful backup recorded".into()),
        );
        schedule.set_text(
            &s.next_schedule
                .and_then(|t| gtk::glib::DateTime::from_unix_local(t).ok())
                .and_then(|d| d.format("Next: %x %X").ok())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    if s.schedule.is_some() {
                        "Schedule saved; no active next run reported".into()
                    } else {
                        "No schedule configured".into()
                    }
                }),
        );
        progress.set_visible(s.active.is_some());
        progress.set_fraction(s.fraction.unwrap_or(0.));
        run.set_sensitive(s.can_backup && s.active.is_none() && s.problem.is_none());
        cancel.set_sensitive(s.can_cancel && s.active.is_some() && s.problem.is_none());
    });
    b.upcast()
}
