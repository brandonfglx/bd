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
