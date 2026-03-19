pub struct Reporter<'a> {
    lines: Vec<&'a str>,
}

impl<'a> Reporter<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            lines: source.lines().collect(),
        }
    }

    pub fn report(&self, line: usize, col: usize, len: usize, message: &str, level: &str) {
        println!("{}: {}", level, message);
        if line > 0 && line <= self.lines.len() {
            let line_content = self.lines[line - 1];
            println!("{:>4} | {}", line, line_content);
            let padding = " ".repeat(col + 6); // 4 (line num) + 3 ( | )
            let highlight = if len > 0 { "~".repeat(len) } else { "^".to_string() };
            println!("{}{}", padding, highlight);
        }
        println!();
    }

    pub fn error(&self, line: usize, col: usize, len: usize, message: &str) {
        self.report(line, col, len, message, "ERROR");
    }
}
