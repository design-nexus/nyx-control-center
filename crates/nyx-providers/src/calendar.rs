//! Up next: the coming events from Nyx Calendar's agenda (`agenda.sqlite`, written by
//! `nyx-calendard`: events from yesterday to 60 days out, a published read-only
//! contract). Read when a view appears, whenever the file is replaced, and every minute
//! (so "in 5 min" stays true).

use crate::{MonitorStop, Stops, TimerStop};
use gio::prelude::*;
use nyx_widgets::live::{Live, Setter, Stop};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Event {
    pub title: String,
    pub calendar: String,
    /// `#rrggbb`, when the calendar has a colour.
    pub color: Option<String>,
    pub location: String,
    /// Unix seconds.
    pub starts: i64,
    pub ends: i64,
    pub all_day: bool,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    /// Nyx Calendar's agenda exists (Calendar is installed and has run).
    pub available: bool,
    /// Not over yet, soonest first, at most 8; the same event in several calendars once.
    pub events: Vec<Event>,
}

pub fn agenda_path() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share"));
    data.join("nyx-calendar/agenda.sqlite")
}

/// The events not over by `now`, soonest first.
pub fn read(path: &std::path::Path, now: i64) -> State {
    let Ok(db) = rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY) else {
        return State::default();
    };
    let Ok(mut stmt) = db.prepare(
        "SELECT title, calendar_name, color, location, starts, ends, all_day FROM events \
         WHERE ends > ?1 ORDER BY all_day DESC, starts, title LIMIT 40",
    ) else {
        return State::default();
    };
    let rows = stmt.query_map([now], |r| {
        Ok(Event {
            title: r.get(0)?,
            calendar: r.get(1)?,
            color: r.get::<_, Option<String>>(2)?.filter(|c| c.starts_with('#')),
            location: r.get(3)?,
            starts: r.get(4)?,
            ends: r.get(5)?,
            all_day: r.get::<_, i64>(6)? != 0,
        })
    });
    let mut events: Vec<Event> = Vec::new();
    for e in rows.into_iter().flatten().flatten() {
        // Holidays come from several calendars: one of each.
        if let Some(prev) = events.iter_mut().find(|p| p.title == e.title && p.starts == e.starts) {
            if prev.color.is_none() {
                prev.color = e.color;
            }
            continue;
        }
        events.push(e);
    }
    events.sort_by(|a, b| a.starts.cmp(&b.starts).then(b.all_day.cmp(&a.all_day)));
    events.truncate(8);
    State { available: true, events }
}

thread_local! {
    static LIVE: Live<State> = Live::new(State::default(), start);
}

pub fn live() -> Live<State> {
    LIVE.with(|l| l.clone())
}

fn now() -> i64 {
    glib::real_time() / 1_000_000
}

fn refresh(set: &Setter<State>) {
    let path = agenda_path();
    let st = if path.exists() { read(&path, now()) } else { State::default() };
    set.set(st);
}

fn start(set: Setter<State>) -> Box<dyn Stop> {
    refresh(&set);
    let mut stops: Vec<Box<dyn Stop>> = Vec::new();
    // The file is swapped in by a rename: watch the folder.
    let dir = agenda_path().parent().map(PathBuf::from).unwrap_or_default();
    if let Ok(m) = gio::File::for_path(&dir).monitor_directory(gio::FileMonitorFlags::WATCH_MOVES, gio::Cancellable::NONE) {
        let s = set.clone();
        m.connect_changed(move |_, f, other, _| {
            let hit = |f: Option<&gio::File>| f.and_then(|f| f.basename()).is_some_and(|n| n.as_os_str() == "agenda.sqlite");
            if hit(Some(f)) || hit(other) {
                refresh(&s);
            }
        });
        stops.push(Box::new(MonitorStop(m)));
    }
    let s = set.clone();
    let id = glib::timeout_add_local(Duration::from_secs(60), move || {
        refresh(&s);
        glib::ControlFlow::Continue
    });
    stops.push(Box::new(TimerStop(Some(id))));
    Box::new(Stops(stops))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_agenda() {
        let path = std::env::temp_dir().join(format!("nyx-agenda-{}.sqlite", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE events (calendar TEXT, calendar_name TEXT, color TEXT, uid TEXT, rid TEXT, title TEXT, location TEXT,
                                  description TEXT, url TEXT, starts INTEGER, ends INTEGER, all_day INTEGER, first_day TEXT, last_day TEXT);
             INSERT INTO events VALUES ('a','Holidays',NULL,'1',NULL,'Election Day','','',NULL,1000,87400,1,'','');
             INSERT INTO events VALUES ('b','US Holidays','#8e44ad','2',NULL,'Election Day','','',NULL,1000,87400,1,'','');
             INSERT INTO events VALUES ('c','Work','#3584e4','3',NULL,'Review','Studio B','',NULL,5000,8600,0,'','');
             INSERT INTO events VALUES ('c','Work','#3584e4','4',NULL,'Over','','',NULL,100,200,0,'','');",
        )
        .unwrap();
        drop(db);
        let st = read(&path, 1500);
        let titles: Vec<&str> = st.events.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(titles, vec!["Election Day", "Review"], "one of each holiday, nothing over");
        assert_eq!(st.events[0].color.as_deref(), Some("#8e44ad"), "the colour from whichever calendar has one");
        let _ = std::fs::remove_file(&path);
    }
}
