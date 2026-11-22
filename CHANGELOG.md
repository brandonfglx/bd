# Release v0.0.3
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