use anyhow::{Result, bail};

const MAX_LENGTH: usize = 80;
const RESERVED: &[&str] = &[
	"build", "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7",
	"com8", "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

pub fn slugify(title: &str) -> String {
	slug::slugify(title)
}

pub fn validate(slug: &str) -> Result<()> {
	if slug.is_empty() {
		bail!("slug cannot be empty");
	}

	if slug.len() > MAX_LENGTH {
		bail!("slug must be at most {MAX_LENGTH} characters");
	}

	if slug.starts_with("-") || slug.ends_with("-") {
		bail!("slug cannot start or end with '-'");
	}

	if !slug
		.bytes()
		.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
	{
		bail!("slug may only contain lowercase ASCII letters, digits, and '-'");
	}

	if RESERVED.contains(&slug) {
		bail!("slug {slug:?} is reserved");
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn slugify_lowercases_and_seperates_words() {
		assert_eq!(slugify("Hello, World!"), "hello-world");
		assert_eq!(slugify("Hello World"), "hello-world");
	}

	#[test]
	fn slugify_returns_empty_for_punctuation_only() {
		assert_eq!(slugify("!!!"), "");
	}

	#[test]
	fn validate_accepts_valid_slugs() {
		for slug in ["a", "hello-world", "post-2", "0", "a--b"] {
			assert!(validate(slug).is_ok(), "{slug:?} should be valid");
		}
	}

	#[test]
	fn validate_rejects_empty_and_invalid_characters() {
		for slug in [
			"",
			"Hello",
			"hello_world",
			"hello world",
			"hello.world",
			"hello/world",
			"hello\\world",
			"café",
		] {
			assert!(validate(slug).is_err(), "{slug:?} should be invalid");
		}
	}

	#[test]
	fn validate_rejects_leading_and_trailing_hyphens() {
		assert!(validate("-hello").is_err());
		assert!(validate("hello-").is_err());
	}

	#[test]
	fn validate_rejects_long_slugs() {
		let max = "a".repeat(MAX_LENGTH);
		assert!(validate(&max).is_ok());

		let too_long = "a".repeat(MAX_LENGTH + 1);
		assert!(validate(&too_long).is_err());
	}

	#[test]
	fn validate_rejects_reserved_names() {
		for slug in [
			"build", "con", "prn", "aux", "nul", "com1", "com6", "com9", "lpt1", "lpt9",
		] {
			assert!(validate(slug).is_err(), "{slug:?} should be reserved");
		}
	}
}
