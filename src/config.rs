use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct QuietHoursRule {
    pub enabled: bool,
    pub preset: String, // "every_day", "work_days", "weekends", "custom"
    pub start_hour: u32,
    pub end_hour: u32,
    pub days: [bool; 7], // Mon = 0, Sun = 6
    pub overnight: bool,
}

impl QuietHoursRule {
    pub fn serialize(&self) -> String {
        let days_str: String = self.days.iter().map(|&d| if d { '1' } else { '0' }).collect();
        format!(
            "{},{},{},{},{},{}",
            self.enabled, self.preset, self.start_hour, self.end_hour, days_str, self.overnight
        )
    }

    pub fn from_string(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 6 {
            return None;
        }
        let enabled = parts[0].parse::<bool>().ok()?;
        let preset = parts[1].to_string();
        let start_hour = parts[2].parse::<u32>().ok()?;
        let end_hour = parts[3].parse::<u32>().ok()?;
        
        let days_chars: Vec<char> = parts[4].chars().collect();
        if days_chars.len() != 7 {
            return None;
        }
        let mut days = [false; 7];
        for i in 0..7 {
            days[i] = days_chars[i] == '1';
        }
        let overnight = parts[5].parse::<bool>().ok()?;

        Some(QuietHoursRule {
            enabled,
            preset,
            start_hour,
            end_hour,
            days,
            overnight,
        })
    }
}

pub fn parse_rules(s: &str) -> Vec<QuietHoursRule> {
    let mut rules = Vec::new();
    if s.is_empty() {
        return rules;
    }
    for item in s.split(';') {
        if let Some(rule) = QuietHoursRule::from_string(item) {
            rules.push(rule);
        }
    }
    rules
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub enabled: bool,
    pub interval_mins: u32,
    pub volume: u32,
    pub quiet_hours_enabled: bool,
    pub quiet_hours_rules: String, // format: "enabled,preset,start,end,days,overnight;..."
    pub run_at_startup: bool,
    pub first_launch: bool,
}

impl AppConfig {
    pub fn default() -> Self {
        Self {
            enabled: true,
            interval_mins: 30,
            volume: 80,
            quiet_hours_enabled: false,
            quiet_hours_rules: "true,every_day,22,11,1111111,true;true,work_days,9,17,1111100,false".to_string(),
            run_at_startup: false,
            first_launch: true,
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Self {
        let mut config = Self::default();
        if !path.as_ref().exists() {
            config.save_to_file(path);
            return config;
        }

        if let Ok(mut file) = File::open(path) {
            let mut contents = String::new();
            if file.read_to_string(&mut contents).is_ok() {
                for line in contents.lines() {
                    let trimmed = line.trim();
                    if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with('#') {
                        continue;
                    }
                    if let Some((key, val)) = trimmed.split_once('=') {
                        let key = key.trim().to_lowercase();
                        let val = val.trim();
                        match key.as_str() {
                            "enabled" => config.enabled = val.parse().unwrap_or(true),
                            "interval_mins" => config.interval_mins = val.parse().unwrap_or(30),
                            "volume" => config.volume = val.parse().unwrap_or(80).min(100),
                            "quiet_hours_enabled" => config.quiet_hours_enabled = val.parse().unwrap_or(false),
                            "quiet_hours_rules" => config.quiet_hours_rules = val.to_string(),
                            "run_at_startup" => config.run_at_startup = val.parse().unwrap_or(false),
                            "first_launch" => config.first_launch = val.parse().unwrap_or(true),
                            _ => {}
                        }
                    }
                }
            }
        }
        config
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) {
        if let Ok(mut file) = File::create(path) {
            let content = format!(
                "[Settings]\n\
                 enabled = {}\n\
                 interval_mins = {}\n\
                 volume = {}\n\
                 quiet_hours_enabled = {}\n\
                 quiet_hours_rules = {}\n\
                 run_at_startup = {}\n\
                 first_launch = {}\n",
                self.enabled,
                self.interval_mins,
                self.volume,
                self.quiet_hours_enabled,
                self.quiet_hours_rules,
                self.run_at_startup,
                self.first_launch
            );
            let _ = file.write_all(content.as_bytes());
        }
    }
}