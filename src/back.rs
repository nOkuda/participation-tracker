use std::collections::HashSet;
use std::iter::FromIterator;
use std::process;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use chrono::{Date, Local, TimeZone};
use postgres;
use rand;
use rand::seq::SliceRandom;

use crate::model::{Category, Roster, Student};

pub fn get_student_picker(students: Rc<Vec<Student>>) -> StudentPicker {
    StudentPicker::new(students)
}

pub fn get_event_recorder(client: Arc<Mutex<postgres::Client>>, schema: &str) -> EventRecorder {
    EventRecorder::new(client, schema)
}

pub struct StudentPicker {
    students: Rc<Vec<Student>>,
    rng: rand::rngs::ThreadRng,
    shuffled_indices: Vec<usize>,
    cur_ind: usize
}

impl StudentPicker {
    pub fn new(students: Rc<Vec<Student>>) -> StudentPicker {
        let students_len = students.len();
        StudentPicker {
            students: students,
            rng: rand::thread_rng(),
            shuffled_indices: (0..students_len).collect(),
            cur_ind: 0
        }
    }
}

impl Iterator for StudentPicker {
    type Item = Student;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_ind == self.shuffled_indices.len() {
            self.cur_ind = 0
        }
        if self.cur_ind == 0 {
            self.shuffled_indices.shuffle(&mut self.rng);
        }
        let result: usize = match self.shuffled_indices.get(self.cur_ind) {
            Some(r) => *r,
            None => {
                panic!("Could not get next index; cur_ind = {}; shuffled_indices.len() = {}", self.cur_ind, self.shuffled_indices.len());
            }
        };
        self.cur_ind += 1;
        let student: Student = match self.students.get(result) {
            Some(s) => s.clone(),
            None => {
                panic!("Could not get next student; result = {}; students.len() = {}", result, self.students.len());
            }
        };
        Some(student)
    }
}

pub struct EventRecorder {
    client: Arc<Mutex<postgres::Client>>,
    record_statement: postgres::Statement,
    summarize_statement: postgres::Statement,
    retrieve_statement: postgres::Statement,
    change_statement: postgres::Statement,
}

impl EventRecorder {
    pub fn new(client: Arc<Mutex<postgres::Client>>, schema: &str) -> EventRecorder {
        let record_statement = match client.lock().unwrap().prepare(&format!("
            INSERT INTO {schema}.events (student_id, category_id, satisfactory)
            VALUES (
                (SELECT db_id FROM {schema}.students WHERE name = $1),
                (SELECT db_id FROM {schema}.categories WHERE name = $2),
                $3
            )
        ", schema = schema)) {
            Ok(s) => s,
            Err(e) => {
                println!("Could not prepare event recording statement:");
                println!("{:?}", e);
                process::exit(1);
            }
        };
        let summarize_statement = match client.lock().unwrap().prepare(&format!("
            SELECT
                st.username,
                count(CASE WHEN ev.satisfactory AND st.db_id = ev.student_id AND ev.first_entered < $1 THEN 1 END),
                count(CASE WHEN ev.satisfactory AND st.db_id = ev.student_id AND ev.first_entered >= $1 AND ev.first_entered < $2 THEN 1 END),
                count(CASE WHEN ev.satisfactory AND st.db_id = ev.student_id AND ev.first_entered >= $2 AND ev.first_entered < $3 THEN 1 END)
            FROM {schema}.students as st, {schema}.events as ev
            WHERE st.status_id = (SELECT db_id FROM {schema}.statuses WHERE name = 'enrolled')
            GROUP BY st.ub_id, st.username
        ", schema = schema)) {
            Ok(s) => s,
            Err(e) => {
                println!("Could not prepare summary statement:");
                println!("{:?}", e);
                process::exit(1);
            }
        };
        let retrieve_statement = match client.lock().unwrap().prepare(&format!("
            SELECT
                ev.db_id,
                c.name,
                ev.first_entered,
                ev.satisfactory
            FROM {schema}.categories as c, {schema}.events as ev
            WHERE
                ev.student_id = (SELECT st.db_id FROM {schema}.students as st WHERE st.name = $1) AND
                date_trunc('day', ev.first_entered) <= $2 AND
                $2 < date_trunc('day', ev.first_entered) + interval '1 day' AND
                ev.category_id = c.db_id
            ORDER BY
                ev.first_entered
        ", schema = schema)) {
            Ok(s) => s,
            Err(e) => {
                println!("Could not prepare retrieve statement:");
                println!("{:?}", e);
                process::exit(1);
            }
        };
        let change_statement = match client.lock().unwrap().prepare(&format!("
            UPDATE {schema}.events
                SET satisfactory = $1
                WHERE db_id = $2
        ", schema = schema)) {
            Ok(s) => s,
            Err(e) => {
                println!("Could not prepare change statement:");
                println!("{:?}", e);
                process::exit(1);
            }
        };
        EventRecorder {
            client: client,
            record_statement: record_statement,
            summarize_statement: summarize_statement,
            retrieve_statement: retrieve_statement,
            change_statement: change_statement,
        }
    }

    pub fn record(&mut self, student_name: &str, category_name: &str, satisfactory: bool) -> Result<u64, postgres::Error> {
        self.client.lock().unwrap().execute(&self.record_statement, &[&student_name, &category_name, &satisfactory])
    }

    pub fn get_summary(&mut self) -> Result<Vec<postgres::Row>, postgres::Error> {
        self.client.lock().unwrap().query(
            &self.summarize_statement,
            &[
                &Local.ymd(2021, 10, 1).and_hms(0, 0, 0),
                &Local.ymd(2021, 11, 5).and_hms(0, 0, 0),
                &Local.ymd(2021, 12, 13).and_hms(0, 0, 0)
            ]
        )
    }

    pub fn retrieve_events(&mut self, name: &str, date: &Date<Local>) -> Result<Vec<postgres::Row>, postgres::Error> {
        self.client.lock().unwrap().query(
            &self.retrieve_statement,
            &[
                &name,
                &date.and_hms(0, 0, 0)
            ]
        )
    }

    pub fn change_events(&mut self, changes: &Vec<(bool, i32)>) -> Result<(), postgres::Error> {
        let mut client = self.client.lock().unwrap();
        for (sat, db_id) in changes {
            client.execute(&self.change_statement, &[&sat, &db_id])?;
        }
        Ok(())
    }
}

pub fn update_summary(client: &mut postgres::Client, schema: &str) -> Result<(), postgres::Error> {
    client.batch_execute(&format!("
        UPDATE {schema}.summary s
        SET (points) = (SELECT count(CASE WHEN satisfactory THEN 1 END) FROM {schema}.events h
                        WHERE h.student_id = s.student_id)
    ", schema = schema))?;
    client.batch_execute(&format!("
        UPDATE {schema}.metadata
        SET summary_last_updated = CURRENT_TIMESTAMP
        WHERE db_id = 1
    ", schema = schema))?;

    Ok(())
}

pub fn get_categories(client: &mut postgres::Client, schema: &str) -> Result<Vec<Category>, postgres::Error> {
    // need to prepare a statement for a constructed String
    let statement = client.prepare(&format!("SELECT db_id, name, first_entered FROM {schema}.categories", schema = schema))?;
    let rows = client.query(&statement, &[])?;
    let results = rows.iter()
        .map(|a| Category::new(
                a.get(0),
                a.get(1),
                a.get(2)
                ))
        .collect();
    Ok(results)
}

/// Retrieves Student entities in the database whose status is "enrolled"
pub fn get_students(client: &mut postgres::Client, schema: &str) -> Result<Vec<Student>, postgres::Error> {
    // need to prepare a statement for a constructed String
    let statement = client.prepare(&format!("
        SELECT db_id, ub_id, name, first_entered, status_id, last_updated FROM {schema}.students
        WHERE status_id = (SELECT db_id FROM {schema}.statuses WHERE name = 'enrolled')
    ", schema = schema))?;
    let rows = client.query(&statement, &[])?;
    let results = rows.iter()
        .map(|a| Student::new(
                a.get(0),
                a.get(1),
                a.get(2),
                a.get(3),
                a.get(4),
                a.get(5)
                ))
        .collect();
    Ok(results)
}

pub fn get_db_conn(roster: &Option<Roster>, schema: &str) -> Result<postgres::Client, postgres::Error> {
    let mut client = postgres::Client::connect(
        "postgresql://nozomu@%2Fvar%2Frun%2Fpostgresql/fall2021_latin101",
        postgres::NoTls)?;

    initialize_db(&mut client, roster, schema)?;
    Ok(client)
}

fn initialize_db(client: &mut postgres::Client, roster: &Option<Roster>, schema: &str) -> Result<(), postgres::Error> {
    set_up_tables(client, schema)?;
    insert_starting_data(client, roster, schema)?;

    Ok(())
}

fn set_up_tables(client: &mut postgres::Client, schema: &str) -> Result<(), postgres::Error> {
    client.batch_execute(&format!("
        CREATE SCHEMA IF NOT EXISTS {schema}", schema = schema))?;
    client.batch_execute(&format!("
        CREATE TABLE IF NOT EXISTS {schema}.statuses (
            db_id   INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            name    VARCHAR(15) UNIQUE NOT NULL,
            first_entered   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    ", schema = schema))?;
    client.batch_execute(&format!("
        CREATE TABLE IF NOT EXISTS {schema}.categories (
            db_id   INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            name    VARCHAR(25) UNIQUE NOT NULL,
            first_entered   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    ", schema = schema))?;
    client.batch_execute(&format!("
        CREATE TABLE IF NOT EXISTS {schema}.students (
            db_id   INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            ub_id   VARCHAR(9) UNIQUE NOT NULL,
            name    VARCHAR(100) UNIQUE NOT NULL,
            first_entered   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            status_id   INTEGER NOT NULL REFERENCES {schema}.statuses,
            last_updated    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            username    VARCHAR(30) UNIQUE NOT NULL
        )
    ", schema = schema))?;
    client.batch_execute(&format!("
        CREATE TABLE IF NOT EXISTS {schema}.events (
            db_id   INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            student_id  INTEGER NOT NULL REFERENCES {schema}.students,
            category_id INTEGER NOT NULL REFERENCES {schema}.categories,
            first_entered   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            satisfactory    BOOLEAN NOT NULL
        )
    ", schema = schema))?;
    client.batch_execute(&format!("
        CREATE TABLE IF NOT EXISTS {schema}.summary (
            db_id   INTEGER PRIMARY KEY GENERATED ALWAYS AS IDENTITY,
            student_id  INTEGER UNIQUE NOT NULL REFERENCES {schema}.students,
            points  INTEGER NOT NULL DEFAULT 0
        )
    ", schema = schema))?;
    client.batch_execute(&format!("
        CREATE TABLE IF NOT EXISTS {schema}.metadata (
            db_id   INTEGER PRIMARY KEY GENERATED BY DEFAULT AS IDENTITY,
            first_created   TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            last_opened TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP,
            summary_last_updated    TIMESTAMP WITH TIME ZONE NOT NULL DEFAULT CURRENT_TIMESTAMP
        )
    ", schema = schema))?;

    Ok(())
}

fn insert_starting_data(client: &mut postgres::Client, roster: &Option<Roster>, schema: &str) -> Result<(), postgres::Error> {
    let found_metadata = client.query(&*format!("
        SELECT * from {schema}.metadata
    ", schema = schema), &[])?;
    if found_metadata.len() < 1 {
        client.batch_execute(&format!("
            INSERT INTO {schema}.metadata (db_id)
            SELECT 1
            WHERE NOT EXISTS (SELECT * FROM {schema}.metadata)
            ON CONFLICT DO NOTHING
        ", schema = schema))?;
        client.batch_execute(&format!("
            UPDATE {schema}.metadata
            SET last_opened = CURRENT_TIMESTAMP
            WHERE db_id = 1
        ", schema = schema))?;
        client.batch_execute(&format!("
            INSERT INTO {schema}.statuses (name) VALUES
                ('enrolled'),
                ('dropped')
            ON CONFLICT DO NOTHING
        ", schema = schema))?;
        client.batch_execute(&format!("
            INSERT INTO {schema}.categories (name) VALUES
                ('comment'),
                ('error'),
                ('homework'),
                ('practice'),
                ('question'),
                ('review')
            ON CONFLICT DO NOTHING
        ", schema = schema))?;
    }
    if roster.is_some() {
        let ub_id_query = client.prepare(&format!("
                SELECT ub_id from {schema}.students", schema = schema))?;
        let mut ub_ids_already_present: HashSet<String> = HashSet::from_iter(
            client.query(&ub_id_query, &[])?
            .into_iter()
            .map(|row| row.get("ub_id"))
        );
        let enrolled_query = client.prepare(&format!("
            SELECT db_id FROM {schema}.statuses WHERE name = 'enrolled'", schema = schema))?;
        let enrolled_id: i32 = client
            .query_one(&enrolled_query, &[])?
            .get("db_id");
        let dropped_query = client.prepare(&format!("
                SELECT db_id FROM {schema}.statuses WHERE name = 'dropped'", schema = schema))?;
        let dropped_id: i32 = client
            .query_one(&dropped_query, &[])?
            .get("db_id");
        let statement = client.prepare(&format!("
            INSERT INTO {schema}.students AS s (ub_id, name, status_id, username) VALUES
            ($1, $2, $3, $4)
            ON CONFLICT (ub_id) DO UPDATE SET
            (name, status_id, last_updated, username) = ($2, $3, CURRENT_TIMESTAMP, $4)
                WHERE s.status_id != $3 OR s.name != $2 OR s.username != $4 OR s.username IS NULL
        ", schema = schema))?;
        for (ub_id, name, username) in (*roster).as_ref().unwrap().iter() {
            ub_ids_already_present.remove(ub_id);
            client.execute(&statement, &[&ub_id, &name, &enrolled_id, &username])?;
        }
        let dropped_statement = client.prepare(&format!("
            UPDATE {schema}.students SET
            (status_id, last_updated) = ($1, CURRENT_TIMESTAMP)
            WHERE status_id != $1 AND ub_id = $2
        ", schema = schema))?;
        for ub_id in ub_ids_already_present {
            client.execute(&dropped_statement, &[&dropped_id, &ub_id])?;
        }
    }
    update_summary(client, schema)?;

    Ok(())
}
