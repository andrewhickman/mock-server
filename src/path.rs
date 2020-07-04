use regex::Regex;

#[derive(Debug)]
pub struct PathRewriter {
    regex: Regex,
    replace: String,
}

impl PathRewriter {
    pub fn new(regex: Regex, replace: String) -> Self {
        PathRewriter { regex, replace }
    }

    pub fn rewrite<'a>(&self, path: &'a str) -> String {
        self.regex.replace(path, self.replace.as_str()).into_owned()
    }
}
