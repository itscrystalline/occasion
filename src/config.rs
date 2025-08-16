use crate::errors::ConfigError;
use std::{
    collections::HashSet,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use chrono::{Month, Weekday};
use colored::Colorize;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub static CONFIG_VAR: &str = "OCCASION_CONFIG";
pub static CONFIG_FILE_NAME: &str = "occasions.json";
pub static SCHEMA: &str = "https://raw.githubusercontent.com/itscrystalline/occasion/refs/heads/main/occasions.schema.json";

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub dates: Vec<TimeRangeMessage>,
    pub multiple_behavior: Option<MultipleBehavior>,
    pub week_start_day: Option<Weekday>,
    #[serde(default)]
    pub imports: Vec<PathBuf>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct TimeRangeMessage {
    pub message: Option<String>,
    pub command: Option<CustomCommand>,
    pub time: Option<TimeRange>,
    pub condition: Option<RunCondition>,
    #[serde(default)]
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct RunCondition {
    pub shell: Option<CustomCommand>,
    pub predicate: Option<String>,
    #[serde(default)]
    pub merge_strategy: MergeStrategy,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub enum MergeStrategy {
    #[serde(alias = "and")]
    #[serde(alias = "both")]
    #[serde(alias = "&")]
    AND,
    #[default]
    #[serde(alias = "or")]
    #[serde(alias = "any")]
    #[serde(alias = "|")]
    OR,
    #[serde(alias = "xor")]
    #[serde(alias = "either")]
    #[serde(alias = "^")]
    XOR,
    #[serde(alias = "nand")]
    NAND,
    #[serde(alias = "nor")]
    #[serde(alias = "neither")]
    NOR,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct CustomCommand {
    pub run: String,
    pub shell: Option<String>,
    pub shell_flags: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize, Default, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub struct TimeRange {
    pub day_of: Option<DayOf>,
    pub week: Option<HashSet<u32>>,
    pub month: Option<HashSet<Month>>,
    pub year: Option<HashSet<i32>>,
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub enum DayOf {
    #[serde(rename = "week")]
    Week(HashSet<Weekday>),
    #[serde(rename = "month")]
    Month(HashSet<u8>),
}
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(deny_unknown_fields)]
pub enum MultipleBehavior {
    #[serde(rename = "first")]
    First,
    #[serde(rename = "last")]
    Last,
    #[serde(rename = "all")]
    All {
        #[serde(default)]
        seperator: String,
    },
    #[serde(rename = "random")]
    Random,
}
impl Default for MultipleBehavior {
    fn default() -> Self {
        Self::All {
            seperator: "".to_string(),
        }
    }
}

impl Config {
    pub fn load_or_default(log: bool) -> Result<Config, ConfigError> {
        match Config::load(log) {
            Ok(conf) => Ok(conf),
            Err(ConfigError::Io(err)) if err.kind() == ErrorKind::NotFound => {
                match Config::save_default() {
                    Ok(()) => Config::load(log),
                    Err(e) => Err(e),
                }
            }
            Err(e) => Err(e),
        }
    }

    fn load(log: bool) -> Result<Config, ConfigError> {
        let file_path_str = std::env::var(CONFIG_VAR).unwrap_or(format!(
            "{}/{}",
            dirs::config_dir()
                .ok_or(ConfigError::UndeterminableConfigLocation)?
                .to_string_lossy(),
            CONFIG_FILE_NAME
        ));
        Self::load_from(&PathBuf::from(file_path_str), log, 0)
    }

    fn load_from(path: &Path, log: bool, depth: u8) -> Result<Config, ConfigError> {
        if depth > 2 {
            return Err(ConfigError::MaxRecursionDepth);
        }

        let contents = std::fs::read_to_string(path)?;
        let mut val: Value = serde_json::from_str(&contents)?;
        let map_val = val.as_object_mut().ok_or(ConfigError::Unknown)?;
        map_val.remove("$schema");

        let canon_dir_path = path
            .canonicalize()?
            .parent()
            .ok_or(ConfigError::NotAFile)?
            .to_path_buf();
        let mut this_config: Config = serde_json::from_value(val)?;
        if !this_config.imports.is_empty() {
            let mut imported: Option<Config> = None;
            for import in this_config.imports.iter() {
                let mut canon = canon_dir_path.clone();
                canon.push(import);
                let path = canon.to_string_lossy();
                let config = match Self::load_from(&canon, log, depth + 1) {
                    Ok(config) => config,
                    Err(e) => {
                        if log {
                            println!(
                                "{}",
                                format!("[warn] cannot import config file at {path}: {e}").yellow()
                            );
                        }
                        continue;
                    }
                };
                if let Some(ref mut imported) = imported {
                    imported.merge(config);
                } else {
                    _ = imported.replace(config)
                }
            }
            if let Some(imported) = imported {
                this_config.merge(imported);
            }
        }

        Ok(this_config)
    }

    fn merge(&mut self, other: Config) {
        self.dates.extend(other.dates);
        if self.multiple_behavior.is_none() {
            if let Some(val) = other.multiple_behavior {
                _ = self.multiple_behavior.replace(val)
            }
        }
        if self.week_start_day.is_none() {
            if let Some(val) = other.week_start_day {
                _ = self.week_start_day.replace(val)
            }
        }
    }

    fn save_default() -> Result<(), ConfigError> {
        let file_path_str = std::env::var(CONFIG_VAR).unwrap_or(format!(
            "{}/{}",
            dirs::config_dir()
                .ok_or(ConfigError::UndeterminableConfigLocation)?
                .to_string_lossy(),
            CONFIG_FILE_NAME
        ));
        Self::save_default_to(&PathBuf::from(file_path_str))
    }

    fn save_default_to(path: &Path) -> Result<(), ConfigError> {
        let mut json = serde_json::to_value(Config::default())?;
        let map_json = json.as_object_mut().ok_or(ConfigError::Unknown)?;
        map_json.insert("$schema".into(), SCHEMA.into());
        let json_pretty = serde_json::to_string_pretty(map_json)?;
        std::fs::write(path, json_pretty)?;
        Ok(())
    }

    #[cfg(test)]
    fn save_this_to(&self, path: &Path) -> Result<(), ConfigError> {
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }
    #[cfg(test)]
    fn save_this(&self) -> Result<(), ConfigError> {
        let file_path_str = std::env::var(CONFIG_VAR).unwrap_or(format!(
            "{}/{}",
            dirs::config_dir()
                .ok_or(ConfigError::UndeterminableConfigLocation)?
                .to_string_lossy(),
            CONFIG_FILE_NAME
        ));
        self.save_this_to(&PathBuf::from(&file_path_str))
    }
    #[cfg(test)]
    fn save_this_with_name(&self, name: &str) -> Result<(), ConfigError> {
        use std::str::FromStr;

        let file_path_str = std::env::var(CONFIG_VAR).unwrap_or(format!(
            "{}/{}",
            dirs::config_dir()
                .ok_or(ConfigError::UndeterminableConfigLocation)?
                .to_string_lossy(),
            CONFIG_FILE_NAME
        ));
        let folder = PathBuf::from_str(&file_path_str).unwrap();
        let mut folder = folder.parent().unwrap().to_path_buf();
        folder.push(name);
        self.save_this_to(&PathBuf::from(&folder))
    }
}

#[cfg(test)]
mod unit_tests {
    use std::{env::temp_dir, str::FromStr};

    use map_macro::hash_set;

    use super::*;

    fn with_var<F: FnOnce()>(run: F) {
        let mut dir = temp_dir();
        dir.push(format!(
            "occasion-test-{}",
            fastrand::u128(u128::MIN..u128::MAX)
        ));
        let dir_str = dir.to_string_lossy();
        let file = format!("{dir_str}/{CONFIG_FILE_NAME}");
        _ = std::fs::create_dir_all(&dir);
        println!("test dir: {dir_str}");
        temp_env::with_var(CONFIG_VAR, Some(file.clone()), move || {
            run();
        });
        _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    /// A default `Config` should be created.
    fn serialize_default() {
        let test_config = Config::default();
        let json = serde_json::to_string(&test_config).unwrap();

        let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
        let dates = decoded["dates"].as_array().unwrap();
        assert!(dates.is_empty());
    }
    #[test]
    /// A properly made `Config` should serialize to a file properly.
    fn serialize_default_to_file() {
        with_var(|| {
            let save_res = Config::save_default();
            assert!(save_res.is_ok());
            let json = std::fs::read_to_string(std::env::var(CONFIG_VAR).unwrap()).unwrap();

            let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
            let dates = decoded["dates"].as_array().unwrap();
            let schema_string = decoded["$schema"].as_str().unwrap();
            assert!(dates.is_empty());
            assert_eq!(schema_string, SCHEMA);
        });
    }
    #[test]
    /// A properly made `Config` should also deserialize properly.
    fn deserialize() {
        let test_config = Config {
            dates: vec![TimeRangeMessage {
                message: Some("hai :3".to_string()),
                time: Some(TimeRange {
                    day_of: Some(DayOf::Month(hash_set! {1,3,5,7,9})),
                    week: None,
                    month: Some(hash_set! {Month::January,Month::June,Month::July}),
                    year: Some(hash_set! {2016,2017,2018,2022,2024,2005,2030}),
                }),
                ..Default::default()
            }],
            multiple_behavior: Some(MultipleBehavior::default()),
            ..Default::default()
        };
        let test_config_2 = Config {
            dates: vec![TimeRangeMessage {
                command: Some(CustomCommand {
                    run: "echo \"Hello!\"".to_string(),
                    shell: None,
                    shell_flags: None,
                }),
                time: Some(TimeRange {
                    day_of: Some(DayOf::Month(hash_set! { 2 })),
                    week: None,
                    month: None,
                    year: None,
                }),
                ..Default::default()
            }],
            multiple_behavior: Some(MultipleBehavior::First),
            ..Default::default()
        };
        let json = serde_json::to_string(&test_config).unwrap();
        let json_2 = serde_json::to_string(&test_config_2).unwrap();

        let decoded_config: Config = serde_json::from_str(&json).unwrap();
        let decoded_config_2: Config = serde_json::from_str(&json_2).unwrap();
        assert_eq!(test_config, decoded_config);
        assert_eq!(test_config_2, decoded_config_2);
    }
    #[test]
    /// A properly made `Config` should also deserialize properly.
    fn deserialize_from_file() {
        with_var(|| {
            let test_config = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                multiple_behavior: Some(MultipleBehavior::default()),
                ..Default::default()
            };

            let json = serde_json::to_string(&test_config).unwrap();
            std::fs::write(std::env::var(CONFIG_VAR).unwrap(), &json).unwrap();

            let decoded_config = Config::load(false).unwrap();
            assert_eq!(test_config, decoded_config);
        });
    }
    #[test]
    /// A properly made `Config` should also deserialize properly.
    fn deserialize_from_broken_file() {
        with_var(|| {
            let test_config = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                multiple_behavior: Some(MultipleBehavior::default()),
                ..Default::default()
            };

            let mut json = serde_json::to_string(&test_config).unwrap();
            json.push_str("lalalalalalalal mreow :3");
            std::fs::write(std::env::var(CONFIG_VAR).unwrap(), &json).unwrap();

            let decoded_config = Config::load(false);
            assert!(matches!(decoded_config, Err(ConfigError::Deserialize(_))));
        });
    }

    #[test]
    fn deserialize_unreadable() {
        with_var(|| {
            // no written config
            let decoded_config = Config::load(false);
            println!("{decoded_config:?}");
            assert!(matches!(decoded_config, Err(ConfigError::Io(_))));
        });
    }

    #[test]
    fn read_default() {
        with_var(|| {
            let config = Config::load_or_default(false);
            assert!(config.is_ok());
            let config = config.unwrap();
            assert!(config.dates.is_empty());
        });
    }
    #[test]
    fn read_existing() {
        with_var(|| {
            let test_config = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                multiple_behavior: Some(MultipleBehavior::default()),
                ..Default::default()
            };
            test_config.save_this().unwrap();

            let read = Config::load_or_default(false);
            assert!(read.is_ok());
            let read = read.unwrap();
            assert_eq!(read, test_config);
        });
    }
    #[test]
    fn import() {
        with_var(|| {
            let root = Config {
                imports: vec![PathBuf::from_str("import_1.json").unwrap()],
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let import_1 = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("hewwo".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                week_start_day: Some(Weekday::Wed),
                multiple_behavior: Some(MultipleBehavior::Random),
                ..Default::default()
            };
            import_1.save_this_with_name("import_1.json").unwrap();
            root.save_this().unwrap();

            let mut root_merge = root.clone();
            root_merge.merge(import_1.clone());

            let read = Config::load_or_default(true).unwrap();
            assert_eq!(read, root_merge);
        });
    }
    #[test]
    fn import_multiple() {
        with_var(|| {
            let root = Config {
                imports: vec![
                    PathBuf::from_str("import_1.json").unwrap(),
                    PathBuf::from_str("import_2.json").unwrap(),
                ],
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let import_1 = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("hewwo".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                week_start_day: Some(Weekday::Wed),
                multiple_behavior: Some(MultipleBehavior::Random),
                ..Default::default()
            };
            let import_2 = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("trans rights !!".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            import_1.save_this_with_name("import_1.json").unwrap();
            import_2.save_this_with_name("import_2.json").unwrap();
            root.save_this().unwrap();

            let mut root_merge = root.clone();
            root_merge.merge(import_1.clone());
            root_merge.merge(import_2.clone());

            let read = Config::load_or_default(true).unwrap();
            assert_eq!(read, root_merge);
        });
    }
    #[test]
    fn import_depth() {
        with_var(|| {
            let root = Config {
                imports: vec![PathBuf::from_str("import_1.json").unwrap()],
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let import_1 = Config {
                imports: vec![PathBuf::from_str("import_2.json").unwrap()],
                dates: vec![TimeRangeMessage {
                    message: Some("hewwo".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let import_2 = Config {
                imports: vec![PathBuf::from_str("import_3.json").unwrap()],
                dates: vec![TimeRangeMessage {
                    message: Some("trans rights !!".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let import_3 = Config {
                dates: vec![TimeRangeMessage {
                    message: Some("this will not be added".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                week_start_day: Some(Weekday::Wed),
                ..Default::default()
            };
            import_1.save_this_with_name("import_1.json").unwrap();
            import_2.save_this_with_name("import_2.json").unwrap();
            import_3.save_this_with_name("import_3.json").unwrap();
            root.save_this().unwrap();

            let mut root_merge = root.clone();
            root_merge.merge(import_1.clone());
            root_merge.merge(import_2.clone());

            let read = Config::load_or_default(true).unwrap();
            assert_eq!(read, root_merge);
        });
    }
    #[test]
    fn import_circular() {
        with_var(|| {
            let root = Config {
                imports: vec![PathBuf::from_str("import_1.json").unwrap()],
                dates: vec![TimeRangeMessage {
                    message: Some("hai :3".to_string()),
                    time: Some(TimeRange {
                        day_of: Some(DayOf::Month(hash_set! { 1, 3, 5, 7, 9 })),
                        week: None,
                        month: Some(hash_set! { Month::January, Month::June, Month::July }),
                        year: Some(hash_set! { 2016, 2017, 2018, 2022, 2024, 2005, 2030 }),
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            let import_1 = Config {
                imports: vec![PathBuf::from_str("occasions.json").unwrap()],
                dates: vec![TimeRangeMessage {
                    message: Some("hewwo".to_string()),
                    condition: Some(RunCondition {
                        predicate: Some("true".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            import_1.save_this_with_name("import_1.json").unwrap();
            root.save_this().unwrap();

            let mut root_merge = root.clone();
            root_merge.merge(import_1.clone());
            root_merge.merge(root.clone());

            let read = Config::load_or_default(true).unwrap();
            assert_eq!(read, root_merge);
        });
    }
}
