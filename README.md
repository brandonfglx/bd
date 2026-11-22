<div align="center">
	<h1>bd</h1>
	<picture>
		<img src="./resources/demo/demo.gif" alt="bd Demo Clip">
	</picture>
	<h3>A small blogging system hosted on git.</h3>
</div>

> [!NOTE]
> bd is currently in alpha. Some features may not be complete! Check out the change log [here.](./CHANGELOG.md)

## Quick Start
1. Download the latest release builds [here.](https://github.com/brandonfglx/bd/releases/latest)
2. Extract the binary from the downloaded file
3. (macOS only) Open the terminal and remove the app quarantine: `xattr -d com.apple.quarantine bd` (Check the FAQ to learn more)
4. Launch bd using the terminal: `./bd --help` (Unix)

## Features
 - init: intializes a new bd directory and scaffolds templates/ and static/
 - add: creates a new page using the local date and prints the UTC offset
 - rm: removes a page with the given name
 - build: generates a static build, copying static/ and applying templates/ overrides

## Technicals

## Layout
 - blog.toml - site config (title, description, author, language, build)
 - slug/page.toml - page metadata (title, description, date, draft, content)
 - slug/page.md - page content by default; other files in the page directory are copied as-is
 - templates/ - optional overrides for base.html, index.html, and post.html
 - static/ - copied to the build root (style.css, CNAME, robots.txt, 404.html, etc.)

Template partials may use `.html`, `.htm`, `.jinja`, `.j2`, `.txt`, or `.xml` extensions.

## Build Safety
`bd build` renders into a temporary directory next to the output and swaps it into place. If rendering or writing fails, the previous build (if present) is left untouched.

The output directory contains a `.bd-output` marker. `bd` only replaces a directory that contains this marker, so builds older than `v0.0.2` will need to use `bd build --force` to replace a directory that bd cannot verify that it created.

## FAQ
 - Test
	- two

## License
bd is licensed under the [GNU General Public License Version 3](./LICENSE).