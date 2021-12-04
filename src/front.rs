use std::collections::HashMap;
use std::fs::File;
use std::iter::FromIterator;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use chrono::{Local, Date, Datelike, DateTime, NaiveDate, TimeZone};
use cursive::align::HAlign;
use cursive::traits::Scrollable;
use cursive::view::{Boxable, Identifiable};
use cursive::views::{Button, Checkbox, Dialog, DummyView, EditView, LinearLayout, PaddedView, ResizedView, SelectView, TextView};
use cursive::Cursive;
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;

use crate::back::{EventRecorder, StudentPicker};
use crate::model::{Category, Student};
use crate::gate::{export_summary};

pub fn cli(students: Rc<Vec<Student>>, categories: Vec<Category>, picker: StudentPicker, event_recorder: EventRecorder) {
    let categories = Rc::new(categories);
    let picker = Arc::new(Mutex::new(picker));
    let event_recorder = Arc::new(Mutex::new(event_recorder));

    let mut siv = cursive::crossterm();
    siv.load_theme_file("data/style.toml").unwrap();
    siv.add_layer(
        build_main_menu(students, categories, picker, event_recorder)
    );
    siv.run();

    ()
}

struct NamedFinder<T: Named> {
    items: Rc<Vec<T>>,
    matcher: SkimMatcherV2,
}

impl<T: Named> NamedFinder<T> {
    fn new(items: Rc<Vec<T>>) -> NamedFinder<T> {
        NamedFinder {
            items: items,
            matcher: SkimMatcherV2::default(),
        }
    }

    fn find<'a>(&'a self, query: &str) -> Vec<&'a T> {
        let mut found_scores_names_things = Vec::from_iter(self.items.iter()
            .enumerate()
            .filter_map(|(i, item)| match self.matcher.fuzzy_match(item.get_name(), query) {
                Some(score) => Some((score, item.get_name(), i)),
                None => None
            })
        );
        found_scores_names_things.sort();
        Vec::from_iter(found_scores_names_things.iter()
            // go from highest to lowest score
            .rev()
            // keep only reference to Named struct reference
            .filter_map(|a| self.items.get(a.2))
        )
    }
}

trait Named {
    fn get_name(&self) -> &str;
}

impl Named for Category {
    fn get_name(&self) -> &str {
        &self.name
    }
}

impl Named for Student {
    fn get_name(&self) -> &str {
        &self.name
    }
}

fn build_main_menu(students: Rc<Vec<Student>>, categories: Rc<Vec<Category>>, picker: Arc<Mutex<StudentPicker>>, event_recorder: Arc<Mutex<EventRecorder>>) -> Dialog {
    let students_for_recording = Rc::clone(&students);
    let categories_for_recording = Rc::clone(&categories);
    let recorder_for_recording = Arc::clone(&event_recorder);
    let recorder_for_summary = Arc::clone(&event_recorder);
    let students_for_redeeming = Rc::clone(&students);
    let categories_for_redeeming = Rc::clone(&categories);
    let picker_for_redeeming = Arc::clone(&picker);
    let recorder_for_redeeming = Arc::clone(&event_recorder);
    Dialog::around(
        LinearLayout::vertical()
        .child(
            Button::new("Record Participation", move |siv: &mut Cursive| {
                siv.pop_layer();
                siv.add_layer(build_recording_dialog(
                    Rc::clone(&students_for_recording),
                    Rc::clone(&categories_for_recording),
                    Arc::clone(&picker),
                    Arc::clone(&recorder_for_recording),
                    "Ready"
                ))
            })
        )
        .child(
            Button::new("Export Summary", move |siv: &mut Cursive| {
                let recorder_for_summary = Arc::clone(&recorder_for_summary);
                siv.add_layer(Dialog::around(
                    LinearLayout::vertical()
                    .child(
                        TextView::new("Choose output filename and location:")
                    )
                    .child(
                        EditView::new()
                        .content("data/participation_points.tsv")
                        .on_submit(|siv: &mut Cursive, _: &str| {
                            siv.focus_name("exporting_submit_button").unwrap();
                        })
                        .with_name("exporting_edit")
                    )
                    .child(
                        Button::new("Submit", move |inner_siv: &mut Cursive| {
                            let chosen = inner_siv.call_on_name("exporting_edit", |v: &mut EditView| {
                                v.get_content()
                            }).unwrap();
                            match File::create(&*chosen) {
                                Ok(mut outfile) => {
                                    inner_siv.pop_layer();
                                    inner_siv.add_layer(Dialog::around(TextView::new("Starting export").with_name("export_msg")).dismiss_button("Ok"));
                                    match recorder_for_summary.lock().unwrap().get_summary() {
                                        Ok(rows) => {
                                            match export_summary(rows, &mut outfile) {
                                                Ok(()) => { display_export_msg(inner_siv, &*format!("Finished export:\n{}", chosen)); },
                                                Err(e) => { display_export_msg(inner_siv, &*format!("File error: {}", e)); }
                                            }
                                        },
                                        Err(e) => {
                                            display_export_msg(inner_siv, &*format!("Database error: {}", e));
                                        }
                                    }
                                },
                                Err(e) => {
                                    inner_siv.call_on_name("exporting_status_msg", |v: &mut TextView| {
                                        v.set_content(format!("File opening error: {:?}", e))
                                    });
                                }
                            }
                        })
                        .with_name("exporting_submit_button")
                    )
                    .child(
                        TextView::new("Ready")
                        .with_name("exporting_status_msg")
                    )
                ))
            })
        )
        .child(
            Button::new("Redeem Points", move |siv: &mut Cursive| {
                siv.pop_layer();
                siv.add_layer(build_redeeming_dialog_input(
                    Rc::clone(&students_for_redeeming),
                    Rc::clone(&categories_for_redeeming),
                    Arc::clone(&picker_for_redeeming),
                    Arc::clone(&recorder_for_redeeming),
                ));
            })
        )
        .child(
            Button::new("Quit", Cursive::quit)
        )
    )
}

fn build_recording_dialog(students: Rc<Vec<Student>>, categories: Rc<Vec<Category>>, picker: Arc<Mutex<StudentPicker>>, event_recorder: Arc<Mutex<EventRecorder>>, status_msg: &str) -> Dialog {
    Dialog::around(
        LinearLayout::vertical()
        .child(
            LinearLayout::horizontal()
            .child(
                build_recording_student_selector(Rc::clone(&students), Arc::clone(&picker))
            )
            .child(
                build_category_selector(Rc::clone(&categories))
            )
            .child(
                build_satisfactory_selector()
            )
            .child(
                build_recording_buttons_column(
                    Rc::clone(&students),
                    Rc::clone(&categories),
                    Arc::clone(&picker),
                    Arc::clone(&event_recorder)
                )
            )
        )
        .child(
            TextView::new(status_msg)
            .with_name("recording_status")
        )
    )
    .title("Event Recorder")
}

fn build_recording_student_selector(students: Rc<Vec<Student>>, picker: Arc<Mutex<StudentPicker>>) -> PaddedView<ResizedView<LinearLayout>> {
    let student_finder = Rc::new(NamedFinder::new(Rc::clone(&students)));
    let picker = Arc::clone(&picker);
    let students_for_on_edit = Rc::clone(&students);
    let student_finder_for_on_edit = Rc::clone(&student_finder);
    let students_for_on_submit = Rc::clone(&students);
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            TextView::new("Student")
        )
        .child(
            EditView::new()
            // update results every time the query changes
            .on_edit(move |siv: &mut Cursive, query: &str, _cursor: usize| {
                if query.len() > 1 && students_for_on_edit.iter().find(|s| s.name == query[0..query.len()-1]).is_some() {
                    // assume that user wants to change selection
                    let query = &query[query.len()-1..];
                    siv.call_on_name("recording_student_query", |v: &mut EditView| {
                        v.set_content(query.to_string());
                    });
                    let matches = student_finder_for_on_edit.find(query);
                    // Update the `matches` view with the filtered array of student names
                    siv.call_on_name("recording_student_matches", |v: &mut SelectView| {
                        v.clear();
                        v.add_all_str(matches.iter().map(|s| s.name.to_string()));
                    });
                } else {
                    // update without changing query
                    let matches = student_finder_for_on_edit.find(query);
                    // Update the `matches` view with the filtered array of student names
                    siv.call_on_name("recording_student_matches", |v: &mut SelectView| {
                        v.clear();
                        v.add_all_str(matches.iter().map(|s| s.name.to_string()));
                    });
                }
                siv.call_on_name("recording_status", |v: &mut TextView| {
                    v.set_content("Select student");
                });
            })
            // if possible, select student when pressing enter on this edit view
            .on_submit(move |siv: &mut Cursive, text: &str| {
                if text.len() > 0 && students_for_on_submit.iter().find(|s| s.name == text).is_none() {
                    // try to get the top matching student
                    let choice = siv.call_on_name("recording_student_matches", |v: &mut SelectView| {
                        match v.get_item(0) {
                            Some((name, _)) => name.to_string(),
                            None => "".to_string()
                        }
                    }).unwrap();
                    if choice.len() > 0 {
                        siv.call_on_name("recording_student_query", |v: &mut EditView| {
                            v.set_content(choice);
                        });
                        // move focus to next column
                        siv.focus_name("category_edit").unwrap();
                        siv.call_on_name("recording_status", |v: &mut TextView| {
                            v.set_content("Select category");
                        });
                    } else {
                        siv.call_on_name("recording_status", |v: &mut TextView| {
                            v.set_content("No matching student; try again");
                        });
                    }
                } else {
                    // choose a random student
                    let choice_for_edit_view = match picker.lock().unwrap().next() {
                        Some(student) => student.name,
                        None => "".to_string()
                    };
                    let choice_for_select_view = choice_for_edit_view.clone();
                    // Update the `matches` view with random student
                    siv.call_on_name("recording_student_matches", |v: &mut SelectView| {
                        v.clear();
                        v.add_item_str(choice_for_select_view);
                    });
                    siv.call_on_name("recording_student_query", |v: &mut EditView| {
                        v.set_content(choice_for_edit_view);
                    });
                    // move focus to next column
                    siv.focus_name("category_edit").unwrap();
                    siv.call_on_name("recording_status", |v: &mut TextView| {
                        v.set_content("Select category");
                    });
                }
            })
            .with_name("recording_student_query")
        )
        // search results below the input
        .child(
            SelectView::<String>::new()
                // show only top match
                .popup()
                // no students by default
                // freezes popup, passing view on tab (but updates top name still)
                .disabled()
                .with_name("recording_student_matches"),
        )
        .fixed_width(30),
    )
}

fn build_category_selector(categories: Rc<Vec<Category>>) -> PaddedView<LinearLayout> {
    let categories_keeper: HashMap<String, Category> = HashMap::from_iter(categories.iter()
        .map(|c| (c.name[0..1].to_string(), c.clone()))
    );
    let mut sorted_categories = Vec::from_iter(categories.iter()
        .map(|c| c.name.to_string())
    );
    sorted_categories.sort();
    let categories_sign = sorted_categories.iter()
        .map(|c| format!("[{}]{}", &c[0..1], &c[1..]))
        .collect::<Vec<_>>()
        .join("\n");
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            TextView::new("Category")
        )
        .child(
            EditView::new()
            .on_edit(|siv: &mut Cursive, query: &str, _cursor: usize| {
                if query.len() > 1 {
                    siv.call_on_name("category_edit", |v: &mut EditView| {
                        v.set_content(query[query.len()-1..].to_string());
                    });
                }
            })
            .on_submit(move |siv: &mut Cursive, text: &str| {
                match categories_keeper.get(text) {
                    Some(c) => {
                        siv.call_on_name("category_edit", |v: &mut EditView| {
                            v.set_content(c.name.to_string());
                        });
                        siv.focus_name("satisfactory_checkbox").unwrap();
                        siv.call_on_name("recording_status", |v: &mut TextView| {
                            v.set_content("Satisfactory?");
                        });
                    }
                    None => {
                        // this was not a valid category; try again
                        ()
                    }
                }
            })
            .with_name("category_edit")
        )
        .child(
            TextView::new(categories_sign)
        )
    )
}

fn build_satisfactory_selector() -> PaddedView<LinearLayout> {
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            TextView::new("?")
        )
        .child(
            Checkbox::new()
            .on_change(|siv: &mut Cursive, _val: bool| {
                siv.focus_name("recording_submit_button").unwrap();
                siv.call_on_name("recording_status", |v: &mut TextView| {
                    v.set_content("Submit");
                });
            })
            .with_name("satisfactory_checkbox")
        )
    )
}

fn build_recording_buttons_column(students: Rc<Vec<Student>>, categories: Rc<Vec<Category>>, picker: Arc<Mutex<StudentPicker>>, event_recorder: Arc<Mutex<EventRecorder>>) -> PaddedView<LinearLayout> {
    let students_for_submit = Rc::clone(&students);
    let categories_for_submit = Rc::clone(&categories);
    let recorder_for_submit = Arc::clone(&event_recorder);
    let students_for_main = Rc::clone(&students);
    let categories_for_main = Rc::clone(&categories);
    let picker_for_main = Arc::clone(&picker);
    let recorder_for_main = Arc::clone(&event_recorder);
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            Button::new("Submit", move |siv: &mut Cursive| {
                siv.call_on_name("recording_status", |v: &mut TextView| {
                    v.set_content("Submit button pushed");
                });
                let student_name: Rc<String> = siv.find_name::<EditView>("recording_student_query").unwrap().get_content();
                let category_name: Rc<String> = siv.find_name::<EditView>("category_edit").unwrap().get_content();
                let satisfactory: bool = siv.find_name::<Checkbox>("satisfactory_checkbox").unwrap().is_checked();
                match recorder_for_submit.lock().unwrap().record(&student_name, &category_name, satisfactory) {
                    Ok(n) => {
                        match n {
                            1 => {
                                siv.pop_layer();
                                siv.add_layer(build_recording_dialog(
                                    Rc::clone(&students_for_submit),
                                    Rc::clone(&categories_for_submit),
                                    Arc::clone(&picker),
                                    Arc::clone(&recorder_for_submit),
                                    "Submitted successfully"
                                ))
                            },
                            _ => {
                                siv.call_on_name("recording_status", |v: &mut TextView| {
                                    v.set_content(format!("Problem: submitted {} (are all fields correct?)", n));
                                });
                            }
                        }
                    },
                    Err(e) => {
                        match e.as_db_error() {
                            Some(dbe) => {
                                siv.call_on_name("recording_status", |v: &mut TextView| {
                                    v.set_content(format!("DB Error ({}): {}", dbe.severity(), dbe.message()));
                                });
                            },
                            None => {
                                siv.call_on_name("recording_status", |v: &mut TextView| {
                                    v.set_content(format!("Error: {}", e));
                                });
                            }
                        }
                    }
                };
            })
            .with_name("recording_submit_button")
        )
        .child(
            Button::new("Back to main", move |siv: &mut Cursive| {
                siv.pop_layer();
                siv.add_layer(build_main_menu(
                    Rc::clone(&students_for_main),
                    Rc::clone(&categories_for_main),
                    Arc::clone(&picker_for_main),
                    Arc::clone(&recorder_for_main)))
            })
            .with_name("recording_back_button")
        )
    )
}

fn display_export_msg(siv: &mut Cursive, msg: &str) {
    match siv.find_name::<TextView>("export_msg") {
        Some(mut v) => { v.set_content(msg); },
        None => { Dialog::info(msg); }
    };
}

fn build_redeeming_dialog_input(students: Rc<Vec<Student>>, categories: Rc<Vec<Category>>, picker: Arc<Mutex<StudentPicker>>, event_recorder: Arc<Mutex<EventRecorder>>) -> Dialog {
    Dialog::around(
        LinearLayout::vertical()
        .child(
            LinearLayout::horizontal()
            .child(
                build_redeeming_student_selector(Rc::clone(&students))
            )
            .child(
                build_date_selector()
            )
            .child(
                build_redeeming_buttons_column(
                    Rc::clone(&students),
                    Rc::clone(&categories),
                    Arc::clone(&picker),
                    Arc::clone(&event_recorder)
                )
            )
        )
        .child(
            TextView::new("Ready")
            .with_name("redeeming_status")
        )
    )
}

fn build_redeeming_student_selector(students: Rc<Vec<Student>>) -> PaddedView<ResizedView<LinearLayout>> {
    let student_finder = Rc::new(NamedFinder::new(Rc::clone(&students)));
    let students_for_on_edit = Rc::clone(&students);
    let student_finder_for_on_edit = Rc::clone(&student_finder);
    let students_for_on_submit = Rc::clone(&students);
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            TextView::new("Student")
        )
        .child(
            EditView::new()
            // update results every time the query changes
            .on_edit(move |siv: &mut Cursive, query: &str, _cursor: usize| {
                if query.len() > 1 && students_for_on_edit.iter().find(|s| s.name == query[0..query.len()-1]).is_some() {
                    // assume that user wants to change selection
                    let query = &query[query.len()-1..];
                    siv.call_on_name("redeeming_student_query", |v: &mut EditView| {
                        v.set_content(query.to_string());
                    });
                    let matches = student_finder_for_on_edit.find(query);
                    // Update the `matches` view with the filtered array of student names
                    siv.call_on_name("redeeming_student_matches", |v: &mut SelectView| {
                        v.clear();
                        v.add_all_str(matches.iter().map(|s| s.name.to_string()));
                    });
                } else {
                    // update without changing query
                    let matches = student_finder_for_on_edit.find(query);
                    // Update the `matches` view with the filtered array of student names
                    siv.call_on_name("redeeming_student_matches", |v: &mut SelectView| {
                        v.clear();
                        v.add_all_str(matches.iter().map(|s| s.name.to_string()));
                    });
                }
                siv.call_on_name("redeeming_status", |v: &mut TextView| {
                    v.set_content("Select student");
                });
            })
            // if possible, select student when pressing enter on this edit view
            .on_submit(move |siv: &mut Cursive, text: &str| {
                if text.len() > 0 && students_for_on_submit.iter().find(|s| s.name == text).is_none() {
                    // try to get the top matching student
                    let choice = siv.call_on_name("redeeming_student_matches", |v: &mut SelectView| {
                        match v.get_item(0) {
                            Some((name, _)) => name.to_string(),
                            None => "".to_string()
                        }
                    }).unwrap();
                    if choice.len() > 0 {
                        siv.call_on_name("redeeming_student_query", |v: &mut EditView| {
                            v.set_content(choice);
                        });
                        // move focus to next column
                        siv.focus_name("redeeming_date_edit").unwrap();
                        siv.call_on_name("redeeming_status", |v: &mut TextView| {
                            v.set_content("Select date");
                        });
                    } else {
                        siv.call_on_name("redeeming_status", |v: &mut TextView| {
                            v.set_content("No matching student; try again");
                        });
                    }
                } else {
                    siv.call_on_name("redeeming_status", |v: &mut TextView| {
                        v.set_content("No matching student; try again");
                    });
                }
            })
            .with_name("redeeming_student_query")
        )
        // search results below the input
        .child(
            SelectView::<String>::new()
                .popup()
                // freezes popup, passing view on tab (but updates top name still)
                .disabled()
                .with_name("redeeming_student_matches"),
        )
        .fixed_width(30),
    )
}

fn build_date_selector() -> PaddedView<ResizedView<LinearLayout>> {
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            TextView::new("Date")
        )
        .child(
            EditView::new()
            .content(format!("{}", Local::today().format("%Y-%m-%d")))
            .on_submit(move |siv: &mut Cursive, _text: &str| {
                siv.focus_name("redeeming_retrieve_button").unwrap();
            })
            .with_name("redeeming_date_edit")
        )
        .fixed_width(20)
    )
}

fn build_redeeming_buttons_column(students: Rc<Vec<Student>>, categories: Rc<Vec<Category>>, picker: Arc<Mutex<StudentPicker>>, event_recorder: Arc<Mutex<EventRecorder>>) -> PaddedView<LinearLayout> {
    let students_for_main = Rc::clone(&students);
    let categories_for_main = Rc::clone(&categories);
    let picker_for_main = Arc::clone(&picker);
    let recorder_for_main = Arc::clone(&event_recorder);
    PaddedView::lrtb(
        2, 2, 0, 0,
        LinearLayout::vertical()
        .child(
            Button::new("Retrieve", move |siv: &mut Cursive| {
                let student_name: Rc<String> = siv.find_name::<EditView>("redeeming_student_query").unwrap().get_content();
                let date_str: Rc<String> = siv.find_name::<EditView>("redeeming_date_edit").unwrap().get_content();
                match NaiveDate::parse_from_str(&date_str, "%Y-%m-%d") {
                    Ok(d) => {
                        let d = Local.ymd(d.year(), d.month(), d.day());
                        match event_recorder.lock().unwrap().retrieve_events(&student_name, &d) {
                            Ok(rows) => {
                                siv.pop_layer();
                                siv.add_layer(build_redeeming_dialog_choose(
                                    &student_name,
                                    d,
                                    rows,
                                    Rc::clone(&students),
                                    Rc::clone(&categories),
                                    Arc::clone(&picker),
                                    Arc::clone(&event_recorder)
                                ))
                            },
                            Err(e) => {
                                match e.as_db_error() {
                                    Some(dbe) => {
                                        siv.call_on_name("redeeming_status", |v: &mut TextView| {
                                            v.set_content(format!("DB Error ({}): {}", dbe.severity(), dbe.message()));
                                        });
                                    },
                                    None => {
                                        siv.call_on_name("redeeming_status", |v: &mut TextView| {
                                            v.set_content(format!("Error: {}", e));
                                        });
                                    }
                                }
                            }
                        }
                    },
                    Err(e) => {
                        siv.call_on_name("redeeming_status", |v: &mut TextView| {
                            v.set_content(format!("Date parsing error: {:?}", e))
                        });
                    }
                }
            })
            .with_name("redeeming_retrieve_button")
        )
        .child(
            Button::new("Back to main", move |siv: &mut Cursive| {
                siv.pop_layer();
                siv.add_layer(build_main_menu(
                    Rc::clone(&students_for_main),
                    Rc::clone(&categories_for_main),
                    Arc::clone(&picker_for_main),
                    Arc::clone(&recorder_for_main)))
            })
            .with_name("redeeming_back_button")
        )
    )
}

fn build_redeeming_dialog_choose(student_name: &str, chosen_date: Date<Local>, rows: Vec<postgres::Row>, students: Rc<Vec<Student>>, categories: Rc<Vec<Category>>, picker: Arc<Mutex<StudentPicker>>, event_recorder: Arc<Mutex<EventRecorder>>) -> Dialog {
    let mut data = LinearLayout::vertical();
    let id_width: usize = 4;
    let category_width: usize = 10;
    let date_width: usize = 20;
    let satisfactory_width: usize = 4;
    let rows_len = rows.len();
    for (i, row) in rows.iter().enumerate() {
        let event_id: i32 = row.get(0);
        let category_name: String = row.get(1);
        let first_entered: DateTime<Local> = row.get(2);
        let sat: bool = row.get(3);
        data.add_child(LinearLayout::horizontal()
            .child(
                TextView::new(format!("{}", event_id))
                .h_align(HAlign::Right)
                .fixed_width(id_width)
            )
            .child(DummyView)
            .child(
                TextView::new(format!("{}", category_name))
                .fixed_width(category_width)
            )
            .child(DummyView)
            .child(
                TextView::new(format!("{}", first_entered.format("%H:%M %F")))
                .fixed_width(date_width)
            )
            .child(DummyView)
            .child(
                Checkbox::new()
                .with_checked(sat)
                .on_change(move |siv: &mut Cursive, _val: bool| {
                    let next_i = i + 1;
                    if next_i >= rows_len {
                        siv.focus_name("redeeming_submit_button").unwrap();
                    } else {
                        siv.focus_name(&*format!("redeeming_checkbox_{}", next_i)).unwrap();
                    }
                })
                .fixed_width(satisfactory_width)
                .with_name(format!("redeeming_checkbox_{}", i))
            )
        );
    }
    Dialog::around(
        LinearLayout::vertical()
        .child(LinearLayout::horizontal()
            .child(
                TextView::new("ID")
                .h_align(HAlign::Right)
                .fixed_width(id_width)
            )
            .child(DummyView)
            .child(
                TextView::new("Category")
                .fixed_width(category_width)
            )
            .child(DummyView)
            .child(
                TextView::new("Date")
                .fixed_width(date_width)
            )
            .child(DummyView)
            .child(
                TextView::new("?")
                .fixed_width(satisfactory_width)
            )
        )
        .child(DummyView)
        .child(data.full_height().scrollable())
        .child(DummyView)
        .child(
            Button::new("Submit", move |siv: &mut Cursive| {
                let changes: Vec<(bool, i32)> = rows.iter().enumerate()
                    .filter_map(|(i, row)| {
                        let db_id: i32 = row.get(0);
                        let original: bool = row.get(3);
                        let submitted: bool = siv.find_name::<ResizedView<Checkbox>>(&*format!("redeeming_checkbox_{}", i)).unwrap().get_inner().is_checked();
                        if original != submitted {
                            Some((submitted, db_id))
                        } else {
                            None
                        }
                    })
                    .collect();
                siv.call_on_name("redeeming_chooser_status_msg", |v: &mut TextView| {
                    v.set_content("Updating database");
                });
                match event_recorder.lock().unwrap().change_events(&changes) {
                    Ok(()) => {
                        siv.pop_layer();
                        siv.add_layer(build_main_menu(
                            Rc::clone(&students),
                            Rc::clone(&categories),
                            Arc::clone(&picker),
                            Arc::clone(&event_recorder),
                        ));
                        siv.add_layer(Dialog::info("Database changes recorded"))
                    },
                    Err(e) => {
                        match e.as_db_error() {
                            Some(dbe) => {
                                siv.call_on_name("redeeming_chooser_status_msg", |v: &mut TextView| {
                                    v.set_content(format!("DB Error ({}): {}", dbe.severity(), dbe.message()));
                                });
                            },
                            None => {
                                siv.call_on_name("redeeming_chooser_status_msg", |v: &mut TextView| {
                                    v.set_content(format!("Error: {}", e));
                                });
                            }
                        }
                    }
                }
            })
            .with_name("redeeming_submit_button")
        )
        .child(
            TextView::new("Ready")
            .with_name("redeeming_chooser_status_msg")
        )
    )
    .title(format!("{} ({})", student_name, chosen_date))
}
