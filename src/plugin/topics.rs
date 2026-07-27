//! MQTT topic-filter matching, shared by the plugins that map rules/schemas to
//! topics (`topic-alerts`, `json-schema`).

/// Whether an MQTT topic-filter matches a concrete topic. `+` matches exactly
/// one level; `#` matches the remaining levels (and must be the last segment).
pub(crate) fn matches(filter: &str, topic: &str) -> bool {
    let f: Vec<&str> = filter.split('/').collect();
    let t: Vec<&str> = topic.split('/').collect();
    let mut fi = 0;
    let mut ti = 0;
    while fi < f.len() {
        match f[fi] {
            "#" => return true,
            "+" => {
                if ti >= t.len() {
                    return false;
                }
                fi += 1;
                ti += 1;
            }
            seg => {
                if ti >= t.len() || t[ti] != seg {
                    return false;
                }
                fi += 1;
                ti += 1;
            }
        }
    }
    ti == t.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wildcards() {
        assert!(matches("a/b", "a/b"));
        assert!(!matches("a/b", "a/c"));
        assert!(matches("a/+/c", "a/x/c"));
        assert!(!matches("a/+/c", "a/x/y"));
        assert!(matches("a/#", "a/b/c"));
        assert!(matches("a/#", "a"));
        assert!(!matches("a/#", "b"));
    }
}
