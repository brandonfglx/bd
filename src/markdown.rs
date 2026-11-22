use pulldown_cmark::{Options, Parser, html};

pub fn render(input: &str) -> String {
	let mut options = Options::empty();
	options.insert(Options::ENABLE_TABLES);
	options.insert(Options::ENABLE_STRIKETHROUGH);
	options.insert(Options::ENABLE_TASKLISTS);
	options.insert(Options::ENABLE_FOOTNOTES);
	options.insert(Options::ENABLE_SMART_PUNCTUATION);

	let parser = Parser::new_ext(input, options);
	let mut output = String::with_capacity(input.len() * 2);
	html::push_html(&mut output, parser);

	output
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn renders_common_markdown() {
		let html = render("# Title\n\n**bold** and *italic*\n");
		assert!(html.contains("<h1>Title</h1>"));
		assert!(html.contains("<strong>bold</strong>"));
		assert!(html.contains("<em>italic</em>"));
	}

	#[test]
	fn renders_tables_and_strikethrough() {
		let html = render("| A | B |\n|---|---|\n| 1 | 2 |\n\n~~hello~~\n");
		assert!(html.contains("table"));
		assert!(html.contains("<del>hello</del>"));
	}

	#[test]
	fn renders_task_lists() {
		let html = render("- [x] done\n- [ ] todo\n");
		assert!(html.contains("type=\"checkbox\""));
		assert!(html.contains("checked"));
	}
}
