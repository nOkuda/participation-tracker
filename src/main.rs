mod back;
mod front;
mod gate;
mod model;

use std::env;
use std::process;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn main() -> () {
    let schema = "real";
    let roster = match env::args_os().nth(1) {
        Some(path) => {
            match gate::read_roster(path) {
                Ok(r) => Some(r),
                Err(e) => {
                    println!("Error in reading roster:");
                    println!("{:?}", e);
                    process::exit(1);
                }
            }
        },
        None => None
    };
    let client = match back::get_db_conn(&roster, schema) {
        Ok(c) => Arc::new(Mutex::new(c)),
        Err(e) => {
            println!("Database error:");
            println!("{:?}", e);
            process::exit(1);
        },
    };
    let categories = match back::get_categories(&mut client.lock().unwrap(), schema) {
        Ok(c) => c,
        Err(e) => {
            println!("Couldn't get categories");
            println!("{:?}", e);
            process::exit(1);
        }
    };
    let students = match back::get_students(&mut client.lock().unwrap(), schema) {
        Ok(c) => c,
        Err(e) => {
            println!("Couldn't get students");
            println!("{:?}", e);
            process::exit(1);
        }
    };
    if students.len() <= 0 {
        println!("No students in database; exiting");
        println!("(If you would like to add students to the database or update them, run the program with the path to the student roster file as the first argument)");
        process::exit(1);
    }
    let event_recorder = back::get_event_recorder(Arc::clone(&client), schema);
    let students = Rc::new(students);
    let picker = back::get_student_picker(Rc::clone(&students));
    front::cli(students, categories, picker, event_recorder);
}
