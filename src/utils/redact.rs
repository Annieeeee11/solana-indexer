/// Redact credentials from a database URL for safe logging.
pub fn redact_database_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let Some(at) = url[scheme_end + 3..].find('@') else {
        return url.to_string();
    };
    let at = scheme_end + 3 + at;
    format!("{}://***:***@{}", &url[..scheme_end], &url[at + 1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_postgres_credentials() {
        let url = "postgresql://postgres:secret@db.example.com:5432/postgres?sslmode=require";
        assert_eq!(
            redact_database_url(url),
            "postgresql://***:***@db.example.com:5432/postgres?sslmode=require"
        );
    }

    #[test]
    fn leaves_urls_without_credentials_unchanged() {
        let url = "postgresql://db.example.com:5432/postgres";
        assert_eq!(redact_database_url(url), url);
    }
}
