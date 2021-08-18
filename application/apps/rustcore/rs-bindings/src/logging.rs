use anyhow::Result;
use log::{Level, LevelFilter, Metadata, Record};
use log4rs::{
    append::file::FileAppender,
    config::{Appender, Config, Root},
    encode::pattern::PatternEncoder,
};
use std::{
    fs,
    io::prelude::*,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Trace
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!(
                "[RUST]:{}({:?}) {} - {}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis(),
                std::thread::current().id(),
                record.level(),
                record.args()
            );
        }
    }

    fn flush(&self) {}
}

pub fn init_logging() -> Result<()> {
    let log_config_path = chipmunk_log_config();
    let logging_correctly_initialized = if log_config_path.exists() {
        // log4rs.yaml exists, try to parse it
        match log4rs::init_file(&log_config_path, Default::default()) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("problems with existing log config ({}), write fresh", e);
                // log4rs.yaml exists, could not parse it
                initialize_from_fresh_yml().is_ok()
            }
        }
    } else {
        // log4rs.yaml did not exists
        initialize_from_fresh_yml().is_ok()
    };
    if !logging_correctly_initialized {
        setup_fallback_logging()?;
    }
    Ok(())
}

pub fn chipmunk_home_dir() -> PathBuf {
    dirs::home_dir()
        .expect("we need to have access to home-dir")
        .join(".chipmunk")
}

pub fn chipmunk_log_config() -> PathBuf {
    chipmunk_home_dir().join("log4rs.yaml")
}

pub fn initialize_from_fresh_yml() -> Result<()> {
    println!("initialize_from_fresh_yml()");
    let home_dir = dirs::home_dir().expect("Could not access home-directory");
    let log_config_path = chipmunk_log_config();
    let indexer_log_path = chipmunk_home_dir().join("chipmunk.indexer.log");
    let launcher_log_path = chipmunk_home_dir().join("chipmunk.launcher.log");
    let log_config_content = std::include_str!("../log4rs.yaml")
        .replace("$INDEXER_LOG_PATH", &indexer_log_path.to_string_lossy())
        .replace("$LAUNCHER_LOG_PATH", &launcher_log_path.to_string_lossy())
        .replace("$HOME_DIR", &home_dir.to_string_lossy());
    remove_entity(&log_config_path)?;
    std::fs::write(&log_config_path, log_config_content)?;
    log4rs::init_file(&log_config_path, Default::default())?;
    Ok(())
}

pub fn remove_entity(entity: &Path) -> Result<()> {
    if !entity.exists() {
        return Ok(());
    }
    if entity.is_dir() {
        std::fs::remove_dir_all(&entity)?;
    } else if entity.is_file() {
        std::fs::remove_file(&entity)?;
    }
    Ok(())
}

pub fn setup_fallback_logging() -> Result<()> {
    let log_path = chipmunk_home_dir().join("chipmunk.launcher.log");
    let appender_name = "startup-appender";
    let logfile = FileAppender::builder()
        .encoder(Box::new(PatternEncoder::new("{d} - {l}:: {m}\n")))
        .build(log_path)?;

    let config = Config::builder()
        .appender(Appender::builder().build(appender_name, Box::new(logfile)))
        .build(
            Root::builder()
                .appender(appender_name)
                .build(LevelFilter::Trace),
        )
        .expect("log4rs config could not be created");

    log4rs::init_config(config).expect("logging could not be initialized");
    Ok(())
}

//     println!("init logging");
//     let home_dir = dirs::home_dir().expect("we need to have access to home-dir");
//     let log_file_path = home_dir.join(".chipmunk").join("chipmunk.indexer.log");
//     let log_config_path = home_dir.join(".chipmunk").join("log4rs.yaml");
//     if !log_config_path.exists() {
//         let log_config_content = std::include_str!("../log4rs.yaml")
//             .replace("$LOG_FILE_PATH", &log_file_path.to_string_lossy())
//             .replace("$HOME_DIR", &home_dir.to_string_lossy());

//         match std::fs::write(&log_config_path, log_config_content) {
//             Ok(_) => (),
//             Err(e) => eprintln!("error while trying to write log config file: {}", e),
//         }
//     } else {
//         // make sure the env variables are correctly replaced
//         let mut content = String::new();
//         {
//             let mut log_config_file = fs::File::open(&log_config_path)?;
//             log_config_file.read_to_string(&mut content)?;
//         }
//         let log_config_content = content
//             .replace("$LOG_FILE_PATH", &log_file_path.to_string_lossy())
//             .replace("$HOME_DIR", &home_dir.to_string_lossy());

//         match fs::write(&log_config_path, &log_config_content) {
//             Ok(_) => (),
//             Err(e) => eprintln!("error while trying to write log config file: {}", e),
//         }
//     }

//     println!("try to init logging system");
//     match log4rs::init_file(&log_config_path, Default::default()) {
//         Ok(_) => (),
//         Err(e) => {
//             eprintln!("could not initialize logging with init_file: {}", e);
//             let log_path = home_dir.join(".chipmunk").join("chipmunk.indexer.log");
//             let appender_name = "indexer-root";
//             let logfile = FileAppender::builder()
//                 .encoder(Box::new(PatternEncoder::new("{d} - {l}:: {m}\n")))
//                 .build(log_path)?;

//             let config = Config::builder()
//                 .appender(Appender::builder().build(appender_name, Box::new(logfile)))
//                 .build(
//                     Root::builder()
//                         .appender(appender_name)
//                         .build(LevelFilter::Warn),
//                 )
//                 .expect("logging config was incorrect");
//             println!("initing log4rs with {:?}", config);
//             match log4rs::init_config(config) {
//                 Ok(_) => println!("logging was initialized"),
//                 Err(e) => println!("problems setting up logging: {}", e),
//             }
//             // anyhow::Context::with_context(, || {
//             //     "logging config could not be applied"
//             // })?;
//         }
//     }
//     Ok(())
// }
