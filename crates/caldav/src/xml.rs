//! Reading the `207 Multi-Status` answers of a CalDAV server.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use crate::{Error, Result};

/// One `<response>`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Resp {
    pub href: String,
    /// The status of the whole response (`404` for an item a sync report says is gone).
    pub status: Option<u16>,
    pub etag: String,
    pub name: String,
    pub is_calendar: bool,
    pub is_collection: bool,
    /// `VEVENT`, `VTODO` ...
    pub components: Vec<String>,
    /// `calendar-data`
    pub data: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Multi {
    pub responses: Vec<Resp>,
    /// `<sync-token>` of a sync report.
    pub sync_token: Option<String>,
    /// `<current-user-principal>`
    pub principal: Option<String>,
    /// `<calendar-home-set>`
    pub home: Option<String>,
}

fn local(e: &BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

fn status_code(line: &str) -> Option<u16> {
    line.split_whitespace().nth(1)?.parse().ok()
}

/// Reads an answer. A malformed one is an error; unknown elements are skipped.
pub fn parse(xml: &str) -> Result<Multi> {
    let mut reader = Reader::from_str(xml);
    let mut out = Multi::default();
    let mut cur: Option<Resp> = None;
    let mut path: Vec<String> = Vec::new();
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let name = local(&e);
                if name == "response" {
                    cur = Some(Resp::default());
                }
                path.push(name);
                text.clear();
            }
            Ok(Event::Empty(e)) => {
                let name = local(&e);
                let parent = path.last().map(String::as_str).unwrap_or("");
                if let Some(c) = cur.as_mut() {
                    match (parent, name.as_str()) {
                        ("resourcetype", "calendar") => c.is_calendar = true,
                        ("resourcetype", "collection") => c.is_collection = true,
                        ("supported-calendar-component-set", "comp") => {
                            for a in e.attributes().flatten() {
                                if a.key.local_name().as_ref() == b"name" {
                                    c.components
                                        .push(String::from_utf8_lossy(&a.value).into_owned());
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let Ok(s) = t.decode() else { continue };
                let s = quick_xml::escape::unescape(&s)
                    .map_or_else(|_| s.to_string(), |c| c.into_owned());
                text.push_str(&s);
            }
            Ok(Event::GeneralRef(r)) => {
                // `&amp;` and the like arrive as events of their own
                let name = String::from_utf8_lossy(&r).into_owned();
                match name.as_str() {
                    "amp" => text.push('&'),
                    "lt" => text.push('<'),
                    "gt" => text.push('>'),
                    "quot" => text.push('"'),
                    "apos" => text.push('\''),
                    n => {
                        let code =
                            n.strip_prefix('#')
                                .and_then(|d| match d.strip_prefix(['x', 'X']) {
                                    Some(h) => u32::from_str_radix(h, 16).ok(),
                                    None => d.parse().ok(),
                                });
                        text.extend(code.and_then(char::from_u32));
                    }
                }
            }
            Ok(Event::CData(c)) => text.push_str(&String::from_utf8_lossy(&c)),
            Ok(Event::End(e)) => {
                let name = String::from_utf8_lossy(e.local_name().as_ref()).into_owned();
                path.pop();
                let parent = path.last().map(String::as_str).unwrap_or("");
                let value = text.trim().to_string();
                match (parent, name.as_str()) {
                    ("response", "href") => {
                        if let Some(c) = cur.as_mut() {
                            c.href = value;
                        }
                    }
                    ("response", "status") => {
                        if let Some(c) = cur.as_mut() {
                            c.status = status_code(&value);
                        }
                    }
                    ("current-user-principal", "href") => out.principal = Some(value),
                    ("calendar-home-set", "href") => out.home = Some(value),
                    ("multistatus", "sync-token") => out.sync_token = Some(value),
                    (_, "getetag") => {
                        if let Some(c) = cur.as_mut() {
                            c.etag = notatnik_sync::norm_etag(&value);
                        }
                    }
                    (_, "displayname") => {
                        if let Some(c) = cur.as_mut() {
                            c.name = value;
                        }
                    }
                    (_, "calendar-data") => {
                        if let Some(c) = cur.as_mut() {
                            // keep the text as sent: no trimming of its line breaks
                            c.data = Some(text.clone());
                        }
                    }
                    _ => {}
                }
                if name == "response" {
                    out.responses.extend(cur.take());
                }
                text.clear();
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(Error::Server(format!("unreadable answer: {e}"))),
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendars_of_a_nextcloud_account() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:cal="urn:ietf:params:xml:ns:caldav" xmlns:cs="http://calendarserver.org/ns/">
 <d:response><d:href>/remote.php/dav/calendars/notatkiA/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/calendars/notatkiA/personal/</d:href>
  <d:propstat><d:prop><d:displayname>Personal</d:displayname>
   <d:resourcetype><d:collection/><cal:calendar/></d:resourcetype>
   <cal:supported-calendar-component-set><cal:comp name="VEVENT"/><cal:comp name="VTODO"/></cal:supported-calendar-component-set>
  </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/calendars/notatkiA/inbox/</d:href>
  <d:propstat><d:prop><d:resourcetype><d:collection/><cal:schedule-inbox/></d:resourcetype></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
</d:multistatus>"#;
        let m = parse(xml).unwrap();
        assert_eq!(m.responses.len(), 3);
        let cals: Vec<&Resp> = m.responses.iter().filter(|r| r.is_calendar).collect();
        assert_eq!(cals.len(), 1);
        assert_eq!(cals[0].name, "Personal");
        assert_eq!(cals[0].components, ["VEVENT", "VTODO"]);
        assert_eq!(cals[0].href, "/remote.php/dav/calendars/notatkiA/personal/");
    }

    #[test]
    fn principal_and_home_set() {
        let xml = r#"<d:multistatus xmlns:d="DAV:" xmlns:c="urn:ietf:params:xml:ns:caldav"><d:response>
<d:href>/remote.php/dav/principals/users/notatkiA/</d:href><d:propstat><d:prop>
<c:calendar-home-set><d:href>/remote.php/dav/calendars/notatkiA/</d:href></c:calendar-home-set>
<d:current-user-principal><d:href>/remote.php/dav/principals/users/notatkiA/</d:href></d:current-user-principal>
</d:prop></d:propstat></d:response></d:multistatus>"#;
        let m = parse(xml).unwrap();
        assert_eq!(
            m.home.as_deref(),
            Some("/remote.php/dav/calendars/notatkiA/")
        );
        assert_eq!(
            m.principal.as_deref(),
            Some("/remote.php/dav/principals/users/notatkiA/")
        );
    }

    #[test]
    fn a_sync_report_has_changed_and_deleted_items_and_a_token() {
        let xml = r#"<d:multistatus xmlns:d="DAV:">
<d:response><d:href>/c/a.ics</d:href><d:propstat><d:prop><d:getetag>"abc"</d:getetag></d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
<d:response><d:href>/c/gone.ics</d:href><d:status>HTTP/1.1 404 Not Found</d:status></d:response>
<d:sync-token>http://sabre.io/ns/sync/42</d:sync-token></d:multistatus>"#;
        let m = parse(xml).unwrap();
        assert_eq!(m.sync_token.as_deref(), Some("http://sabre.io/ns/sync/42"));
        assert_eq!(m.responses[0].etag, "abc");
        assert_eq!(m.responses[0].status, None);
        assert_eq!(m.responses[1].status, Some(404));
    }

    #[test]
    fn calendar_data_keeps_its_text_and_entities_are_decoded() {
        let xml = "<d:multistatus xmlns:d=\"DAV:\" xmlns:c=\"urn:ietf:params:xml:ns:caldav\"><d:response><d:href>/c/a.ics</d:href><d:propstat><d:prop><d:getetag>\"e\"</d:getetag><c:calendar-data>BEGIN:VCALENDAR\r\nSUMMARY:a &amp; b\r\nEND:VCALENDAR\r\n</c:calendar-data></d:prop></d:propstat></d:response></d:multistatus>";
        let m = parse(xml).unwrap();
        assert_eq!(
            m.responses[0].data.as_deref(),
            Some("BEGIN:VCALENDAR\r\nSUMMARY:a & b\r\nEND:VCALENDAR\r\n")
        );
    }

    #[test]
    fn broken_xml_is_a_server_error() {
        assert!(matches!(
            parse("<d:multistatus><d:response></d:multistatus>"),
            Err(Error::Server(_))
        ));
    }
}
