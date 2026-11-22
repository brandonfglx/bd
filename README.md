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
3. (macOS only) Open the terminal and remove the app quarantine: `xattr -d com.apple.quarantine bd` (Check [Troubleshooting](https://github.com/brandonfglx/bd/glob/main/README.md#troubleshooting) to learn more)
4. Launch bd using the terminal: `./bd --help` (Unix)
5. Create a new `bd` project using `./bd init`
6. Now you can create new pages using `./bd add [name]`, build with `./bd build`, and preview with `./bd preview`
7. To publish, you must have [git](http://git-scm.org/) installed. You can download it from your package manager or [the official website](http://git-scm.org/install). Then call `./bd publish` - `bd` may ask you to input the remote repository's URL which should be in HTTP or SSH.

## Features
 - init: initializes a new bd directory and scaffolds `templates/` and `static/`
 - add: creates a new page using the local date and prints the UTC offset
 - rm: removes a page with the given name
 - build: generates a static build, copying `static/` and applying `templates/` overrides
 - preview: open a small web server to preview `build/` files (use `--port` to set the port)
 - publish: automatically stage, commit, and push changes to the remote repository (use `--page` to only publish a page)

## Technicals
`bd` written entirely in Rust. It leverages multiple dependencies including (but not limited to) clap, toml, serde, minijinja, and pulldown-cmark. These dependencies ensure that `bd` relies on the actual user-facing logic.

Dependency usage:
 - clap - used to parse command line arguments
 - toml - parses TOML for a `bd` project
 - serde - manages serialization and deseralization
 - minijinja - allows dynamic static creation of HTML
 - pulldown-cmark - markdown processor (pages use markdown as main markup language)

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

## Troubleshooting
 - Why does macOS warn me about `bd` being dangerous?
	- macOS has a feature called Gatekeeper that prevents certain binaries to be run if they are not verfied by an Apple Developer account. Since this project is small and I do not have the capability to verify `bd`, you will have to manually remove the quarantine protection using `xattr -d com.apple.quarantine [bd executable]`.
 - How do I publish?
	- Publishing using `bd` requires you to configure a remote repository (another location on the internet) through GitHub (or any other hosting service that supports git) and connect it to your local repository. First, create a GitHub repository then execute `git remote add origin [URL]` (substitute URL for your repository's URL) and then call `./bd publish`.
 - How do I upgrade bd?
	- Since `bd` does not have a self-updater, you must install the latest release from GitHub if it's available. You may need to follow the migration guides below to transfer data.

## Migration
Older versions of `bd` relies on older files that may have been changed in a later release. As a result, the `build/` artifacts may be outdated and requires a newer version of `bd` to remedy. Before you upgrade, make sure you create a backup of your projects. To resolve this, install the latest version of `bd` [here]() and add it to your PATH if neccessary. Then inside all your projects that need migrating, save all of your page file directories and then use `bd init` to create a new GitHub repository. Once completed, migrate all of your previous pages over to the new repository. Make sure to add your remote again since this is a new repository.

NOTE: Migrating will lose any commit information from previous builds. To publish, you must force an update to the remote repository. Currently, there is no in-place migration support.

## License
bd is licensed under the [GNU General Public License Version 3](./LICENSE).