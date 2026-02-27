use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::{ForwardMapping, Parameters};

static PARAM_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?m)^\s*(.*?) = (.*?)\r?$").expect("valid parameter regex"));

pub fn read_targets(result: &str) -> Vec<String> {
    if result.contains("Empty") {
        return Vec::new();
    }

    result
        .split('\n')
        .filter(|line| !line.is_empty())
        .map(|line| line.to_string())
        .collect()
}

pub fn read_ports(result: &str, reverse: bool) -> Vec<ForwardMapping> {
    if result.contains("Empty") {
        return Vec::new();
    }

    result
        .split('\n')
        .filter(|line| {
            !line.is_empty()
                && if reverse {
                    line.contains("Reverse")
                } else {
                    line.contains("Forward")
                }
        })
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let target = parts.first().copied().unwrap_or_default().to_string();

            if reverse {
                ForwardMapping {
                    target,
                    local: parts.get(2).copied().unwrap_or_default().to_string(),
                    remote: parts.get(1).copied().unwrap_or_default().to_string(),
                }
            } else {
                ForwardMapping {
                    target,
                    local: parts.get(1).copied().unwrap_or_default().to_string(),
                    remote: parts.get(2).copied().unwrap_or_default().to_string(),
                }
            }
        })
        .collect()
}

pub fn parse_parameters(result: &str) -> Parameters {
    let mut params = Parameters::new();

    for captures in PARAM_RE.captures_iter(result) {
        let key = captures
            .get(1)
            .map(|m| m.as_str())
            .unwrap_or_default()
            .to_string();
        let value = captures
            .get(2)
            .map(|m| m.as_str())
            .unwrap_or_default()
            .to_string();

        params.insert(key, value);
    }

    params
}

#[cfg(test)]
mod tests {
    use super::{parse_parameters, read_ports, read_targets};

    #[test]
    fn parse_targets() {
        assert_eq!(read_targets("Empty target"), Vec::<String>::new());
        assert_eq!(
            read_targets("abc\ndef\n"),
            vec!["abc".to_string(), "def".to_string()]
        );
    }

    #[test]
    fn parse_ports() {
        let forward = read_ports("dev1 tcp:1 tcp:2 Forward\n", false);
        assert_eq!(forward[0].local, "tcp:1");
        assert_eq!(forward[0].remote, "tcp:2");

        let reverse = read_ports("dev1 tcp:2 tcp:1 Reverse\n", true);
        assert_eq!(reverse[0].local, "tcp:1");
        assert_eq!(reverse[0].remote, "tcp:2");
    }

    #[test]
    fn parse_key_values() {
        let params = parse_parameters("a = b\n c = d\r\ninvalid\n");
        assert_eq!(params.get("a"), Some(&"b".to_string()));
        assert_eq!(params.get("c"), Some(&"d".to_string()));
    }
}
