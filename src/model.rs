use chrono::{DateTime, Local};

#[derive(Clone, Debug)]
pub struct Category {
    pub db_id: i32,
    pub name: String,
    first_entered: DateTime<Local>
}

impl Category {
    pub fn new(db_id: i32, name: String, first_entered: DateTime<Local>) -> Category {
        Category { db_id, name, first_entered }
    }
}

#[derive(Clone, Debug)]
pub struct Student {
    pub db_id: i32,
    pub ub_id: String,
    pub name: String,
    first_entered: DateTime<Local>,
    status_id: i32,
    last_updated: DateTime<Local>
}

impl Student {
    pub fn new(db_id: i32, ub_id: String, name: String, first_entered: DateTime<Local>, status_id: i32, last_updated: DateTime<Local>) -> Student {
        Student { db_id, ub_id, name, first_entered, status_id, last_updated }
    }
}

pub struct Roster {
    ub_ids: Vec<String>,
    names: Vec<String>,
    usernames: Vec<String>,
}

impl Roster {
    pub fn new(ub_ids: Vec<String>, names: Vec<String>, usernames: Vec<String>) -> Roster {
        Roster { ub_ids: ub_ids, names: names, usernames: usernames }
    }

    pub fn iter(&self) -> RosterIterator {
        RosterIterator {
            inner: self,
            pos: 0
        }
    }
}

pub struct RosterIterator<'a> {
    inner: &'a Roster,
    pos: usize,
}

impl<'a> Iterator for RosterIterator<'a> {
    type Item = (&'a String, &'a String, &'a String);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos >= self.inner.ub_ids.len() || self.pos >= self.inner.names.len() || self.pos >= self.inner.usernames.len() {
            None
        } else {
            let cur_pos = self.pos;
            self.pos += 1;
            Some((
                self.inner.ub_ids.get(cur_pos).unwrap(),
                self.inner.names.get(cur_pos).unwrap(),
                self.inner.usernames.get(cur_pos).unwrap()
            ))
        }
    }
}
