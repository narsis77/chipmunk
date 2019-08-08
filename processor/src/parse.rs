// Copyright (c) 2019 E.S.R.Labs. All rights reserved.
//
// NOTICE:  All information contained herein is, and remains
// the property of E.S.R.Labs and its suppliers, if any.
// The intellectual and technical concepts contained herein are
// proprietary to E.S.R.Labs and its suppliers and may be covered
// by German and Foreign Patents, patents in process, and are protected
// by trade secret or copyright law.
// Dissemination of this information or reproduction of this material
// is strictly forbidden unless prior written permission is obtained
// from E.S.R.Labs.
use indexer_base::timedline::TimedLine;
use indexer_base::error_reporter::*;
use chrono::{NaiveDate, NaiveDateTime, Utc, Datelike};
use std::borrow::Cow;
use nom::bytes::complete::tag;

use nom::character::complete::{char, digit1};
use nom::combinator::{map, map_res, opt};
use nom::multi::{fold_many0, many1};
use nom::IResult;

use regex::Regex;
use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;
use std::fs;

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

const MAX_LINES_TO_INSPECT: usize = 1_000_000;
const LINE_DETECTION_THRESHOLD: usize = 5;

/// add a format string in normal form and with TZD for a possible timezone match
macro_rules! add_twice {
    ($var:expr, $a:expr, $m:expr) => {
        $a.push(concat!($var, " TZD"));
        $a.push($var);
        $m.insert(
            concat!($var, " TZD"),
            date_format_str_to_regex(concat!($var, " TZD")).unwrap(),
        );
        $m.insert($var, date_format_str_to_regex($var).unwrap());
    };
}

lazy_static! {
    static ref FORMAT_REGEX_MAPPINGS: (Vec<&'static str>, BTreeMap<&'static str, Regex>) = {
        let mut arr = Vec::new();
        let mut v = BTreeMap::default();
        add_twice!("YYYY-MM-DD hh:mm:ss.s", &mut arr, &mut v);
        add_twice!("YYYY-MM-DDThh:mm:ss.s", &mut arr, &mut v);
        add_twice!("YYYY-MM-DDThh:mm:ss", &mut arr, &mut v);
        add_twice!("MM-DDThh:mm:ss.s", &mut arr, &mut v);
        add_twice!("MM-DD hh:mm:ss.s", &mut arr, &mut v);
        add_twice!("MM-DD-YYYYThh:mm:ss.s", &mut arr, &mut v);
        add_twice!("MM-DD-YYYY hh:mm:ss.s", &mut arr, &mut v);
        add_twice!("MM/DD/YYYY hh:mm:ss a", &mut arr, &mut v);
        add_twice!("MM/DD/yy hh:mm:ss", &mut arr, &mut v);
        add_twice!("DD/MMM/YYYY:hh:mm:ss", &mut arr, &mut v);
        add_twice!("DD/MMM/YYYY hh:mm:ss", &mut arr, &mut v);
        add_twice!("DD-MMM-YYYY hh:mm:ss", &mut arr, &mut v);
        add_twice!("DD/MMM/YYYYThh:mm:ss", &mut arr, &mut v);
        add_twice!("DD MMM YYYY hh:mm:ss", &mut arr, &mut v);
        (arr, v)
    };
    static ref AVAILABLE_REGEXES: Vec<&'static str> = FORMAT_REGEX_MAPPINGS.0.clone();
    static ref FORMAT_REGEX_MAPPING: BTreeMap<&'static str, Regex> =
        FORMAT_REGEX_MAPPINGS.1.clone();
}

#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub enum FormatPiece {
    Day,
    Month,
    MonthName,
    Year,
    YearShort,
    Hour,
    Minute,
    Second,
    Fraction,
    TimeZone,
    AmPm,
    AbsoluteMilliseconds,
    SeperatorChar(char),
    Seperator(String),
}
impl std::fmt::Display for FormatPiece {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            FormatPiece::Day => write!(f, "Day"),
            FormatPiece::Month => write!(f, "Month"),
            FormatPiece::MonthName => write!(f, "MonthName"),
            FormatPiece::Year => write!(f, "Year"),
            FormatPiece::YearShort => write!(f, "YearShort"),
            FormatPiece::Hour => write!(f, "Hour"),
            FormatPiece::Minute => write!(f, "Minute"),
            FormatPiece::Second => write!(f, "Second"),
            FormatPiece::Fraction => write!(f, "Fraction"),
            FormatPiece::AmPm => write!(f, "AmPm"),
            FormatPiece::TimeZone => write!(f, "TimeZone"),
            FormatPiece::AbsoluteMilliseconds => write!(f, "AbsoluteMilliseconds"),
            FormatPiece::SeperatorChar(_) => write!(f, "SeperatorChar"),
            FormatPiece::Seperator(_) => write!(f, "Seperator"),
        }
    }
}
/// takes a format string for a date representation and tokenize it
/// into it's individual elements
/// e.g. "DD-MM" => [Day,Seperator("-"),Month]
///      YYYY = four-digit year
///      MMM  = short month form (Jan, Feb, Mar, ..., Dec)
///      MM   = two-digit month (01=January, etc.)
///      DD   = two-digit day of month (01 through 31)
///      hh   = two digits of hour (00 through 23) (am/pm NOT allowed)
///      mm   = two digits of minute (00 through 59)
///      ss   = two digits of second (00 through 59)
///      s    = one or more digits representing a decimal fraction of a second
///      TZD  = time zone designator (Z or +hh:mm or -hh:mm)
static DAY_FORMAT_TAG: &str = "DD";
static MONTH_FORMAT_SHORT_NAME_TAG: &str = "MMM";
static MONTH_FORMAT_TAG: &str = "MM";
static YEAR_FORMAT_TAG: &str = "YYYY";
static YEAR_SHORT_FORMAT_TAG: &str = "yy";
static HOURS_FORMAT_TAG: &str = "hh";
static MINUTES_FORMAT_TAG: &str = "mm";
static SECONDS_FORMAT_TAG: &str = "ss";
static TIMEZONE_FORMAT_TAG: &str = "TZD";
const FRACTION_FORMAT_CHAR: char = 's';
const AM_PM_TAG: char = 'a';

fn days(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(DAY_FORMAT_TAG), |_| FormatPiece::Day)(input)
}
fn month_short(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(MONTH_FORMAT_SHORT_NAME_TAG), |_| FormatPiece::MonthName)(input)
}
fn month(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(MONTH_FORMAT_TAG), |_| FormatPiece::Month)(input)
}
fn year(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(YEAR_FORMAT_TAG), |_| FormatPiece::Year)(input)
}
fn year_short(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(YEAR_SHORT_FORMAT_TAG), |_| FormatPiece::YearShort)(input)
}
fn hours(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(HOURS_FORMAT_TAG), |_| FormatPiece::Hour)(input)
}
fn minutes(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(MINUTES_FORMAT_TAG), |_| FormatPiece::Minute)(input)
}
fn seconds(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(SECONDS_FORMAT_TAG), |_| FormatPiece::Second)(input)
}
fn fraction(input: &str) -> IResult<&str, FormatPiece> {
    map(nom::character::complete::char(FRACTION_FORMAT_CHAR), |_| {
        FormatPiece::Fraction
    })(input)
}
fn am_pm(input: &str) -> IResult<&str, FormatPiece> {
    map(nom::character::complete::char(AM_PM_TAG), |_| {
        FormatPiece::AmPm
    })(input)
}
fn timezone(input: &str) -> IResult<&str, FormatPiece> {
    map(tag(TIMEZONE_FORMAT_TAG), |_| FormatPiece::TimeZone)(input)
}
fn absolute_millis(input: &str) -> IResult<&str, FormatPiece> {
    map(tag("sss"), |_| FormatPiece::AbsoluteMilliseconds)(input)
}
fn many_spaces(input: &str) -> IResult<&str, char> {
    map(many1(nom::character::complete::char(' ')), |_| ' ')(input)
}
fn seperator(input: &str) -> IResult<&str, FormatPiece> {
    map(
        nom::branch::alt((many_spaces, nom::character::complete::anychar)),
        FormatPiece::SeperatorChar,
    )(input)
}
fn any_date_format(input: &str) -> IResult<&str, FormatPiece> {
    nom::branch::alt((
        days,
        month_short,
        month,
        year,
        year_short,
        hours,
        minutes,
        absolute_millis,
        seconds,
        fraction,
        am_pm,
        timezone,
        seperator,
    ))(input)
}
fn escape_metacharacters(c: char) -> Cow<'static, str> {
    match c {
        //  .|?*+(){}[]DD
        ' ' => r"\s?",
        '.' => r"\.",
        '|' => r"\|",
        '?' => r"\?",
        '+' => r"\+",
        '(' => r"\(",
        ')' => r"\)",
        '[' => r"\[",
        '{' => r"\{",
        '^' => r"\^",
        '$' => r"\$",
        '*' => r"\*",
        _ => return c.to_string().into(),
    }
    .into()
}
pub fn date_expression(input: &str) -> IResult<&str, Vec<FormatPiece>> {
    let parser = fold_many0(
        any_date_format,
        (String::from(""), Vec::new()),
        |mut acc: (String, Vec<_>), item| {
            match item {
                FormatPiece::SeperatorChar(c) => acc.0.push_str(&escape_metacharacters(c)),
                _ => {
                    if !acc.0.is_empty() {
                        acc.1.push(FormatPiece::Seperator(acc.0));
                        acc.0 = String::from("")
                    }
                    acc.1.push(item)
                }
            };
            acc
        },
    );
    map(parser, |p: (String, Vec<FormatPiece>)| {
        if !p.0.is_empty() {
            let mut res_vec = p.1;
            res_vec.push(FormatPiece::Seperator(p.0));
            return res_vec;
        }
        p.1
    })(input)
}

pub fn date_format_str_to_regex(date_format: &str) -> Result<Regex, failure::Error> {
    if date_format.is_empty() {
        return Err(failure::err_msg("cannot construct regex from empty string"));
    }
    let format_pieces = date_expression(date_format);
    match format_pieces {
        Ok(r) => {
            if r.1.is_empty() {
                return Err(failure::err_msg(
                    "could not create regex, problems with format pieces",
                ));
            }
            let s = r.1.iter().fold(String::from(r""), |mut acc, x| {
                let part = format_piece_as_regex_string(x);
                acc.push_str(part.as_str());
                acc
            });

            return match Regex::new(s.as_str()) {
                Ok(regex) => Ok(regex),
                Err(e) => Err(failure::err_msg(format!("could not create regex: {}", e))),
            };
        }
        Err(e) => report_error(format!("{:?}", e)),
    }
    Err(failure::err_msg("could not detect timestamp in"))
}
static DAY_GROUP: &str = "d";
static MONTH_GROUP: &str = "m";
static MONTH_SHORT_NAME_GROUP: &str = "MMM";
static YEAR_GROUP: &str = "Y";
static YEAR_SHORT_GROUP: &str = "y";
static HOUR_GROUP: &str = "H";
static MINUTE_GROUP: &str = "M";
static SECONDS_GROUP: &str = "S";
static FRACTION_GROUP: &str = "millis";
static AM_PM_GROUP: &str = "am_pm";
static TIMEZONE_GROUP: &str = "timezone";
static ABSOLUTE_MS_GROUP: &str = "absolute";

fn named_group(regex: &str, capture_id: &str) -> String {
    format!(r"(?P<{}>{})", capture_id, regex)
}

fn format_piece_as_regex_string(p: &FormatPiece) -> String {
    match p {
        FormatPiece::Day => named_group(r"([0-2]\d|3[01])", DAY_GROUP),
        FormatPiece::Month => named_group(r"(0?\d|1[0-2])", MONTH_GROUP),
        FormatPiece::MonthName => named_group(
            r"(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)",
            MONTH_SHORT_NAME_GROUP,
        ),
        FormatPiece::Year => named_group(r"[0-2]\d{3}", YEAR_GROUP),
        FormatPiece::YearShort => named_group(r"\d{2}", YEAR_SHORT_GROUP),
        FormatPiece::Hour => named_group(r"(0?\d|1\d|2[0-3])", HOUR_GROUP),
        FormatPiece::Minute => named_group(r"[0-5]\d", MINUTE_GROUP),
        FormatPiece::Second => named_group(r"[0-5]\d", SECONDS_GROUP),
        FormatPiece::Fraction => named_group(r"\d+", FRACTION_GROUP),
        FormatPiece::AmPm => named_group(r"(AM|PM)", AM_PM_GROUP),
        FormatPiece::TimeZone => named_group(r"[\+\-](0\d|1[0-4]):?(00|30|45)", TIMEZONE_GROUP),
        FormatPiece::AbsoluteMilliseconds => named_group(r"\d+", ABSOLUTE_MS_GROUP),
        FormatPiece::SeperatorChar(c) => {
            let mut s = String::from("");
            s.push(*c);
            s
        }
        FormatPiece::Seperator(s) => s.to_string(),
    }
}
#[derive(Serialize, Deserialize, Debug)]
pub struct FormatStringMatches {
    pub regex: String,
    pub matching_lines: usize,
    pub nonmatching_lines: usize,
    pub processed_bytes: usize,
}
#[derive(Serialize, Deserialize, Debug)]
pub struct FormatTestOptions {
    pub file: String,
    pub lines_to_test: i64,
    pub format: String,
}

pub fn read_format_string_options(f: &mut fs::File) -> Result<FormatTestOptions, failure::Error> {
    let mut contents = String::new();
    f.read_to_string(&mut contents)
        .expect("something went wrong reading the file");
    let v: FormatTestOptions = serde_json::from_str(&contents[..])?;
    Ok(v)
}

pub fn match_format_string_in_file(
    format_expr: &str,
    file_name: &str,
    max_lines: i64,
) -> Result<FormatStringMatches, failure::Error> {
    let regex = date_format_str_to_regex(format_expr)?;
    let path = PathBuf::from(file_name);
    let f: fs::File = fs::File::open(path)?;
    let mut reader: BufReader<&std::fs::File> = BufReader::new(&f);
    let mut buf = vec![];
    let mut inspected_lines = 0usize;
    let mut matched_lines = 0usize;
    let mut processed_bytes = 0;
    while let Ok(len) = reader.read_until(b'\n', &mut buf) {
        if len == 0 {
            break; // file is done
        }
        let s = unsafe { std::str::from_utf8_unchecked(&buf) };
        if !s.trim().is_empty() {
            inspected_lines += 1;
            if regex.is_match(s.trim()) {
                matched_lines += 1;
            }
            processed_bytes += s.trim().len();
        }
        buf = vec![];
        if inspected_lines > max_lines as usize {
            break;
        }
    }
    Ok(FormatStringMatches {
        regex: regex.to_string(),
        matching_lines: matched_lines,
        nonmatching_lines: inspected_lines - matched_lines,
        processed_bytes,
    })
}

pub fn line_matching_format_expression(
    format_expr: &str,
    line: &str,
) -> Result<bool, failure::Error> {
    let regex = date_format_str_to_regex(format_expr)?;
    let res = regex.is_match(line);
    Ok(res)
}
fn parse_from_month(mmm: &str) -> Result<u32, failure::Error> {
    match mmm {
        "Jan" => Ok(1),
        "Feb" => Ok(2),
        "Mar" => Ok(3),
        "Apr" => Ok(4),
        "May" => Ok(5),
        "Jun" => Ok(6),
        "Jul" => Ok(7),
        "Aug" => Ok(8),
        "Sep" => Ok(9),
        "Oct" => Ok(10),
        "Nov" => Ok(11),
        "Dec" => Ok(12),
        _ => Err(failure::err_msg(format!("could not parse month {:?}", mmm))),
    }
}
// return the timestamp and wether the year was missing
pub fn to_posix_timestamp(
    line: &str,
    regex: &Regex,
    year: Option<i32>,
    time_offset: Option<i64>,
) -> Result<(i64, bool), failure::Error> {
    let caps = regex
        .captures(line)
        .ok_or_else(|| failure::err_msg("no captures in regex"))?;
    if caps.name(ABSOLUTE_MS_GROUP).is_some() && caps.len() == 1 + 1 {
        // only one matched group in addition to the full match
        let abs_ms_capt = caps.name(ABSOLUTE_MS_GROUP).expect("was verified");
        let absolute_ms: i64 = abs_ms_capt.as_str().parse()?;
        return Ok((absolute_ms - time_offset.unwrap_or(0), false));
    }
    let day_capt = caps
        .name(DAY_GROUP)
        .ok_or_else(|| failure::err_msg("no group for days found in regex"))?;
    let day: u32 = day_capt.as_str().parse()?;
    let month = match caps.name(MONTH_GROUP) {
        Some(month_capt) => month_capt
            .as_str()
            .parse()
            .map_err(|e| failure::err_msg(format!("could not parse month: {}", e))),
        None => match caps.name(MONTH_SHORT_NAME_GROUP) {
            Some(month_short_name) => parse_from_month(month_short_name.as_str()),
            None => Err(failure::err_msg("no group for month found in regex")),
        },
    }?;
    let hour_capt = caps
        .name(HOUR_GROUP)
        .ok_or_else(|| failure::err_msg("no group for hour found in regex"))?;
    let min_capt = caps
        .name(MINUTE_GROUP)
        .ok_or_else(|| failure::err_msg("no group for minute found in regex"))?;
    let sec_capt = caps
        .name(SECONDS_GROUP)
        .ok_or_else(|| failure::err_msg("no group for seconds found in regex"))?;
    let mil_str = match caps.name(FRACTION_GROUP) {
        Some(m) => m.as_str(),
        None => "0",
    };
    let am_pm_hour_offset: u32 = match caps.name(AM_PM_GROUP) {
        Some(m) => match m.as_str() {
            "PM" => 12,
            _ => 0,
        },
        None => 0,
    };
    let (hour, minutes, seconds, millis): (u32, u32, u32, u32) = (
        hour_capt
            .as_str()
            .parse()
            .map(|h: u32| h + am_pm_hour_offset)?,
        min_capt.as_str().parse()?,
        sec_capt.as_str().parse()?,
        mil_str.parse()?,
    );

    let timezone_n = caps.name(TIMEZONE_GROUP);
    if time_offset.is_none() && timezone_n.is_none() {
        return Err(failure::err_msg(
            "timestamp cannot be applied, timezone not known",
        ));
    }
    let offset_result = if time_offset.is_none() {
        parse_timezone(&caps[TIMEZONE_GROUP])
    } else {
        time_offset.ok_or_else(|| failure::err_msg("could not detect timestamp in (line {})"))
    };
    // for the year first try YYYY, then yy, then fallback on the supplied year
    let the_year: Option<i32> = match caps.name(YEAR_GROUP) {
        Some(year_str) => year_str.as_str().parse().ok(),
        None => match caps.name(YEAR_SHORT_GROUP) {
            Some(year_short_str) => year_short_str
                .as_str()
                .parse()
                .map(|ys: i32| ys + 2000i32)
                .ok(),
            None => year,
        },
    };
    match (the_year, offset_result) {
        (Some(y), Ok(offset)) => {
            let date_time: Option<NaiveDateTime> = NaiveDate::from_ymd_opt(y, month, day)
                .and_then(|d| d.and_hms_milli_opt(hour, minutes, seconds, millis));
            match date_time {
                Some(dt) => Ok((dt.timestamp_millis() - offset, false)),
                None => Err(failure::err_msg(
                    "error while parsing year/month/day/hour/minute/seconds",
                )),
            }
        }
        (None, Ok(offset)) => {
            let y = Utc::now().year();
            let date_time: Option<NaiveDateTime> = NaiveDate::from_ymd_opt(y, month, day)
                .and_then(|d| d.and_hms_milli_opt(hour, minutes, seconds, millis));
            match date_time {
                Some(dt) => Ok((dt.timestamp_millis() - offset, true)),
                None => Err(failure::err_msg(
                    "error while parsing year/month/day/hour/minute/seconds",
                )),
            }
        }
        (Some(_), Err(e)) => Err(failure::err_msg(format!(
            "could not determine the timezone or offset! ({})",
            e
        ))),
        (None, Err(_)) => Err(failure::err_msg(
            "could not determine the year and timezone or offset!",
        )),
    }
}
#[allow(clippy::too_many_arguments)]
pub fn line_to_timed_line(
    line: &str,
    original_line_length: usize,
    tag: &str,
    regex: &Regex,
    year: Option<i32>,
    time_offset: Option<i64>,
    line_nr: usize,
    reporter: &mut Reporter,
) -> Result<TimedLine, failure::Error> {
    match to_posix_timestamp(line, regex, year, time_offset) {
        Ok((posix_timestamp, year_was_missing)) => Ok(TimedLine {
            timestamp: posix_timestamp,
            content: line.to_string(),
            tag: tag.to_string(),
            original_length: original_line_length,
            year_was_missing,
            line_nr,
        }),
        Err(e) => {
            reporter.add_to_report(
                Severity::WARNING,
                format!("could not extract timestamp: {}", e),
            );
            Err(e)
        }
    }
}
pub fn detect_timestamp_format_in_file(path: &Path) -> Result<String, failure::Error> {
    let f: fs::File = fs::File::open(path)?;
    let mut reader: BufReader<&std::fs::File> = BufReader::new(&f);

    let mut buf = vec![];
    let mut inspected_lines = 0;
    let mut matched_format: BTreeMap<String, usize> = BTreeMap::default();
    let mut last_match: Option<String> = None;

    while let Ok(len) = reader.read_until(b'\n', &mut buf) {
        if len == 0 {
            break; // file is done
        }
        let s = unsafe { std::str::from_utf8_unchecked(&buf) }.trim();
        if !s.is_empty() {
            if let Ok(format) = detect_timeformat_in_string(s, last_match.as_ref()) {
                last_match = Some(format.clone());
                *matched_format.entry(format).or_insert(0) += 1;
            }
            inspected_lines += 1;
        }
        buf = vec![];
        if inspected_lines > MAX_LINES_TO_INSPECT {
            break;
        }
    }
    let best_format_string =
        matched_format.into_iter().fold(
            (None, 0usize),
            |acc, (k, v)| {
                if acc.1 < v {
                    (Some(k), v)
                } else {
                    acc
                }
            },
        );
    let min_matched_lines = std::cmp::min(LINE_DETECTION_THRESHOLD, inspected_lines);
    if let (Some(s), n) = best_format_string {
        if n >= min_matched_lines {
            return Ok(s.clone());
        } else {
            let warning = format!(
                "could not verify enough timestamps in {:?}, only found {}",
                path, n
            );
            return Err(failure::err_msg(warning));
        }
    }

    Err(failure::err_msg(format!(
        "could not detect timestamp in {:?}",
        path
    )))
}
/// Trys to detect a valid timestamp in a string
/// Returns the a pair of the timestamp as posix timestamp and if the year was missing
/// in case the year was missing, we assume the current year (local time)
///
/// # Arguments
///
/// * `input` - A string slice that should be parsed
pub fn detect_timestamp_in_string(
    input: &str,
    offset: Option<i64>,
) -> Result<(i64, bool, String), failure::Error> {
    let trimmed = input.trim();
    for format in AVAILABLE_REGEXES.iter() {
        let regex = &FORMAT_REGEX_MAPPING[format];
        if regex.is_match(trimmed) {
            if let Ok((timestamp, year_missing)) = to_posix_timestamp(input, regex, None, offset) {
                return Ok((timestamp, year_missing, format.to_string()));
            }
        }
    }
    Err(failure::err_msg("try to detect timestamp but no match"))
}
/// Trys to detect a valid time-format in a string
/// Returns the found format if any
///
/// # Arguments
///
/// * `input` - A string slice that should be examined
pub fn detect_timeformat_in_string(
    input: &str,
    last_match: Option<&String>,
) -> Result<String, failure::Error> {
    let trimmed = input.trim();
    // if we already had a match, try this first
    if let Some(last) = last_match {
        let l: &str = last.as_ref();
        if let Some(regex) = FORMAT_REGEX_MAPPING.get(l) {
            if regex.is_match(trimmed) {
                return Ok(last.clone());
            }
        }
    }
    for format in AVAILABLE_REGEXES.iter() {
        let regex = &FORMAT_REGEX_MAPPING[format];
        // println!("check with regex: {}", regex.to_string());
        if regex.is_match(trimmed) {
            return Ok(format.to_string());
        }
    }
    Err(failure::err_msg(format!(
        "no timestamp match found in {}",
        input
    )))
}

/// should parse timezone string, valid formats are
/// +hh:mm, +hhmm, or +hh
/// -hh:mm, -hhmm, or -hh
/// results in the offset in milliseconds
fn timezone_parser(input: &str) -> IResult<&str, i64> {
    let timezone_sign = map(nom::branch::alt((char('+'), char('-'))), |c| c == '+');
    fn timezone_count(input: &str) -> IResult<&str, i64> {
        let (rest, r) = nom::bytes::complete::take(2usize)(input)?;
        let second = map_res(digit1, |s: &str| s.parse::<i64>())(r)?;
        Ok((rest, second.1))
    }
    fn timezone_minutes(input: &str) -> IResult<&str, i64> {
        nom::sequence::preceded(opt(char(':')), timezone_count)(input)
    }
    let parser = nom::sequence::tuple((timezone_sign, timezone_count, opt(timezone_minutes)));
    map(parser, |(positiv, hour, min): (bool, i64, Option<i64>)| {
        let absolute = 1000 * (3600 * hour + 60 * min.unwrap_or(0));
        (if positiv { 1 } else { -1 }) * absolute
    })(input)
}
fn parse_timezone(input: &str) -> Result<i64, failure::Error> {
    match timezone_parser(input) {
        Ok((_, res)) => Ok(res),
        Err(e) => Err(failure::err_msg(format!("error parsing timezone: {:?}", e))),
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    use proptest::prelude::*;

    static VALID_TIMESTAMP_FORMAT: &str = "[+-]{1}[0-9]{2}[0-5]{1}[0-9]{1}";

    proptest! {
        #[test]
        fn offset_from_timezone_in_ms_doesnt_crash(s in "\\PC*") {
            let _ = parse_timezone(&s);
        }
        #[test]
        fn parses_all_valid_dates(s in VALID_TIMESTAMP_FORMAT) {
            parse_timezone(&s).unwrap();
        }
    }

    #[test]
    fn test_date_parsers() {
        assert_eq!(any_date_format("MMM23"), Ok(("23", FormatPiece::MonthName)));
        assert_eq!(any_date_format("DD23"), Ok(("23", FormatPiece::Day)));
        assert_eq!(
            any_date_format("sss23"),
            Ok(("23", FormatPiece::AbsoluteMilliseconds))
        );
        assert_eq!(any_date_format("MM23"), Ok(("23", FormatPiece::Month)));
        assert_eq!(any_date_format("DDMM"), Ok(("MM", FormatPiece::Day)));
        assert_eq!(any_date_format("YYYY-"), Ok(("-", FormatPiece::Year)));
        assert_eq!(any_date_format("yy-"), Ok(("-", FormatPiece::YearShort)));

        assert_eq!(
            any_date_format("-YYYY"),
            Ok(("YYYY", FormatPiece::SeperatorChar('-')))
        );
    }

    #[test]
    fn test_offset_from_timezone_in_ms_non_ascii() {
        assert!(parse_timezone("aࡠA").is_err());
    }
    #[test]
    fn test_offset_from_timezone_in_ms_invalid_input() {
        assert!(parse_timezone("0Aaa0").is_err());
    }

    #[test]
    fn test_timezone_parser() {
        if let Ok(res) = parse_timezone("+01:00") {
            dbg!(res);
        } else {
            println!("could not parse");
        }
    }
    #[test]
    fn test_offset_from_timezone_in_ms() {
        assert_eq!(0, parse_timezone("+0000").expect("could not parse"));
        assert_eq!(0, parse_timezone("-0000").expect("could not parse"));
        assert_eq!(
            2 * 3600 * 1000,
            parse_timezone("+0200").expect("could not parse")
        );
        assert_eq!(
            2 * 3600 * 1000,
            parse_timezone("+02:00").expect("could not parse")
        );
        assert_eq!(
            2 * 3600 * 1000 + 30 * 60 * 1000,
            parse_timezone("+0230").expect("could not parse")
        );
        assert_eq!(
            -2 * 3600 * 1000,
            parse_timezone("-0200").expect("could not parse")
        );
        assert_eq!(
            -30 * 60 * 1000,
            parse_timezone("-00:30").expect("could not parse")
        );
    }
}
