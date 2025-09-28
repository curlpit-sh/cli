use std::collections::{HashMap, HashSet};

pub fn apply_header_rules(
    headers: Vec<(String, String)>,
    include: Option<&[String]>,
    exclude: Option<&[String]>,
    append: Option<&HashMap<String, String>>,
) -> Vec<(String, String)> {
    let include_set = include.filter(|slice| !slice.is_empty()).map(|slice| {
        slice
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect::<HashSet<_>>()
    });
    let exclude_set = exclude.filter(|slice| !slice.is_empty()).map(|slice| {
        slice
            .iter()
            .map(|s| s.to_ascii_lowercase())
            .collect::<HashSet<_>>()
    });

    let mut present: HashSet<String> = HashSet::new();
    let mut result = Vec::with_capacity(headers.len());

    for (name, value) in headers.into_iter() {
        let lower = name.to_ascii_lowercase();
        if let Some(ref include) = include_set {
            if !include.contains(&lower) {
                continue;
            }
        }
        if let Some(ref exclude) = exclude_set {
            if exclude.contains(&lower) {
                continue;
            }
        }
        present.insert(lower);
        result.push((name, value));
    }

    if let Some(map) = append {
        let mut entries: Vec<_> = map.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(b.0));
        for (name, value) in entries {
            let lower = name.to_ascii_lowercase();
            if present.insert(lower) {
                result.push((name.clone(), value.clone()));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn include_and_exclude_filters_headers() {
        let headers = vec![
            ("Accept".to_string(), "*/*".to_string()),
            ("User-Agent".to_string(), "curl/8".to_string()),
            ("X-Auth".to_string(), "token".to_string()),
        ];
        let include = vec!["Accept".to_string(), "X-Auth".to_string()];
        let exclude = vec!["X-Auth".to_string()];

        let result = apply_header_rules(headers, Some(&include), Some(&exclude), None);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "Accept");
    }

    #[test]
    fn append_headers_adds_missing_entries() {
        let headers = vec![("Accept".to_string(), "*/*".to_string())];
        let mut append = HashMap::new();
        append.insert("X-Trace".to_string(), "abc".to_string());
        append.insert("Accept".to_string(), "application/json".to_string());

        let result = apply_header_rules(headers, None, None, Some(&append));
        assert_eq!(result.len(), 2);
        assert!(result.iter().any(|(n, v)| n == "X-Trace" && v == "abc"));
        assert!(result.iter().any(|(n, v)| n == "Accept" && v == "*/*"));
    }
}
