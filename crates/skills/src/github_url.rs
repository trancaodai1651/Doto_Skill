//! Phân tích URL GitHub cho import Skills: repo, thư mục con (`/tree/<ref>/<path>`),
//! file skill (`/blob/<ref>/<path>/SKILL.md`), SSH, `owner/repo` rút gọn, `@ref`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GithubTarget {
    pub owner: String,
    pub repo: String,
    /// Ref (branch/tag/commit). Với `/tree/a/b/c` có thể chưa tách được ref chứa dấu `/`;
    /// khi đó `ref_and_path` giữ phần thô để resolve qua API.
    pub git_ref: Option<String>,
    pub sub_path: Option<String>,
    pub ref_and_path: Option<String>,
    /// URL trỏ tới một file SKILL.md cụ thể.
    pub is_file: bool,
}

impl GithubTarget {
    pub fn canonical_repo_url(&self) -> String {
        format!("https://github.com/{}/{}", self.owner, self.repo)
    }
}

fn valid_name(s: &str, max: usize) -> bool {
    !s.is_empty() && s.len() <= max && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.') && s != "." && s != ".."
}

pub fn valid_ref(r: &str) -> bool {
    !r.is_empty()
        && r.len() <= 255
        && !r.starts_with('-')
        && !r.contains("..")
        && !r.ends_with('/')
        && !r.ends_with(".lock")
        && r.chars().all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c))
}

fn clean_path(p: &str) -> Result<Option<String>, String> {
    let parts: Vec<&str> = p.split('/').filter(|s| !s.is_empty()).collect();
    if parts.iter().any(|s| *s == ".." || *s == "." || s.contains('\\')) {
        return Err("đường dẫn con không hợp lệ".into());
    }
    Ok((!parts.is_empty()).then(|| parts.join("/")))
}

/// Phân tích URL; `explicit_ref` (do người dùng nhập riêng) ưu tiên hơn ref trong URL.
pub fn parse(input: &str, explicit_ref: Option<&str>) -> Result<GithubTarget, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("URL trống".into());
    }
    if s.contains('@') && s.contains("://") && s.split("://").nth(1).map(|r| r.split('/').next().unwrap_or("").contains('@')).unwrap_or(false) {
        return Err("không nhúng thông tin đăng nhập vào URL — dùng đăng nhập GitHub của ứng dụng".into());
    }
    let (body, at_ref) = if s.starts_with("git@") {
        (s, None)
    } else if let Some((b, r)) = s.rsplit_once('@') {
        (b, Some(r.to_string()))
    } else {
        (s, None)
    };
    let rest = if let Some(r) = body.strip_prefix("git@github.com:") {
        r.to_string()
    } else if let Some(r) = body
        .strip_prefix("https://github.com/")
        .or_else(|| body.strip_prefix("http://github.com/"))
        .or_else(|| body.strip_prefix("https://www.github.com/"))
        .or_else(|| body.strip_prefix("github.com/"))
    {
        r.to_string()
    } else if !body.contains("://") && body.matches('/').count() == 1 && !body.starts_with('/') {
        body.to_string()
    } else {
        return Err("chỉ hỗ trợ URL github.com".into());
    };
    let rest = rest.split(['?', '#']).next().unwrap_or("").trim_end_matches('/');
    let mut segs = rest.splitn(3, '/');
    let owner = segs.next().unwrap_or("").to_string();
    let repo = segs.next().unwrap_or("").trim_end_matches(".git").to_string();
    if !valid_name(&owner, 39) || !valid_name(&repo, 100) {
        return Err("owner/repo không hợp lệ".into());
    }
    let tail = segs.next().unwrap_or("");
    let mut t = GithubTarget { owner, repo, git_ref: None, sub_path: None, ref_and_path: None, is_file: false };
    if !tail.is_empty() {
        let (kind, remainder) = tail.split_once('/').unwrap_or((tail, ""));
        match kind {
            "tree" | "blob" => {
                if remainder.is_empty() {
                    return Err("URL thiếu ref".into());
                }
                t.is_file = kind == "blob";
                let first_slash = remainder.find('/');
                // Heuristic: ref là đoạn đầu; nếu ref có thể chứa '/', API resolve lại từ ref_and_path.
                let (r, p) = match first_slash {
                    Some(i) => (&remainder[..i], &remainder[i + 1..]),
                    None => (remainder, ""),
                };
                if !valid_ref(r) {
                    return Err("ref không hợp lệ".into());
                }
                t.git_ref = Some(r.to_string());
                t.sub_path = clean_path(p)?;
                t.ref_and_path = Some(remainder.to_string());
                if t.is_file {
                    let file = t.sub_path.clone().unwrap_or_default();
                    if !file.to_ascii_lowercase().ends_with("skill.md") {
                        return Err("link file phải trỏ tới SKILL.md".into());
                    }
                    // import thư mục chứa SKILL.md
                    t.sub_path = file.rsplit_once('/').map(|(d, _)| d.to_string());
                }
            }
            _ => return Err("URL GitHub không phải repo/tree/blob".into()),
        }
    }
    if let Some(r) = at_ref {
        if !valid_ref(&r) {
            return Err("ref không hợp lệ".into());
        }
        t.git_ref = Some(r);
    }
    if let Some(r) = explicit_ref.map(str::trim).filter(|r| !r.is_empty()) {
        if !valid_ref(r) {
            return Err("ref không hợp lệ".into());
        }
        t.git_ref = Some(r.to_string());
    }
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_repo_variants() {
        for u in [
            "https://github.com/anthropics/skills",
            "https://github.com/anthropics/skills/",
            "https://github.com/anthropics/skills.git",
            "github.com/anthropics/skills",
            "git@github.com:anthropics/skills.git",
            "anthropics/skills",
        ] {
            let t = parse(u, None).unwrap_or_else(|e| panic!("{u}: {e}"));
            assert_eq!((t.owner.as_str(), t.repo.as_str()), ("anthropics", "skills"), "{u}");
            assert_eq!(t.git_ref, None);
            assert_eq!(t.sub_path, None);
        }
    }

    #[test]
    fn tree_and_blob_urls() {
        let t = parse("https://github.com/o/r/tree/main/skills/pdf", None).unwrap();
        assert_eq!(t.git_ref.as_deref(), Some("main"));
        assert_eq!(t.sub_path.as_deref(), Some("skills/pdf"));
        let t = parse("https://github.com/o/r/blob/v1.2.0/skills/pdf/SKILL.md", None).unwrap();
        assert!(t.is_file);
        assert_eq!(t.git_ref.as_deref(), Some("v1.2.0"));
        assert_eq!(t.sub_path.as_deref(), Some("skills/pdf"));
        let t = parse("https://github.com/o/r/blob/main/SKILL.md", None).unwrap();
        assert_eq!(t.sub_path, None);
        assert!(parse("https://github.com/o/r/blob/main/README.md", None).is_err());
    }

    #[test]
    fn refs_and_overrides() {
        assert_eq!(parse("o/r@v2", None).unwrap().git_ref.as_deref(), Some("v2"));
        assert_eq!(parse("https://github.com/o/r", Some("abc1234")).unwrap().git_ref.as_deref(), Some("abc1234"));
        assert_eq!(parse("https://github.com/o/r/tree/main", Some("dev")).unwrap().git_ref.as_deref(), Some("dev"));
    }

    #[test]
    fn rejects_malicious_or_foreign() {
        assert!(parse("https://gitlab.com/o/r", None).is_err());
        assert!(parse("https://evil.com/github.com/o/r", None).is_err());
        assert!(parse("https://user:ghp_x@github.com/o/r", None).is_err(), "không nhận token trong URL");
        assert!(parse("https://github.com/o/r/tree/main/../../etc", None).is_err());
        assert!(parse("https://github.com/o/r/tree/--upload-pack=x", None).is_err());
        assert!(parse("https://github.com/../r", None).is_err());
        assert!(parse("o/r", Some("bad ref;rm")).is_err());
        assert!(parse("", None).is_err());
        assert!(parse("https://github.com/o", None).is_err());
    }

    #[test]
    fn strips_query_and_fragment() {
        let t = parse("https://github.com/o/r/tree/main/a?tab=readme#x", None).unwrap();
        assert_eq!(t.sub_path.as_deref(), Some("a"));
    }
}
