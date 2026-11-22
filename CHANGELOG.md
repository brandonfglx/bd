# Release v0.0.1
What's new:
 - Basic functionality is now present, including `init`, `add`, `rm`, and `build`
 - `bd` executable requires a valid version to be pulled from the releases page for easy building to work
 - Basic default webpage showing rendered HTML
 - Uses minijinja to create formatted HTMl outputs, slug to create dir names, clap to parse cli inputs

Known Issues:
 - No options for changing HTML file
 - Potentially broken `bd` builds using GitHub Actions
 - Code is tested, but functionality is not