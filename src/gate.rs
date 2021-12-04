use std::error::Error;
use std::fs::File;
use std::ffi::OsString;
use std::io;
use std::io::{Write};

use encoding_rs::UTF_16LE;
use encoding_rs_io::DecodeReaderBytesBuilder;

use crate::model::{Roster};

pub fn read_roster(path: OsString) -> Result<Roster, Box<dyn Error>> {
    // https://stackoverflow.com/a/53833111
    let fh = File::open(path)?;
    let transcoded = DecodeReaderBytesBuilder::new()
        .encoding(Some(UTF_16LE))
        .build(fh);
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(b'\t')
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(transcoded);
    let mut ub_ids = vec![];
    let mut names = vec![];
    let mut usernames = vec![];
    for r in rdr.records() {
        let res = r?;
        let cur_ub_id = match res.get(3) {
            Some(a) => a,
            None => continue,
        };
        let last_name = match res.get(0) {
            Some(a) => a,
            None => continue,
        };
        let first_name = match res.get(1) {
            Some(a) => a,
            None => continue,
        };
        let username = match res.get(2) {
            Some(a) => a,
            None => continue,
        };
        ub_ids.push(format!("{}", cur_ub_id));
        names.push(format!("{} {}", first_name, last_name));
        usernames.push(format!("{}", username));
        //println!("{:?}", res);
    }
    let roster = Roster::new(
        ub_ids,
        names,
        usernames,
    );
    Ok(roster)
}

pub fn export_summary(rows: Vec<postgres::Row>, outfile: &mut File) -> Result<(), io::Error> {
    let p1_max = rows.iter().map(|a| a.get(1)).fold(i64::MIN, |a, b| a.max(b));
    let p2_max = rows.iter().map(|a| a.get(2)).fold(i64::MIN, |a, b| a.max(b));
    let p3_max = rows.iter().map(|a| a.get(3)).fold(i64::MIN, |a, b| a.max(b));
    // Note that column identifiers are hard-coded here; a more flexible approach might allow for
    // changing them
    let p1_header = format!("Participation 1 [Total Pts: {} Score] |1576192", p1_max);
    let p2_header = format!("Participation 2 [Total Pts: {} Score] |1576193", p2_max);
    let p3_header = format!("Participation 3 [Total Pts: {} Score] |1576194", p3_max);
    let header_line = format!("\"Username\"\t\"{}\"\t\"{}\"\t\"{}\"\n", p1_header, p2_header, p3_header);
    outfile.write_all(header_line.as_bytes())?;
    for row in rows {
        let username: String = row.get(0);
        let p1: i64 = row.get(1);
        let p2: i64 = row.get(2);
        let p3: i64 = row.get(3);
        outfile.write_all(format!("\"{}\"\t{}\t{}\t{}\n", username, p1, p2, p3).as_bytes())?;
    }
    Ok(())
}
