# Participation Tracker

An application for tracking participation during class.

## Architecture (Plans)

A configuration file will store student names and database access information.

The backend will read the configuration file, open the database, and ensure that the database is configured properly.
Its outward facing API exposes higher-level functions to read information from the database, update the database with new information, and recompute some internal database statistics.

The frontend will provide a user interface to interact with the database.
It is a command line interface.

### Database

The database includes the following tables:

* students
* statuses
* history
* categories
* summary
* metadata

The students table contains information about students in the class.
This table contains the following fields:

* `db_id`: database identifier for a student
* `ub_id`: UB student number
* `name`: name of student
* `first_entered`: timestamp when this student was originally entered
* `status_id`: database identifier for a status
* `last_updated`: timestamp of last status change
* `username`: UBLearns username

The statuses table indicates what status a student is in (active, dropped, etc.)
This table contains the following fields:

* `db_id`: database identifier for status
* `name`: status name
* `first_entered`: timestamp originally inserted into table

The history table keeps track of which students have been chosen so far, when they were chosen, and whether they earned a point.
This table contains the following fields:

* `db_id`: event identifier
* `student_id`: student identifier
* `category_id`: category identifier
* `first_entered`: timestamp originally inserted into table
* `satisfactory`: whether point was earned

The categories table indicates what category a point was earned for (homework answer, question, class participation, etc.)
This table contains the following fields:

* `db_id`: category identifier
* `name`: category name
* `first_entered`: timestamp originally inserted into table

The following categories are available by default:

* comment
* error
* homework
* practice
* question
* review

The summary table indicates how many points each student has earned as of the last time this table was updated.
It is not intended to be up to date at all times.
This table contains the following fields:

* `db_id`: row identifier
* `student_id`: database identifier associated with student
* `points`: total points student has earned

The metadata table contains information about the database.
This table contains the following fields:

* `db_id`: row identifier
* `first_created`: timestamp when database was first created
* `last_opened`: timestamp when database was last opened
* `summary_last_updated`: timestamp when summary table was last updated

### Backend API

The backend API exposes the following major methods:

* `get_student_picker`
* `get_event_recorder`
* `get_summary`

The backend API also exposes the following informational methods:

* `get_categories`
* `get_students`
* `get_summary`

### Gate

The gate facilitates interaction between the program and UBLearns, particularly in reading and writing tab separated files.
It exposes the following major methods:

* `get_roster`
* `export_summary`

### User Interface

The command line interface has a main menu through which the following modes are made available:

* Record Participation
* Export Summary
* Redeem Points
* Quit

#### Record Participation

The "Record Participation" option opens event recording mode, which guides the user through a series of text boxes to record participation events.

The first text box expects a student name.
Typing into the text box will fuzzy search for a student's name.
Pressing enter in the text box will select whatever student has the name with the closest fuzzy match,
unless the text box is empty, in which case a random student's name will be chosen.

The second text box expects the first letter of the category name for this event.
The possible categories are displayed, with first letters enclosed in brackets.

The third text box asks whether a contribution was made satisfactorily by this student.
Typing "y" and pressing enter will indicate that the contribution was satisfactory.
Typing "n" and pressing enter will indicate that the contribution was unsatisfactory.

The "Submit" button will attempt to write the event into the database, according to what .
In the case of a database error, an error message will be displayed.

The "Back to main" button will return to the main menu.

#### Export Summary

The "Export Summary" option will generate a tab-delimited file that lists UB IDs and participation points earned for the three rounds.
This exported file can be uploaded to UBLearns to update scores.

#### Redeem Points

The "Redeem Points" option opens a point redemption mode, which guides the user through a series of prompts to change whether events associated with a given student were satisfactory for a particular day.

The user will first be given a text box to enter a student name.
Like in event recording, the text box will perform a fuzzy search for a student's name, and pressing enter will select the students with the closest fuzzy matching name.
However, an empty text box will not be permitted.

The second text box expects a date.
The text box will be pre-generated with the current date.

Finally, "Retrieve" button will lead to a change mode displaying events associated with the given student and the given date.
The change mode will allow for selecting individual events and changing the satsifactory state.
After all events have been reviewed, a "Submit" button will update the database with the changes made.

#### Quit

The "Quit" option exits the program.

## Reminders

Remember to backup the database frequently.

## Administrative Details

### Database

Start database:

`sudo systemctl start postgresql@12-main`

Check database status:

`systemctl status postgresql@12-main`

Log into database:

`psql fall2021_latin101`

Back up database:

`pg_dump fall2021_latin101 -n [schema] > [backup name]`

## Notes on Initial Setup

Postgresql

* <https://www.postgresql.org/docs/12/index.html>
* <https://wiki.postgresql.org/wiki/First_steps>
* use identity instead of serial: <https://stackoverflow.com/a/55300741>
* desribe table: `\d [table name]`
* show tables: `\dt`
* quit: `\q`
* help: `\?`
* configuration files: /etc/postgresql/12/main/
* show schemas: `select schema_name from information_schema.schemata;`
  * <https://dba.stackexchange.com/questions/40045/how-do-i-list-all-schemas-in-postgresql/40051>
* list tables in a schema: `\dt <schema>.*`
  * <https://stackoverflow.com/questions/15644152/list-tables-in-a-postgresql-schema>
* drop all tables in schema: `DROP SCHEMA <schema> CASCADE;`
  * <https://dba.stackexchange.com/a/184985>

Rust

* comparison of string building strategies: <https://github.com/hoodie/concatenation_benchmarks-rs>
* how to wrangle with static lifetime requirements of closures: <https://users.rust-lang.org/t/cursive-closures-with-static-lifetime/19586/7>
* user interface library: <https://docs.rs/cursive/0.16.3/cursive/>
  * finding view by name: <https://docs.rs/cursive/0.16.3/cursive/struct.Cursive.html#method.find_name>
  * finding type of view by name: <https://docs.rs/cursive/0.16.3/cursive/struct.Cursive.html#method.debug_name>
    * note that type returned by `find_name` is what's inside of NamedView<_>
