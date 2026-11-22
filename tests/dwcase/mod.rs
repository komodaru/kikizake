//! A DirectWrite case of the generated fonts, for the laws that run the model's dw* lines.
use kikizake::generators::dwrite::font;

/// the lines the model answers for these lines after loading font `seed`
pub fn run(seed: u32, lines: &[String]) -> Vec<String> {
    let hex: String = font(seed).bytes().iter().map(|x| format!("{x:02x}")).collect();
    let text = format!("case c\ndwfont {hex}\n{}\nend\n", lines.join("\n"));
    kikizake::model::run_case(&kikizake::case::parse_cases(&text)[0]).unwrap()
}

/// the answers of each dwrun line: its lines from its "dwrun hr" on
#[allow(dead_code)]
pub fn runs(out: &[String]) -> Vec<Vec<String>> {
    let mut v: Vec<Vec<String>> = Vec::new();
    for l in out.iter().skip(2) {
        if l.starts_with("dwrun hr") {
            v.push(Vec::new());
        }
        if let Some(last) = v.last_mut() {
            last.push(l.clone());
        }
    }
    v
}
