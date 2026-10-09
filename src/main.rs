mod gdebi;
mod i18n;
mod installer;

use std::cell::RefCell;
use std::rc::Rc;

use gtk::gio;
use gtk::prelude::*;

fn main() {
    i18n::init();
    let arguments: Vec<String> = std::env::args().collect();
    if arguments.get(1).map(String::as_str) == Some("--cli") {
        let Some(path) = arguments.get(2) else {
            eprintln!("{}", i18n::tr("Usage: gdebi-rs --cli PACKAGE.deb"));
            std::process::exit(2);
        };
        std::process::exit(gdebi::gdebi_cli::GDebiCli::run(path));
    }

    let application = gtk::Application::builder()
        .application_id("org.gdebi.GDebi")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    // Keep one window while the application is alive.  This also makes a
    // second double-click reuse the existing window instead of launching a
    // second GTK process.
    let current_window: Rc<RefCell<Option<Rc<gdebi::gdebi_gtk::PackageWindow>>>> =
        Rc::new(RefCell::new(None));

    let activate_window = current_window.clone();
    application.connect_activate(move |application| {
        let window = get_or_create_window(application, &activate_window);
        window.present();
    });

    let open_window = current_window.clone();
    application.connect_open(move |application, files, _hint| {
        let window = get_or_create_window(application, &open_window);
        if let Some(file) = files.first() {
            if let Some(path) = file.path() {
                window.open_path(path);
            } else {
                window.show_error(
                    &i18n::tr("Unable to open package"),
                    &i18n::tr("This location is not a local file."),
                );
            }
        }
        window.present();
    });

    application.run();
}

fn get_or_create_window(
    application: &gtk::Application,
    current_window: &Rc<RefCell<Option<Rc<gdebi::gdebi_gtk::PackageWindow>>>>,
) -> Rc<gdebi::gdebi_gtk::PackageWindow> {
    if let Some(window) = current_window.borrow().as_ref() {
        return window.clone();
    }

    let window = gdebi::gdebi_gtk::PackageWindow::new(application);
    *current_window.borrow_mut() = Some(window.clone());
    window
}
