# Release v0.1.1
What's new:
 - Added `--tag` option to `add` and `rm` to add tags using the CLI instead of modifying the page.toml
 - Added a custom 404 page - replace by having the file `static/404.html`
 - Users can now set the favicon for their blog using the `favicon` attribute in blog.toml (can be a url or a file in `static/`)
 - Added custom footers - add multiple `[[footer]]` entries each with their own `title` and `url` attributes

# Release v0.1.1
What's new:
 - Added a tag system - posts can now have tags which can be viewed on its own tag page
 - Added a search system - search using titles, description, or tag; statically generated and processed locally
 - Added a migration system - run `bd init --upgrade [dir]` to upgrade `static/` and `templates/` to the latest default versions and `bd init --force [dir]` to replace all generated files; both commands leave user created files and directories untouched

# Release v0.1.0
A minor update to fix some backend issues and fix up template HTML. Also, bump version to the first beta!

What's new:
 - Fixes to CI/CD pipeline
 - Changed template CSS to be more monotone/grayscale

# Release v0.0.3
What's new:
 - The default template now supports dark mode - changes depending on device preference
 - `bd` can now directly add, commit, and push information to a remote repository using `bd publish [slug]` - update only a slug, if provided; otherwise update all pages
 - The default template has better formatting - page title, date, and description are more aligned and expanded horizontally
 - `bd` can now display a preview of the latest build by running a small web server on the local machine - use `bd preview` to try it out

# Release v0.0.2
What's new:
 - init now scaffolds templates/, static/, and a blog README
 - add now uses the local date and reports the numeric UTC offset
 - build now copies static/ directly into the build root (used for site-wide resources)
 - templates/ overrides embedded templates and supports partials
 - templates and static are reserved slugs and build-output locations
 - build now renders atomically and preserves the previous site if build fails
 - build now writes a `.bd-output` marker and requires `--force` to be used when migrating older build directories

# Release v0.0.1
What's new:
 - Basic functionality is now present, including `init`, `add`, `rm`, and `build`
 - `bd` executable requires a valid version to be pulled from the releases page for easy building to work
 - Basic default webpage showing rendered HTML
 - Uses minijinja to create formatted HTML outputs, slug to create dir names, clap to parse cli inputs

Known Issues:
 - No options for changing HTML file
 - Potentially broken `bd` builds using GitHub Actions
 - Code is tested, but functionality was not