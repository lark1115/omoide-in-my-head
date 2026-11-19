use crate::{
    args::{HotCommand, HotFile},
    db::*,
};
use fs2::FileExt;
use omoide_store::{Error, Sqlite, Store};
use serde_json::{Value as Json, json};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    path::Path,
};
use tempfile::NamedTempFile;

const BEGIN: &str = "<!-- omoide:begin -->";
const END: &str = "<!-- omoide:end -->";
const MAGIC: &str = "<!-- omoide:meta ";
fn io(e: std::io::Error) -> Error {
    Error::Unknown(e.to_string())
}

fn section(text: &str) -> Result<Option<(usize, usize)>> {
    enum Region {
        Before,
        Open(usize),
        Closed(usize, usize),
    }
    let mut region = Region::Before;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        if bare.contains("<!-- omoide") {
            region = match (region, bare) {
                (Region::Before, BEGIN) => Region::Open(offset),
                (Region::Open(start), END) => Region::Closed(start, offset + line.len()),
                (Region::Open(start), line) if line.starts_with(MAGIC) => Region::Open(start),
                _ => return Err(Error::Input("damaged or duplicated section marker".into())),
            };
        }
        offset += line.len();
    }
    match region {
        Region::Before => Ok(None),
        Region::Open(_) => Err(Error::Input("incomplete section markers".into())),
        Region::Closed(a, b) => Ok(Some((a, b))),
    }
}
fn header(meta: &[omoide_store::Value], cap: usize) -> Result<Json> {
    Ok(
        json!({"store":text(&meta[0])?,"revision":int(&meta[1])?,"usage":int(&meta[2])?,"config":digest(format!("hot-lines-v1/{cap}"))}),
    )
}
fn quote(v: &str) -> String {
    // Escape line breaks and markup so a stored instruction remains one data line.
    json!(v)
        .to_string()
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}
fn render(h: &Json, rows: &omoide_store::Rows, cap: usize) -> Result<(String, Vec<Json>)> {
    let mut result = format!("{BEGIN}\n{MAGIC}{h} -->\n");
    let mut optional = Vec::new();
    let mut included = Vec::new();
    for row in rows {
        let record = parse(&row[0])?;
        let actions = record["actions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Json::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        let actions = if actions.is_empty() {
            "（動作名未登録）"
        } else {
            &actions
        };
        let mut line = format!(
            "- 場面 {}、動作 {} の前に {}。",
            quote(record["scene"].as_str().unwrap()),
            quote(actions),
            quote(record["text"].as_str().unwrap())
        );
        for (field, label) in [
            ("target", "対象"),
            ("exclusion", "除外"),
            ("reference", "参照"),
        ] {
            if let Some(value) = record[field].as_str() {
                line.push_str(&format!(" {label} {}", quote(value)));
            }
        }
        line.push('\n');
        let identity = json!({"id":record["id"],"version_id":record["version_id"]});
        if record["pinned"] == 1 {
            included.push(identity);
            result.push_str(&line)
        } else {
            optional.push((line, identity))
        }
    }
    let required = result.len() + END.len() + 1;
    if required > cap {
        return Err(Error::Input(format!(
            "section exceeds cap by {} bytes",
            required - cap
        )));
    }
    for (line, identity) in optional {
        if result.len() + line.len() + END.len() < cap {
            included.push(identity);
            result.push_str(&line)
        }
    }
    result.push_str(END);
    result.push('\n');
    Ok((result, included))
}
fn stored_header(part: &str) -> Result<Json> {
    let mut lines = part.lines().filter(|l| l.starts_with(MAGIC));
    let raw = lines
        .next()
        .and_then(|l| l.strip_prefix(MAGIC))
        .and_then(|s| s.strip_suffix(" -->"))
        .ok_or_else(|| Error::Input("damaged section header".into()))?;
    if lines.next().is_some() {
        return Err(Error::Input("duplicate section header".into()));
    }
    let h: Json =
        serde_json::from_str(raw).map_err(|_| Error::Input("invalid section header".into()))?;
    if !h["store"].is_string()
        || !h["revision"].is_u64()
        || !h["usage"].is_u64()
        || !h["config"].is_string()
    {
        return Err(Error::Input("incomplete section header".into()));
    }
    Ok(h)
}
#[cfg(any(test, feature = "fault-injection"))]
fn stop_for_test(attempt: u32, armed: bool) -> bool {
    attempt == 0 && armed
}
pub fn run(db: &mut Sqlite, cmd: &HotCommand, db_path: &Path) -> Result<Json> {
    let (HotFile { file, limit_bytes }, init, status) = match cmd {
        HotCommand::Init(f) => (f, true, false),
        HotCommand::Export(f) => (f, false, false),
        HotCommand::Status(f) => (f, false, true),
    };
    let file = match file.canonicalize() {
        Ok(f) => f,
        Err(e) if e.kind() == ErrorKind::NotFound && init => file.to_path_buf(),
        Err(e) => return Err(io(e)),
    };
    if db_path.canonicalize().ok().as_ref() == Some(&file) {
        return Err(Error::Input("section file cannot be the database".into()));
    }
    let mut lockname = file.as_os_str().to_owned();
    lockname.push(".omoide.lock");
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lockname)
        .map_err(io)?;
    lock.lock_exclusive().map_err(io)?;
    let original = match fs::read_to_string(&file) {
        Ok(s) => s,
        Err(e) if init && e.kind() == ErrorKind::NotFound => String::new(),
        Err(e) => return Err(io(e)),
    };
    let span = section(&original)?;
    let previous = span
        .map(|(s, e)| stored_header(&original[s..e]))
        .transpose()?;
    if !init && span.is_none() {
        return Err(Error::Input("file has no omoide section".into()));
    }
    let metadata = "SELECT store_id,revision,usage_revision FROM meta WHERE singleton=1";
    for _attempt in 0..12 {
        let sets=db.read_batch(&[q(metadata,vec![]),q(format!("SELECT {} FROM rules r WHERE state='current' ORDER BY pinned DESC,(SELECT count(*) FROM notices WHERE kind='used' AND version=r.version) DESC,created DESC,r.version",omoide_store::lookup::RULE),vec![])])?;
        let h = header(&sets[0][0], *limit_bytes)?;
        if status || (init && previous.is_some()) {
            return Ok(
                json!({"state":if previous.as_ref()==Some(&h){"current"}else{"stale"},"stored":previous,"current":h}),
            );
        }
        let (block, included) = render(&h, &sets[1], *limit_bytes)?;
        let updated = match span {
            Some((s, e)) => format!("{}{}{}", &original[..s], block, &original[e..]),
            None => format!(
                "{}{}{block}",
                original,
                if original.is_empty() || original.ends_with('\n') {
                    ""
                } else {
                    "\n"
                }
            ),
        };
        let dir = file
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let mut tmp = NamedTempFile::new_in(dir).map_err(io)?;
        if let Ok(m) = fs::metadata(&file) {
            tmp.as_file().set_permissions(m.permissions()).map_err(io)?;
        }
        tmp.write_all(updated.as_bytes()).map_err(io)?;
        tmp.as_file().sync_all().map_err(io)?;
        #[cfg(all(unix, feature = "fault-injection"))]
        if stop_for_test(_attempt, std::env::var_os("OMOIDE_TEST_STOP_HOT").is_some()) {
            unsafe {
                libc::raise(libc::SIGSTOP);
            }
        }
        if db.publish_if_current(
            text(&sets[0][0][0])?,
            int(&sets[0][0][1])?,
            int(&sets[0][0][2])?,
            || {
                tmp.persist(&file).map_err(|e| io(e.error))?;
                fs::File::open(dir).and_then(|d| d.sync_all()).map_err(io)
            },
        )? {
            return Ok(
                json!({"state":"current","header":h,"section_bytes":block.len(),"records":included}),
            );
        }
    }
    Err(Error::Busy)
}

#[cfg(test)]
mod tests {
    use super::*;
    // [REQ-HOT-1]
    #[test]
    fn marker_offsets_cover_end_line_and_preserve_the_surrounding_text() {
        let part = format!("{BEGIN}\r\nany\r\n{END}\r\n");
        let text = format!("紙\r\n{part}後");
        let (a, b) = section(&text).unwrap().unwrap();
        assert_eq!(&text[..a], "紙\r\n");
        assert_eq!(&text[a..b], part);
        assert_eq!(&text[b..], "後");
        for invalid in [
            format!("{END}\n{BEGIN}"),
            format!("{BEGIN}\n{END}\n{END}"),
            format!("{BEGIN}\n{END}\n{BEGIN}"),
            format!("{BEGIN}\n{BEGIN}\n{END}"),
        ] {
            assert!(section(&invalid).is_err());
        }
        assert_eq!(section("plain").unwrap(), None);
        let good = json!({"store":"id","revision":0,"usage":0,"config":"hash"});
        for key in ["store", "revision", "usage", "config"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(stored_header(&format!("{BEGIN}\n{MAGIC}{bad} -->\n{END}")).is_err());
        }
        assert_eq!(
            quote("a\n<!-- omoide:end -->"),
            "\"a\\n&lt;!-- omoide:end --&gt;\""
        );
    }
}

#[cfg(test)]
mod byte_budget {
    use super::*;
    // [REQ-HOT-1]
    #[test]
    fn optional_line_obeys_exact_utf8_budget() {
        let record = json!({"id":"i","version_id":"v","text":"紙","pinned":0,"scene":"場","actions":["折る"]});
        let rows = vec![vec![s(&record.to_string())]];
        let full = render(&Json::Null, &rows, 1000).unwrap();
        let cap = full.0.len();
        assert_eq!(render(&Json::Null, &rows, cap).unwrap(), full);
        let partial = render(&Json::Null, &rows, cap - 1).unwrap();
        assert!(partial.1.is_empty());
        assert_eq!(
            partial.0.len(),
            render(&Json::Null, &vec![], cap).unwrap().0.len()
        );
    }
}

#[cfg(test)]
mod hooks {
    // [REQ-HOT-1]
    #[test]
    fn interruption_is_armed_only_for_the_first_attempt() {
        for (attempt, armed, expected) in [
            (0, false, false),
            (1, false, false),
            (1, true, false),
            (0, true, true),
        ] {
            assert_eq!(super::stop_for_test(attempt, armed), expected);
        }
    }
}
