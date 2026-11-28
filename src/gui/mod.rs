use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread::JoinHandle;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

use crate::commands::add::default_page_markdown;
use crate::commands::{build, init, publish};
use crate::config::{BLOG_CONFIG, BlogConfig, FooterLink};
use crate::git;
use crate::page::{PageFile, format_date, today};
use crate::server::{PORT, PreviewServer};
use crate::slug;
use crate::theme::ThemeName;

pub fn run(path: Option<PathBuf>) -> Result<()> {
	let options = eframe::NativeOptions {
		viewport: egui::ViewportBuilder::default()
			.with_inner_size([1280.0, 820.0])
			.with_maximized(true),
		..Default::default()
	};

	eframe::run_native(
		"bd",
		options,
		Box::new(move |cc| Ok(Box::new(App::new(cc, path)))),
	)
	.map_err(|error| anyhow::anyhow!("gui: {error}"))
}

struct App {
	project: Option<Project>,
	welcome: WelcomeState,
	log: Vec<String>,
}

impl App {
	fn new(cc: &eframe::CreationContext<'_>, path: Option<PathBuf>) -> Self {
		cc.egui_ctx.set_visuals(egui::Visuals::dark());

		let start = path.or_else(|| std::env::current_dir().ok());
		let mut app = Self {
			project: None,
			welcome: WelcomeState::new(start.clone()),
			log: Vec::new(),
		};

		if let Some(start) = start
			&& start.join(BLOG_CONFIG).is_file()
		{
			app.open(start);
		}

		app
	}

	fn open(&mut self, root: PathBuf) {
		match Project::open(root) {
			Ok(project) => {
				self.project = Some(project);
				self.log.clear();
			}
			Err(error) => self.log.push(format!("{error:#}")),
		}
	}

	fn welcome_ui(&mut self, ui: &mut egui::Ui) {
		let mut browse = false;
		let mut open = false;
		let mut create = false;

		egui::CentralPanel::default().show(ui, |ui| {
			ui.vertical_centered(|ui| {
				ui.add_space(60.0);
				ui.heading(egui::RichText::new("bd").size(42.0).strong());
				ui.label("A small blogging system hosted on git.");
				ui.add_space(24.0);
				ui.separator();
				ui.add_space(12.0);

				ui.label("Project folder");
				ui.add(
					egui::TextEdit::singleline(&mut self.welcome.path)
						.desired_width(520.0)
						.hint_text("Enter path to blog"),
				);

				ui.horizontal(|ui| {
					if ui.button("Browse...").clicked() {
						browse = true;
					}

					if ui.button("Open existing").clicked() {
						open = true;
					}
				});

				ui.add_space(16.0);
				ui.label("New project title");
				ui.add(egui::TextEdit::singleline(&mut self.welcome.title).desired_width(320.0));

				egui::ComboBox::from_label("Theme")
					.selected_text(self.welcome.theme.as_str())
					.show_ui(ui, |ui| {
						for theme in ThemeName::ALL {
							ui.selectable_value(&mut self.welcome.theme, theme, theme.as_str());
						}
					});

				if ui.button("Create new project").clicked() {
					create = true;
				}

				if !self.log.is_empty() {
					ui.add_space(16.0);
					ui.collapsing("Log", |ui| {
						for line in &self.log {
							ui.monospace(line);
						}
					});
				}
			});
		});

		if browse && let Some(folder) = rfd::FileDialog::new().pick_file() {
			self.welcome.path = folder.display().to_string();
		}

		if open {
			let path = PathBuf::from(self.welcome.path.trim());
			if path.as_os_str().is_empty() {
				self.log.push("Choose a project folder first.".to_string());
			} else {
				self.open(path);
			}
		}

		if create {
			let path = PathBuf::from(self.welcome.path.trim());
			if path.as_os_str().is_empty() {
				self.log.push("Choose a project folder first.".to_string());
				return;
			}

			match init::run(&path, false, false, None, Some(self.welcome.theme)) {
				Ok(()) => self.open(path),
				Err(error) => self.log.push(format!("{error:#}")),
			}
		}
	}
}

impl eframe::App for App {
	fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
		if let Some(project) = &mut self.project {
			project.ui(ui);
		} else {
			self.welcome_ui(ui);
		}
	}
}

struct WelcomeState {
	path: String,
	title: String,
	theme: ThemeName,
}

impl WelcomeState {
	fn new(start: Option<PathBuf>) -> Self {
		Self {
			path: start
				.map(|path| path.display().to_string())
				.unwrap_or_default(),
			title: "Default Blog".to_string(),
			theme: ThemeName::Classic,
		}
	}
}

struct Project {
	root: PathBuf,
	config: BlogConfig,
	pages: Vec<PageSummary>,
	selected: Option<usize>,
	editor: Option<PageEditor>,
	settings: SettingsState,
	view: View,
	preview: Option<PreviewHandle>,
	new_page: Option<NewPageDialog>,
	job: Option<Job>,
	log: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
	Pages,
	Settings,
}

#[derive(Debug, Clone)]
struct PageSummary {
	dir: PathBuf,
	slug: String,
	title: String,
	date: String,
	draft: bool,
}

struct PageEditor {
	dir: PathBuf,
	meta: PageFile,
	tags_text: String,
	markdown: String,
	saved_meta: PageFile,
	saved_tags: String,
	saved_markdown: String,
	cache: CommonMarkCache,
	scroll: ScrollSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScrollPane {
	Editor,
	Preview,
}

struct ScrollSync {
	leader: ScrollPane,
	offset: f32,
	editor_offset: f32,
	preview_offset: f32,
	editor_max: f32,
	preview_max: f32,
	initialized: bool,
}

impl Default for ScrollSync {
	fn default() -> Self {
		Self {
			leader: ScrollPane::Editor,
			offset: 0.0,
			editor_offset: 0.0,
			preview_offset: 0.0,
			editor_max: 0.0,
			preview_max: 0.0,
			initialized: false,
		}
	}
}

impl PageEditor {
	fn load(dir: PathBuf) -> Result<Self> {
		let meta = PageFile::load(&dir)?;
		let markdown = fs::read_to_string(dir.join(&meta.content))
			.with_context(|| format!("failed to read {}", dir.join(&meta.content).display()))?;
		let tags_text = meta.tags.join(", ");

		Ok(Self {
			dir,
			meta: meta.clone(),
			tags_text: tags_text.clone(),
			markdown: markdown.clone(),
			saved_meta: meta,
			saved_tags: tags_text,
			saved_markdown: markdown,
			cache: CommonMarkCache::default(),
			scroll: ScrollSync::default(),
		})
	}

	fn slug(&self) -> String {
		self.dir
			.file_name()
			.and_then(|name| name.to_str())
			.unwrap_or_default()
			.to_string()
	}

	fn dirty(&self) -> bool {
		self.meta != self.saved_meta
			|| self.tags_text != self.saved_tags
			|| self.markdown != self.saved_markdown
	}

	fn apply_tags(&mut self) {
		self.meta.tags = self
			.tags_text
			.split(',')
			.map(str::trim)
			.filter(|tag| !tag.is_empty())
			.map(str::to_string)
			.collect();
	}

	fn save(&mut self) -> Result<()> {
		self.apply_tags();
		self.meta.validate()?;
		self.meta.save(&self.dir)?;

		let content = self.dir.join(&self.meta.content);
		fs::write(&content, &self.markdown)
			.with_context(|| format!("failed to write {}", content.display()))?;

		self.saved_meta = self.meta.clone();
		self.saved_tags = self.tags_text.clone();
		self.saved_markdown = self.markdown.clone();

		Ok(())
	}
}

struct SettingsState {
	config: BlogConfig,
	saved: BlogConfig,
	output: String,
	remote_url: String,
}

impl SettingsState {
	fn new(config: BlogConfig) -> Self {
		let output = config.build.output.display().to_string();
		Self {
			saved: config.clone(),
			config,
			output,
			remote_url: String::new(),
		}
	}

	fn sync(&mut self) {
		self.config.build.output = PathBuf::from(self.output.trim());
	}

	fn dirty(&self) -> bool {
		self.config != self.saved || *self.output.trim() != self.config.build.output
	}

	fn save(&mut self, root: &Path) -> Result<()> {
		self.sync();
		self.config.validate()?;
		self.config.save(root)?;
		self.saved = self.config.clone();
		Ok(())
	}
}

struct NewPageDialog {
	title: String,
	slug: String,
	error: String,
}

impl NewPageDialog {
	fn new() -> Self {
		Self {
			title: String::new(),
			slug: String::new(),
			error: String::new(),
		}
	}
}

struct Job {
	name: String,
	rx: Receiver<Result<String>>,
}

struct PreviewHandle {
	url: String,
	stop: Arc<AtomicBool>,
	thread: Option<JoinHandle<()>>,
}

impl Drop for PreviewHandle {
	fn drop(&mut self) {
		self.stop.store(true, Ordering::Relaxed);
		if let Some(thread) = self.thread.take() {
			let _ = thread.join();
		}
	}
}

impl Project {
	fn open(root: PathBuf) -> Result<Self> {
		let root = root
			.canonicalize()
			.with_context(|| format!("failed to resolve {}", root.display()))?;
		let config = BlogConfig::load(&root)?;

		let mut project = Self {
			root,
			settings: SettingsState::new(config.clone()),
			config,
			pages: Vec::new(),
			selected: None,
			editor: None,
			view: View::Pages,
			preview: None,
			new_page: None,
			job: None,
			log: Vec::new(),
		};

		project.reload_pages();
		if !project.pages.is_empty() {
			project.select_page(0);
		}

		Ok(project)
	}

	fn ui(&mut self, ui: &mut egui::Ui) {
		self.poll_job(ui.ctx());
		self.top_bar(ui);
		self.bottom_log(ui);
		self.sidebar(ui);

		egui::CentralPanel::default().show(ui, |ui| match self.view {
			View::Pages => self.editor_ui(ui),
			View::Settings => self.settings_ui(ui),
		});

		self.new_page_window(ui.ctx());
	}

	fn top_bar(&mut self, ui: &mut egui::Ui) {
		let mut build = false;
		let mut preview = false;
		let mut stop = false;
		let mut publish = false;
		let mut settings = false;

		egui::Panel::top("top").show(ui, |ui| {
			ui.horizontal(|ui| {
				ui.heading("bd");
				ui.separator();
				ui.label(self.root.display().to_string());

				ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
					if ui.button("Publish").clicked() {
						publish = true;
					}
					if self.preview.is_some() {
						if ui.button("Stop preview").clicked() {
							stop = true;
						}
					} else if ui.button("Preview").clicked() {
						preview = true;
					}
					if ui.button("Build").clicked() {
						build = true;
					}
					if ui.button("Settings").clicked() {
						settings = true;
					}
				});
			});

			if let Some(preview) = &self.preview {
				ui.horizontal(|ui| {
					ui.label("Preview:");
					ui.monospace(&preview.url);
					if ui.button("Copy URL").clicked() {
						ui.ctx().copy_text(preview.url.clone());
					}
					if ui.button("Open").clicked() {
						ui.ctx()
							.open_url(egui::OpenUrl::new_tab(preview.url.clone()));
					}
				});
			}
		});

		if build {
			self.build();
		}
		if preview {
			self.preview();
		}
		if stop {
			self.preview = None;
			self.log.push("Preview stopped.".to_string());
		}
		if publish {
			self.start_publish(None);
		}
		if settings {
			self.view = View::Settings;
		}
	}

	fn sidebar(&mut self, ui: &mut egui::Ui) {
		let mut clicked = None;
		let mut new_page = false;
		let mut settings = false;

		egui::Panel::left("pages")
			.resizable(true)
			.default_size(220.0)
			.show(ui, |ui| {
				ui.heading("Pages");
				if ui.button("+ New page").clicked() {
					new_page = true;
				}
				ui.separator();

				egui::ScrollArea::vertical().show(ui, |ui| {
					for (index, page) in self.pages.iter().enumerate() {
						let label = if page.draft {
							format!("[draft] {}", page.title)
						} else {
							page.title.clone()
						};
						if ui
							.selectable_label(self.selected == Some(index), label)
							.clicked()
						{
							clicked = Some(index);
						}
						ui.label(egui::RichText::new(&page.date).small().weak());
					}
				});

				ui.separator();
				if ui.button("Settings").clicked() {
					settings = true;
				}
			});

		if new_page {
			self.new_page = Some(NewPageDialog::new());
		}
		if let Some(index) = clicked {
			self.select_page(index);
		}
		if settings {
			self.view = View::Settings;
		}
	}

	fn bottom_log(&mut self, ui: &mut egui::Ui) {
		egui::Panel::bottom("log")
			.resizable(true)
			.default_size(120.0)
			.show(ui, |ui| {
				ui.horizontal(|ui| {
					ui.heading("Log");
					if self.job.is_some() {
						ui.spinner();
					}
					if ui.button("Clear").clicked() {
						self.log.clear();
					}
				});
				egui::ScrollArea::vertical()
					.stick_to_bottom(true)
					.show(ui, |ui| {
						for line in &self.log {
							ui.monospace(line);
						}
					});
			});
	}

	fn editor_ui(&mut self, ui: &mut egui::Ui) {
		let mut save = false;
		let mut publish = false;

		if let Some(editor) = &mut self.editor {
			ui.horizontal(|ui| {
				ui.heading("Page");
				if editor.dirty() {
					ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
				}
			});
			ui.label(
				egui::RichText::new(editor.dir.display().to_string())
					.small()
					.weak(),
			);
			ui.separator();

			egui::Grid::new("page_fields")
				.num_columns(2)
				.spacing([8.0, 6.0])
				.show(ui, |ui| {
					ui.label("Title");
					ui.text_edit_singleline(&mut editor.meta.title);
					ui.end_row();

					ui.label("Date");
					ui.add(egui::TextEdit::singleline(&mut editor.meta.date).desired_width(120.0));
					ui.end_row();

					ui.label("Description");
					ui.text_edit_singleline(&mut editor.meta.description);
					ui.end_row();

					ui.label("Tags");
					ui.text_edit_singleline(&mut editor.tags_text);
					ui.end_row();

					ui.label("Image");
					ui.text_edit_singleline(&mut editor.meta.image);
					ui.end_row();

					ui.label("Content file");
					ui.text_edit_singleline(&mut editor.meta.content);
					ui.end_row();

					ui.label("Draft");
					ui.checkbox(&mut editor.meta.draft, "Hide from listings");
					ui.end_row();
				});

			ui.separator();
			ui.label("Markdown");

			let footer_height =
				ui.spacing().interact_size.y + ui.spacing().item_spacing.y * 2.0 + 8.0;
			let available = (ui.available_height() - footer_height).max(240.0);

			ui.columns(2, |columns| {
				let pointer = columns[0].ctx().pointer_hover_pos();
				let scroll_input = columns[0]
					.input(|input| input.smooth_scroll_delta().y)
					.abs()
					> 0.0;

				let editor_hovered = pointer.is_some_and(|pos| columns[0].max_rect().contains(pos));
				let preview_hovered =
					pointer.is_some_and(|pos| columns[1].max_rect().contains(pos));

				let editor_target = editor.scroll.offset.min(editor.scroll.editor_max);
				let force_editor =
					!editor.scroll.initialized || editor.scroll.leader != ScrollPane::Editor;
				let editor_output = columns[0]
					.push_id("markdown_editor", |ui| {
						let mut area = egui::ScrollArea::vertical()
							.id_salt("scroll")
							.max_height(available)
							.auto_shrink([false, false]);
						if force_editor {
							area = area.vertical_scroll_offset(editor_target);
						}
						area.show(ui, |ui| {
							ui.add(
								egui::TextEdit::multiline(&mut editor.markdown)
									.code_editor()
									.desired_width(f32::INFINITY)
									.desired_rows(24),
							);
						})
					})
					.inner;

				let editor_offset = editor_output.state.clamped_offset().y;
				let editor_changed = (editor_offset - editor.scroll.editor_offset).abs() > 0.01;
				editor.scroll.editor_offset = editor_offset;
				editor.scroll.editor_max =
					(editor_output.content_size.y - editor_output.inner_rect.height()).max(0.0);
				if editor.scroll.leader == ScrollPane::Editor {
					editor.scroll.offset = editor_offset;
				}

				let preview_target = editor.scroll.offset.min(editor.scroll.preview_max);
				let force_preview =
					!editor.scroll.initialized || editor.scroll.leader != ScrollPane::Preview;
				let preview_output = columns[1]
					.push_id("markdown_preview", |ui| {
						let mut area = egui::ScrollArea::vertical()
							.id_salt("scroll")
							.max_height(available)
							.auto_shrink([false, false]);
						if force_preview {
							area = area.vertical_scroll_offset(preview_target);
						}
						area.show(ui, |ui| {
							CommonMarkViewer::new().show(ui, &mut editor.cache, &editor.markdown);
						})
					})
					.inner;

				let preview_offset = preview_output.state.clamped_offset().y;
				let preview_changed = (preview_offset - editor.scroll.preview_offset).abs() > 0.01;
				editor.scroll.preview_offset = preview_offset;
				editor.scroll.preview_max =
					(preview_output.content_size.y - preview_output.inner_rect.height()).max(0.0);

				let editor_bar_active = columns[0]
					.ctx()
					.read_response(editor_output.id.with(1_usize))
					.is_some_and(|response| {
						response.is_pointer_button_down_on() || response.dragged()
					});
				let preview_bar_active = columns[0]
					.ctx()
					.read_response(preview_output.id.with(1_usize))
					.is_some_and(|response| {
						response.is_pointer_button_down_on() || response.dragged()
					});

				if (editor_bar_active || (editor_hovered && scroll_input)) && editor_changed {
					editor.scroll.leader = ScrollPane::Editor;
				} else if (preview_bar_active || (preview_hovered && scroll_input))
					&& preview_changed
				{
					editor.scroll.leader = ScrollPane::Preview;
				}

				editor.scroll.offset = match editor.scroll.leader {
					ScrollPane::Editor => editor_offset,
					ScrollPane::Preview => preview_offset,
				};
				editor.scroll.initialized = true;
			});

			ui.separator();
			ui.horizontal(|ui| {
				if ui.button("Save").clicked() {
					save = true;
				}
				if ui.button("Publish this page").clicked() {
					publish = true;
				}
			});
		} else {
			ui.centered_and_justified(|ui| {
				ui.label("Select a page or create a new one.");
			});
		}

		if save && let Some(editor) = &mut self.editor {
			match editor.save() {
				Ok(()) => {
					let slug = editor.slug();
					self.log.push(format!("Saved page \"{slug}\"."));
					self.reload_pages();
				}
				Err(error) => self.log.push(format!("Save failed: {error:#}")),
			}
		}

		if publish && let Some(slug) = self.editor.as_ref().map(PageEditor::slug) {
			self.start_publish(Some(slug));
		}
	}

	fn settings_ui(&mut self, ui: &mut egui::Ui) {
		let mut save = false;
		let mut apply = false;
		let mut set_remote = false;

		ui.heading("Settings");
		ui.separator();

		{
			let settings = &mut self.settings;
			let config = &mut settings.config;

			egui::Grid::new("settings")
				.num_columns(2)
				.spacing([8.0, 6.0])
				.show(ui, |ui| {
					ui.label("Title");
					ui.text_edit_singleline(&mut config.title);
					ui.end_row();

					ui.label("Description");
					ui.text_edit_singleline(&mut config.description);
					ui.end_row();

					ui.label("Author");
					ui.text_edit_singleline(&mut config.author);
					ui.end_row();

					ui.label("Language");
					ui.text_edit_singleline(&mut config.language);
					ui.end_row();

					ui.label("Favicon");
					ui.text_edit_singleline(&mut config.favicon);
					ui.end_row();

					ui.label("Build output");
					ui.text_edit_singleline(&mut settings.output);
					ui.end_row();

					ui.label("Include drafts");
					ui.checkbox(&mut config.build.include_drafts, "");
					ui.end_row();
				});

			ui.separator();
			ui.label("Theme");
			egui::ComboBox::from_label("Template")
				.selected_text(config.template.as_str())
				.show_ui(ui, |ui| {
					for theme in ThemeName::ALL {
						ui.selectable_value(&mut config.template, theme, theme.as_str());
					}
				});

			if ui
				.button("Apply theme (overwrites templates/ and static/)")
				.clicked()
			{
				apply = true;
			}

			ui.separator();
			ui.label("Footer links");
			let mut remove = None;
			for (index, link) in config.footer.iter_mut().enumerate() {
				ui.horizontal(|ui| {
					ui.add(
						egui::TextEdit::singleline(&mut link.title)
							.desired_width(180.0)
							.hint_text("Title"),
					);
					ui.add(
						egui::TextEdit::singleline(&mut link.url)
							.desired_width(360.0)
							.hint_text("URL"),
					);
					if ui.button("Remove").clicked() {
						remove = Some(index);
					}
				});
			}
			if let Some(index) = remove {
				config.footer.remove(index);
			}
			if ui.button("Add footer link").clicked() {
				config.footer.push(FooterLink {
					title: String::new(),
					url: String::new(),
				});
			}

			ui.separator();
			ui.label("Remote URL (used when no git remote exists)");
			ui.text_edit_singleline(&mut settings.remote_url);
			if ui.button("Set remote").clicked() {
				set_remote = true;
			}

			ui.separator();
			ui.horizontal(|ui| {
				if ui.button("Save settings").clicked() {
					save = true;
				}
				if settings.dirty() {
					ui.colored_label(egui::Color32::YELLOW, "Unsaved changes");
				}
			});
		}

		if save {
			match self.settings.save(&self.root) {
				Ok(()) => {
					self.config = self.settings.config.clone();
					self.log.push("Saved blog.toml.".to_string());
				}
				Err(error) => self.log.push(format!("Settings failed: {error:#}")),
			}
		}

		if set_remote {
			let url = self.settings.remote_url.trim().to_string();
			if url.is_empty() {
				self.log.push("Enter a remote URL first.".to_string());
			} else {
				match git::add_remote(&self.root, "origin", &url) {
					Ok(()) => self.log.push(format!("Added remote \"origin\" ({url}).")),
					Err(error) => self.log.push(format!("Remote failed: {error:#}")),
				}
			}
		}

		if apply {
			let theme = self.settings.config.template;
			let result = rfd::MessageDialog::new()
				.set_title("Apply theme")
				.set_description(
					"This overwrites templates/ and static/. Custom templates are replaced. Continue?",
				)
				.set_buttons(rfd::MessageButtons::YesNo)
				.show();

			if result == rfd::MessageDialogResult::Yes {
				match init::apply_theme(&self.root, theme) {
					Ok(()) => {
						self.config.template = theme;
						self.settings.config.template = theme;
						self.settings.save(&self.root).ok();
						self.log
							.push(format!("Applied theme \"{}\".", theme.as_str()));
					}
					Err(error) => self.log.push(format!("Apply theme failed: {error:#}")),
				}
			}
		}
	}

	fn new_page_window(&mut self, ctx: &egui::Context) {
		let mut create = false;
		let mut cancel = false;

		if let Some(dialog) = &mut self.new_page {
			egui::Window::new("New page")
				.collapsible(false)
				.resizable(false)
				.show(ctx, |ui| {
					ui.label("Title");
					ui.text_edit_singleline(&mut dialog.title);
					ui.label("Slug (optional)");
					ui.text_edit_singleline(&mut dialog.slug);
					if !dialog.error.is_empty() {
						ui.colored_label(egui::Color32::RED, &dialog.error);
					}
					ui.horizontal(|ui| {
						if ui.button("Create").clicked() {
							create = true;
						}
						if ui.button("Cancel").clicked() {
							cancel = true;
						}
					});
				});
		}

		if cancel {
			self.new_page = None;
		}

		if create && let Some(dialog) = self.new_page.take() {
			match self.create_page(&dialog.title, &dialog.slug) {
				Ok(()) => self.log.push("Created page.".to_string()),
				Err(error) => {
					self.log.push(format!("New page failed: {error:#}"));
					self.new_page = Some(NewPageDialog {
						title: dialog.title,
						slug: dialog.slug,
						error: format!("{error:#}"),
					});
				}
			}
		}
	}

	fn create_page(&mut self, title: &str, slug_value: &str) -> Result<()> {
		let title = title.trim();
		if title.is_empty() {
			bail!("title cannot be empty");
		}

		let page_slug = if slug_value.trim().is_empty() {
			slug::slugify(title)
		} else {
			slug_value.trim().to_string()
		};
		slug::validate(&page_slug)?;

		let dir = self.root.join(&page_slug);
		if dir.exists() {
			bail!("page directory already exists: {}", dir.display());
		}

		fs::create_dir(&dir).with_context(|| format!("failed to create {}", dir.display()))?;

		let meta = PageFile {
			title: title.to_string(),
			description: String::new(),
			date: format_date(today())?,
			draft: false,
			tags: Vec::new(),
			image: String::new(),
			content: "page.md".to_string(),
		};

		if let Err(error) = meta.save(&dir) {
			let _ = fs::remove_dir_all(&dir);
			return Err(error);
		}

		fs::write(dir.join("page.md"), default_page_markdown())
			.with_context(|| format!("failed to write {}", dir.join("page.md").display()))?;

		self.reload_pages();
		if let Some(index) = self.pages.iter().position(|page| page.slug == page_slug) {
			self.select_page(index);
		}
		Ok(())
	}

	fn select_page(&mut self, index: usize) {
		let Some(summary) = self.pages.get(index).cloned() else {
			return;
		};

		match PageEditor::load(summary.dir) {
			Ok(editor) => {
				self.editor = Some(editor);
				self.selected = Some(index);
				self.view = View::Pages;
			}
			Err(error) => self.log.push(format!("{error:#}")),
		}
	}

	fn reload_pages(&mut self) {
		let selected_slug = self
			.selected
			.and_then(|index| self.pages.get(index))
			.map(|page| page.slug.clone());

		match scan_pages(&self.root, &self.config.build.output) {
			Ok((pages, errors)) => {
				self.pages = pages;
				self.log.extend(errors);
				if let Some(slug) = selected_slug
					&& let Some(index) = self.pages.iter().position(|page| page.slug == slug)
				{
					self.selected = Some(index);
				}
			}
			Err(error) => self.log.push(format!("{error:#}")),
		}
	}

	fn build(&mut self) {
		match build::build_in(&self.root, &build::BuildArgs::default()) {
			Ok(outcome) => self.log.push(format!(
				"Built {} page{} to {}",
				outcome.pages,
				if outcome.pages == 1 { "" } else { "s" },
				outcome.output.display()
			)),
			Err(error) => self.log.push(format!("Build failed: {error:#}")),
		}
	}

	fn preview(&mut self) {
		self.preview = None;

		let outcome = match build::build_in(&self.root, &build::BuildArgs::default()) {
			Ok(outcome) => outcome,
			Err(error) => {
				self.log.push(format!("Preview build failed: {error:#}"));
				return;
			}
		};

		let server = match PreviewServer::bind(outcome.output, "127.0.0.1", PORT) {
			Ok(server) => server,
			Err(error) => {
				self.log.push(format!("Preview failed: {error:#}"));
				self.log.push(format!(
					"Hint: port {PORT} may already be in use. Stop running other `bd preview` and try again."
				));
				return;
			}
		};

		let url = format!("http://{}", server.addr());
		let stop = Arc::new(AtomicBool::new(false));
		let stop_thread = stop.clone();
		let thread = std::thread::spawn(move || {
			if let Err(error) = server.run_until(stop_thread) {
				eprintln!("preview: {error:#}");
			}
		});

		self.log.push(format!("Previewing {url}"));
		self.preview = Some(PreviewHandle {
			url,
			stop,
			thread: Some(thread),
		});
	}

	fn start_publish(&mut self, page: Option<String>) {
		if self.job.is_some() {
			self.log.push("A job is already running.".to_string());
			return;
		}

		let remotes = git::remotes(&self.root).unwrap_or_default();
		if remotes.is_empty() {
			let url = self.settings.remote_url.trim().to_string();
			if url.is_empty() {
				self.log
					.push("No git remote is configured. Add one in Settings.".to_string());
				self.view = View::Settings;
				return;
			}

			if let Err(error) = git::add_remote(&self.root, "origin", &url) {
				self.log.push(format!("Remote failed: {error:#}"));
				return;
			}
		}

		let root = self.root.clone();
		let (tx, rx) = mpsc::channel();
		std::thread::spawn(move || {
			let result = publish::run_in(&root, page.as_deref(), None, false, false)
				.map(|()| "Published".to_string());
			let _ = tx.send(result);
		});

		self.job = Some(Job {
			name: "Publish".to_string(),
			rx,
		});
	}

	fn poll_job(&mut self, ctx: &egui::Context) {
		let Some(job) = self.job.take() else {
			return;
		};

		match job.rx.try_recv() {
			Ok(Ok(message)) => self.log.push(format!("{}: {message}", job.name)),
			Ok(Err(error)) => self.log.push(format!("{} failed: {error:#}", job.name)),
			Err(TryRecvError::Empty) => {
				self.job = Some(job);
				ctx.request_repaint_after(Duration::from_millis(200));
			}
			Err(TryRecvError::Disconnected) => {
				self.log.push(format!("{}: worker stopped", job.name));
			}
		}
	}
}

fn scan_pages(root: &Path, output: &Path) -> Result<(Vec<PageSummary>, Vec<String>)> {
	let output = root.join(output);
	let mut pages = Vec::new();
	let mut errors = Vec::new();

	for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
		let entry = entry?;
		let path = entry.path();
		if !path.is_dir() {
			continue;
		}

		let name = entry.file_name().to_string_lossy().to_string();
		if name.starts_with('.')
			|| name == "static"
			|| name == "templates"
			|| path == output
			|| !path.join(crate::page::PAGE_META).is_file()
		{
			continue;
		}

		match PageFile::load(&path) {
			Ok(meta) => pages.push(PageSummary {
				dir: path,
				slug: name,
				title: meta.title,
				date: meta.date,
				draft: meta.draft,
			}),
			Err(error) => errors.push(format!("{error:#}")),
		}
	}

	pages.sort_by(|left, right| {
		right
			.date
			.cmp(&left.date)
			.then_with(|| left.slug.cmp(&right.slug))
	});

	Ok((pages, errors))
}
