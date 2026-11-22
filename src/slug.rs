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
