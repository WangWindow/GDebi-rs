use super::deb_package::DebPackage;
use super::deb_package::extract_icon;
use super::gdebi_common::GDebiCommon;
use crate::i18n::tr;
use crate::installer::{self, InstallEvent};

use gtk::prelude::*;
use gtk::{gdk, gio, glib};

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, TryRecvError};
use std::thread;
use std::time::Duration;

pub struct PackageWindow {
    pub window: gtk::ApplicationWindow,
    package: RefCell<Option<DebPackage>>,
    busy: Cell<bool>,
    icon_generation: Cell<u64>,
    package_icon: gtk::Image,
    icon_stack: gtk::Stack,
    drop_zone: gtk::Box,
    header_title: gtk::Label,
    name_label: gtk::Label,
    summary_label: gtk::Label,
    meta_label: gtk::Label,
    output_view: gtk::TextView,
    dependency_label: gtk::Label,
    dependency_scroll: gtk::ScrolledWindow,
    dependency_expander: gtk::Expander,
    status_label: gtk::Label,
    progress: gtk::ProgressBar,
    install_button: gtk::Button,
    open_button: gtk::Button,
    log_expander: gtk::Expander,
}

impl PackageWindow {
    pub fn new(application: &gtk::Application) -> Rc<Self> {
        let header = gtk::HeaderBar::new();
        let header_title = gtk::Label::new(Some("GDebi"));
        header.set_title_widget(Some(&header_title));

        let open_button = gtk::Button::from_icon_name("document-open-symbolic");
        open_button.set_tooltip_text(Some(&tr("Open package")));
        header.pack_start(&open_button);

        let install_button = gtk::Button::with_label(&tr("Install package"));
        install_button.add_css_class("suggested-action");
        install_button.set_valign(gtk::Align::Center);
        install_button.set_sensitive(false);

        let package_icon = gtk::Image::from_icon_name("package-x-generic");
        package_icon.set_pixel_size(56);
        package_icon.add_css_class("package-icon");

        let success_icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
        success_icon.set_pixel_size(56);
        success_icon.add_css_class("success");
        let icon_stack = gtk::Stack::new();
        icon_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        icon_stack.set_transition_duration(240);
        icon_stack.set_halign(gtk::Align::Center);
        icon_stack.add_named(&package_icon, Some("package"));
        icon_stack.add_named(&success_icon, Some("success"));
        icon_stack.set_visible_child_name("package");

        let name_label = gtk::Label::new(Some(&tr("Drop a .deb file here")));
        name_label.set_halign(gtk::Align::Center);
        name_label.set_justify(gtk::Justification::Center);
        name_label.add_css_class("title-3");

        let summary_label = gtk::Label::new(Some(""));
        summary_label.set_halign(gtk::Align::Center);
        summary_label.set_wrap(true);
        summary_label.set_justify(gtk::Justification::Center);
        summary_label.add_css_class("dim-label");
        summary_label.set_visible(false);

        let meta_label = gtk::Label::new(Some(""));
        meta_label.set_halign(gtk::Align::Center);
        meta_label.set_justify(gtk::Justification::Center);
        meta_label.add_css_class("dim-label");
        meta_label.set_visible(false);

        let hero = gtk::Box::new(gtk::Orientation::Vertical, 8);
        hero.set_halign(gtk::Align::Center);
        hero.set_valign(gtk::Align::Center);
        hero.set_margin_top(12);
        hero.set_margin_bottom(4);
        hero.append(&icon_stack);
        hero.append(&name_label);
        hero.append(&summary_label);
        hero.append(&meta_label);

        let drop_zone = gtk::Box::new(gtk::Orientation::Vertical, 0);
        drop_zone.add_css_class("drop-zone");
        drop_zone.append(&hero);

        let output_view = read_only_text_view(true);

        let log_expander = gtk::Expander::builder()
            .label(tr("Installation log"))
            .child(&text_scroller(&output_view))
            .build();
        log_expander.set_visible(false);

        let dependency_label = gtk::Label::new(None);
        dependency_label.set_halign(gtk::Align::Start);
        dependency_label.set_xalign(0.0);
        dependency_label.set_wrap(true);
        dependency_label.set_selectable(true);
        dependency_label.set_margin_top(8);
        dependency_label.set_margin_bottom(8);
        dependency_label.set_margin_start(8);
        dependency_label.set_margin_end(8);
        let dependency_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .max_content_height(120)
            .child(&dependency_label)
            .build();
        dependency_scroll.set_visible(false);
        let dependency_expander = gtk::Expander::builder()
            .label(tr("Dependencies"))
            .child(&dependency_scroll)
            .build();
        dependency_expander.set_visible(false);

        let status_label = gtk::Label::new(None);
        status_label.set_halign(gtk::Align::Start);
        status_label.set_xalign(0.0);
        status_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        status_label.set_hexpand(true);
        status_label.set_visible(false);

        let progress = gtk::ProgressBar::new();
        progress.set_show_text(true);
        progress.set_visible(false);

        let action_bar = gtk::ActionBar::new();
        action_bar.pack_start(&status_label);
        action_bar.set_center_widget(Some(&progress));
        action_bar.pack_end(&install_button);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
        content.set_margin_top(2);
        content.set_margin_bottom(8);
        content.set_margin_start(6);
        content.set_margin_end(6);
        content.set_valign(gtk::Align::Center);
        content.set_halign(gtk::Align::Center);
        content.set_hexpand(false);
        content.set_width_request(440);
        content.append(&drop_zone);
        content.append(&dependency_expander);
        content.append(&log_expander);

        let content_scroll = gtk::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .propagate_natural_width(true)
            .propagate_natural_height(true)
            .child(&content)
            .build();

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.set_vexpand(true);
        root.append(&content_scroll);
        root.append(&action_bar);

        let window = gtk::ApplicationWindow::builder()
            .application(application)
            .title("GDebi")
            .default_width(500)
            .default_height(420)
            .build();
        window.set_titlebar(Some(&header));
        window.set_child(Some(&root));

        let state = Rc::new(Self {
            window,
            package: RefCell::new(None),
            busy: Cell::new(false),
            icon_generation: Cell::new(0),
            package_icon,
            icon_stack,
            drop_zone,
            header_title,
            name_label,
            summary_label,
            meta_label,
            output_view,
            dependency_label,
            dependency_scroll,
            dependency_expander,
            status_label,
            progress,
            install_button,
            open_button,
            log_expander,
        });

        install_drop_target(&state);

        state.connect_actions();
        state
    }

    fn connect_actions(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.open_button.connect_clicked(move |_| {
            if let Some(state) = weak.upgrade() {
                state.choose_file();
            }
        });

        let weak = Rc::downgrade(self);
        self.install_button.connect_clicked(move |_| {
            if let Some(state) = weak.upgrade() {
                state.install_package();
            }
        });
    }

    pub fn present(&self) {
        self.window.present();
    }

    pub fn open_path(self: &Rc<Self>, path: impl Into<PathBuf>) {
        if self.busy.replace(true) {
            return;
        }
        let path = path.into();
        self.set_loading(true);

        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let _ = sender.send(GDebiCommon::open(&path));
        });

        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let Some(state) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            match receiver.try_recv() {
                Ok(Ok(package)) => {
                    state.display_package(package);
                    state.busy.set(false);
                    state.set_loading(false);
                    glib::ControlFlow::Break
                }
                Ok(Err(error)) => {
                    state.show_open_error(&error);
                    state.busy.set(false);
                    state.set_loading(false);
                    glib::ControlFlow::Break
                }
                Err(TryRecvError::Empty) => {
                    state.progress.pulse();
                    glib::ControlFlow::Continue
                }
                Err(TryRecvError::Disconnected) => {
                    state.show_open_error(&tr("Unable to open package"));
                    state.busy.set(false);
                    state.set_loading(false);
                    glib::ControlFlow::Break
                }
            }
        });
    }

    fn show_open_error(&self, error: &str) {
        self.icon_generation
            .set(self.icon_generation.get().wrapping_add(1));
        self.package.replace(None);
        self.install_button.set_sensitive(false);
        self.name_label.set_text(&tr("Unable to open package"));
        self.header_title.set_text("GDebi");
        self.package_icon
            .set_from_icon_name(Some("package-x-generic"));
        self.icon_stack.set_visible_child_name("package");
        self.summary_label.set_visible(false);
        self.meta_label.set_text("");
        self.meta_label.set_visible(false);
        self.status_label.set_visible(false);
        self.dependency_label.set_text("");
        self.dependency_scroll.set_visible(false);
        self.dependency_expander.set_visible(false);
        self.status_label.remove_css_class("success");
        self.log_expander.set_visible(false);
        self.show_error(&tr("Unable to open package"), error);
    }

    pub fn show_error(&self, heading: &str, body: &str) {
        let dialog = gtk::MessageDialog::builder()
            .transient_for(&self.window)
            .modal(true)
            .message_type(gtk::MessageType::Error)
            .buttons(gtk::ButtonsType::Close)
            .text(heading)
            .secondary_text(body)
            .build();
        dialog.connect_response(|dialog, _| dialog.close());
        dialog.present();
    }

    fn display_package(self: &Rc<Self>, package: DebPackage) {
        let icon_path = package.path.clone();
        let generation = self.icon_generation.get().wrapping_add(1);
        self.icon_generation.set(generation);
        let display_name = format!("{} — {}", package.package_name(), package.version());
        self.window
            .set_title(Some(&format!("GDebi — {display_name}")));
        self.name_label.set_text(&package.package_name());
        self.header_title.set_text(&package.package_name());
        self.icon_stack.set_visible_child_name("package");
        self.package_icon
            .set_from_icon_name(Some("package-x-generic"));
        let description = package.description();
        self.summary_label
            .set_text(description.lines().next().unwrap_or(""));
        self.summary_label.set_visible(true);
        self.meta_label.set_text(&format!(
            "{}  ·  {}",
            package.version(),
            package.architecture()
        ));
        self.meta_label.set_visible(true);

        let dependencies = package.dependencies();
        if dependencies.is_empty() {
            self.dependency_label.set_text("");
            self.dependency_scroll.set_visible(false);
            self.dependency_expander.set_visible(false);
        } else {
            self.dependency_label.set_text(
                &dependencies
                    .into_iter()
                    .map(|(_, value)| value)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            self.dependency_scroll.set_visible(true);
            self.dependency_expander.set_visible(true);
            self.dependency_expander.set_expanded(false);
        }

        self.package.replace(Some(package));
        self.load_icon(icon_path, generation);
        self.install_button.set_label(&tr("Install package"));
        self.install_button.set_sensitive(true);
        self.status_label.set_visible(false);
        self.status_label.remove_css_class("error");
        self.status_label.remove_css_class("success");
        self.progress.set_visible(false);
        self.log_expander.set_visible(false);
    }

    fn load_icon(self: &Rc<Self>, package: PathBuf, generation: u64) {
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || {
            let icon = extract_icon(&package);
            let _ = sender.send((package, icon));
        });

        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(100), move || {
            let Some(state) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            match receiver.try_recv() {
                Ok((path, Some(icon))) => {
                    let current = state
                        .package
                        .borrow()
                        .as_ref()
                        .map(|package| package.path == path)
                        .unwrap_or(false);
                    if state.icon_generation.get() == generation && current {
                        state.package_icon.set_from_file(Some(&icon));
                    } else if let Some(parent) = icon.parent() {
                        let _ = std::fs::remove_dir_all(parent);
                    }
                    glib::ControlFlow::Break
                }
                Ok((_, None)) => glib::ControlFlow::Break,
                Err(TryRecvError::Empty) => glib::ControlFlow::Continue,
                Err(TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    }

    fn choose_file(self: &Rc<Self>) {
        // FileChooserNative is available in the GTK 4.0 API set.  Do not use
        // GtkFileDialog here: it was introduced later and would raise the
        // minimum GTK runtime version for this application.
        let dialog = gtk::FileChooserNative::new(
            Some(&tr("Open Debian package")),
            Some(&self.window),
            gtk::FileChooserAction::Open,
            Some(&tr("Open")),
            Some(&tr("Cancel")),
        );
        let filter = gtk::FileFilter::new();
        filter.set_name(Some(&tr("Debian packages (*.deb)")));
        filter.add_pattern("*.deb");
        dialog.set_filter(&filter);

        let weak = Rc::downgrade(self);
        dialog.connect_response(move |dialog, response| {
            if response == gtk::ResponseType::Accept
                && let Some(state) = weak.upgrade()
                && let Some(file) = dialog.file()
                && let Some(path) = file.path()
            {
                state.open_path(path);
            }
            dialog.destroy();
        });
        dialog.show();
    }

    fn install_package(self: &Rc<Self>) {
        if self.busy.replace(true) {
            return;
        }
        let Some(package) = self.package.borrow().clone() else {
            self.busy.set(false);
            return;
        };

        self.install_button.set_sensitive(false);
        self.open_button.set_sensitive(false);
        self.window.set_deletable(false);
        self.progress.set_visible(true);
        self.icon_stack.set_visible_child_name("package");
        self.status_label.remove_css_class("success");
        self.progress.set_fraction(0.0);
        self.progress
            .set_text(Some(&tr("Requesting administrator authorization…")));
        self.status_label.set_visible(true);
        self.status_label.set_text(&tr("Authenticating…"));
        self.status_label.remove_css_class("error");
        self.output_view.buffer().set_text("");
        self.log_expander.set_visible(true);
        self.log_expander.set_expanded(true);

        let (sender, receiver) = mpsc::channel();
        installer::install(&package.path, sender);

        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(80), move || {
            let Some(state) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };

            loop {
                match receiver.try_recv() {
                    Ok(InstallEvent::Output(line)) => state.append_log(&line),
                    Ok(InstallEvent::Finished(result)) => {
                        state.finish_install(result);
                        return glib::ControlFlow::Break;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        state.finish_install(Err(tr(
                            "The installation process exited unexpectedly",
                        )));
                        return glib::ControlFlow::Break;
                    }
                }
            }
            glib::ControlFlow::Continue
        });
    }

    fn append_log(&self, line: &str) {
        let buffer = self.output_view.buffer();
        let mut end = buffer.end_iter();
        buffer.insert(&mut end, &format!("{line}\n"));
    }

    fn finish_install(&self, result: Result<(), String>) {
        self.busy.set(false);
        self.open_button.set_sensitive(true);
        self.window.set_deletable(true);
        self.progress.set_fraction(1.0);

        match result {
            Ok(()) => {
                self.icon_stack.set_visible_child_name("success");
                self.progress.set_visible(false);
                self.log_expander.set_visible(false);
                self.status_label.set_visible(true);
                self.status_label
                    .set_text(&tr("The package was installed successfully."));
                self.install_button.set_label(&tr("Reinstall package"));
                self.status_label.remove_css_class("error");
                self.status_label.add_css_class("success");
                self.log_expander.set_visible(true);
                self.log_expander.set_expanded(false);
            }
            Err(error) => {
                self.icon_stack.set_visible_child_name("package");
                self.progress.set_text(Some(&tr("Installation failed")));
                self.status_label.set_visible(true);
                self.status_label.set_text(&error);
                self.status_label.add_css_class("error");
                self.status_label.remove_css_class("success");
                self.install_button.set_label(&tr("Retry installation"));
                self.show_error(&tr("Package installation failed"), &error);
            }
        }
        self.install_button.set_sensitive(true);
    }

    fn set_loading(&self, loading: bool) {
        self.open_button.set_sensitive(!loading);
        if loading {
            self.install_button.set_sensitive(false);
            self.status_label.set_visible(false);
            self.progress.set_visible(true);
            self.progress.pulse();
        } else if self.package.borrow().is_none() {
            self.progress.set_visible(false);
        }
    }
}

fn install_drop_target(state: &Rc<PackageWindow>) {
    install_drop_css();
    let target = gtk::DropTarget::new(gio::File::static_type(), gdk::DragAction::COPY);
    let weak = Rc::downgrade(state);
    target.connect_enter(move |_, _, _| {
        if let Some(state) = weak.upgrade() {
            if state.busy.get() {
                return gdk::DragAction::empty();
            }
            state.drop_zone.add_css_class("drop-active");
        }
        gdk::DragAction::COPY
    });
    let weak = Rc::downgrade(state);
    target.connect_leave(move |_| {
        if let Some(state) = weak.upgrade() {
            state.drop_zone.remove_css_class("drop-active");
        }
    });
    let weak = Rc::downgrade(state);
    target.connect_drop(move |_, value, _, _| {
        let Some(state) = weak.upgrade() else {
            return false;
        };
        if state.busy.get() {
            return false;
        }
        state.drop_zone.remove_css_class("drop-active");
        let Ok(file) = value.get::<gio::File>() else {
            return false;
        };
        let Some(path) = file.path() else {
            state.show_error(
                &tr("Unable to open package"),
                &tr("This location is not a local file."),
            );
            return false;
        };
        state.open_path(path);
        true
    });
    state.window.add_controller(target);
}

fn install_drop_css() {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let provider = gtk::CssProvider::new();
    provider.load_from_data(
        ".drop-zone { border: 1px dashed transparent; border-radius: 12px; padding: 8px; }\
         .drop-zone.drop-active { border-color: @theme_selected_bg_color; background-color: alpha(@theme_selected_bg_color, 0.12); }",
    );
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn read_only_text_view(monospace: bool) -> gtk::TextView {
    let view = gtk::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(if monospace {
            gtk::WrapMode::None
        } else {
            gtk::WrapMode::WordChar
        })
        .left_margin(8)
        .right_margin(8)
        .top_margin(8)
        .bottom_margin(8)
        .build();
    if monospace {
        view.add_css_class("monospace");
    }
    view
}

fn text_scroller(view: &gtk::TextView) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .min_content_height(180)
        .child(view)
        .build()
}
