pub fn redact(input: &str) -> String {
    let mut output = input.to_string();
    for key in ["token=", "api_key=", "apikey=", "authorization=", "cookie="] {
        let mut search_from = 0;
        while let Some(relative) = output[search_from..].to_lowercase().find(key) {
            let start = search_from + relative + key.len();
            let end = output[start..]
                .find(['&', ' ', ';'])
                .map(|offset| start + offset)
                .unwrap_or(output.len());
            if end > start {
                output.replace_range(start..end, "***");
            }
            search_from = start + 3;
            if search_from >= output.len() {
                break;
            }
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_common_secret_keys() {
        assert_eq!(redact("/api?token=abc123&q=x"), "/api?token=***&q=x");
        assert_eq!(
            redact("/api?Authorization=Bearer%20xyz"),
            "/api?Authorization=***"
        );
        assert_eq!(redact("/api?q=600519"), "/api?q=600519");
    }
}
