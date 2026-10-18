//! Calendar sync settings (CA-1, CA-2): which login and which calendars. The login is the one of the graph's
//! sync unless the graph is given one of its own. The sync engine itself is in `notatnik-caldav`.

use notatnik_caldav::{CalDavClient, HttpCalDav};
use notatnik_graph::{CalendarAccount, CalendarConfig};

use super::syncing::{api_err, secret_key};
use super::*;

fn password_key(graph: &str) -> String {
    format!("calendar {graph}")
}

/// What it takes to talk to the calendar server.
struct Account {
    server: String,
    user: String,
    password: String,
    own: bool,
}

impl Account {
    fn view(&self) -> CalendarAccountView {
        CalendarAccountView {
            server: self.server.clone(),
            user: self.user.clone(),
            own: self.own,
        }
    }
}

impl AppState {
    fn calendar_settings(&self, graph: &str) -> CalendarConfig {
        notatnik_graph::DeviceSettings::load(&self.dirs)
            .ok()
            .and_then(|d| d.calendar_of(graph))
            .unwrap_or_default()
    }

    fn store_calendar_settings(&self, graph: &str, cfg: CalendarConfig) -> ApiResult<()> {
        let mut dev = notatnik_graph::DeviceSettings::load(&self.dirs).unwrap_or_default();
        dev.set_calendar(graph, Some(cfg));
        dev.save(&self.dirs)?;
        Ok(())
    }

    /// The login for the calendar server of a graph: its own, else the one of its sync.
    fn calendar_account(&self, graph: &str, cfg: &CalendarConfig) -> ApiResult<Option<Account>> {
        let no_password = || {
            ApiError::new(
                "auth",
                "no password is stored for the calendar login; connect again and enter it",
            )
        };
        if let Some(a) = &cfg.account {
            let password = self
                .secrets
                .get(&password_key(graph))
                .ok_or_else(no_password)?;
            return Ok(Some(Account {
                server: a.server.clone(),
                user: a.user.clone(),
                password,
                own: true,
            }));
        }
        let Some(remote) = self.remote_of(graph) else {
            return Ok(None);
        };
        let password = self
            .secrets
            .get(&secret_key(&remote))
            .ok_or_else(no_password)?;
        Ok(Some(Account {
            server: remote.url,
            user: remote.user,
            password,
            own: false,
        }))
    }

    fn calendar_client_for(&self, a: &Account) -> ApiResult<Arc<dyn CalDavClient + Send + Sync>> {
        if let Some(c) = &self.calendar_client {
            return Ok(c.clone());
        }
        let client = HttpCalDav::new(&a.server, &a.user, &a.password).map_err(|e| api_err(&e))?;
        Ok(Arc::new(client))
    }

    fn calendar_list(client: &dyn CalDavClient) -> ApiResult<Vec<CalendarView>> {
        Ok(client
            .calendars()
            .map_err(|e| api_err(&e))?
            .into_iter()
            .map(|c| CalendarView {
                tasks: c.takes("VTODO"),
                events: c.takes("VEVENT"),
                href: c.href,
                name: c.name,
            })
            .collect())
    }

    fn calendar_status_of(&self, graph: &str) -> ApiResult<CalendarStatus> {
        let cfg = self.calendar_settings(graph);
        let account = match &cfg.account {
            Some(a) => Some(CalendarAccountView {
                server: a.server.clone(),
                user: a.user.clone(),
                own: true,
            }),
            None => self.remote_of(graph).map(|r| CalendarAccountView {
                server: r.url,
                user: r.user,
                own: false,
            }),
        };
        let section = self.with_open(|o| Ok(o.graph.config().calendar.clone()))?;
        Ok(CalendarStatus {
            account,
            enabled: cfg.enabled,
            read: cfg.read,
            tasks: cfg.tasks,
            tasks_as: section.tasks_as,
            on_delete: section.on_delete,
        })
    }

    /// `calendar_status`: the calendar settings of the open graph.
    pub fn calendar_status(&self, _: NoArgs) -> ApiResult<CalendarStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        self.calendar_status_of(&id)
    }

    /// `calendar_connect`: logs in and lists the calendars. With no arguments the login of the graph's sync is
    /// used; with a server, user and password the graph gets a login of its own (kept only if it works).
    pub fn calendar_connect(&self, a: CalendarConnectArgs) -> ApiResult<CalendarList> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut cfg = self.calendar_settings(&id);
        let given = [&a.server, &a.user, &a.password]
            .iter()
            .any(|v| v.as_deref().is_some_and(|s| !s.trim().is_empty()));
        if !given {
            let account = self.calendar_account(&id, &cfg)?.ok_or_else(|| {
                ApiError::new(
                    "invalid",
                    "this graph is not synced with a server; enter the server and your login",
                )
            })?;
            let calendars = Self::calendar_list(&*self.calendar_client_for(&account)?)?;
            return Ok(CalendarList {
                account: account.view(),
                calendars,
            });
        }
        let field = |v: &Option<String>, what: &str| -> ApiResult<String> {
            v.as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| ApiError::new("invalid", format!("enter the {what}")))
        };
        let account = Account {
            server: field(&a.server, "server address")?,
            user: field(&a.user, "user name")?,
            password: a
                .password
                .clone()
                .filter(|p| !p.is_empty())
                .ok_or_else(|| ApiError::new("invalid", "enter the password"))?,
            own: true,
        };
        let calendars = Self::calendar_list(&*self.calendar_client_for(&account)?)?;
        self.secrets
            .set(&password_key(&id), &account.password)
            .map_err(|e| {
                ApiError::new("server", format!("the password could not be stored: {e}"))
            })?;
        cfg.account = Some(CalendarAccount {
            server: account.server.clone(),
            user: account.user.clone(),
        });
        self.store_calendar_settings(&id, cfg)?;
        Ok(CalendarList {
            account: account.view(),
            calendars,
        })
    }

    /// `calendar_save`: turns the sync on or off and chooses the calendars. The task calendar must take tasks.
    pub fn calendar_save(&self, a: CalendarSaveArgs) -> ApiResult<CalendarStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut cfg = self.calendar_settings(&id);
        let status = self.calendar_status_of(&id)?;
        if a.enabled && a.tasks.is_none() && a.read.is_empty() {
            return Err(ApiError::new(
                "invalid",
                "choose a calendar for tasks or calendars to read events from",
            ));
        }
        if let Some(href) = &a.tasks {
            let account = self
                .calendar_account(&id, &cfg)?
                .ok_or_else(|| ApiError::new("invalid", "connect to a calendar server first"))?;
            let list = Self::calendar_list(&*self.calendar_client_for(&account)?)?;
            let cal = list
                .iter()
                .find(|c| &c.href == href)
                .ok_or_else(|| ApiError::new("not_found", "that calendar is not on the server"))?;
            let takes = if status.tasks_as == "vevent" {
                cal.events
            } else {
                cal.tasks
            };
            if !takes {
                return Err(ApiError::new(
                    "invalid",
                    format!(
                        "the calendar `{}` does not take {}; choose another",
                        cal.name,
                        if status.tasks_as == "vevent" {
                            "events"
                        } else {
                            "tasks"
                        }
                    ),
                ));
            }
        }
        cfg.enabled = a.enabled;
        cfg.read = a.read;
        cfg.tasks = a.tasks;
        self.store_calendar_settings(&id, cfg)?;
        self.calendar_status_of(&id)
    }

    /// `calendar_forget`: removes the graph's own calendar login (the sync login stays) and turns the sync off.
    pub fn calendar_forget(&self, _: NoArgs) -> ApiResult<CalendarStatus> {
        let id = self.with_open(|o| Ok(o.graph.id()))?;
        let mut cfg = self.calendar_settings(&id);
        cfg.account = None;
        cfg.enabled = false;
        self.secrets.delete(&password_key(&id));
        self.store_calendar_settings(&id, cfg)?;
        self.calendar_status_of(&id)
    }
}
